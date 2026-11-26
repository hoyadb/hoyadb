//! SPEC-0026 / API-0004: publish a complete schema/data/metadata candidate once.
use super::SnapshotStorage;
use crate::{
    model::{RESERVED_PREFIX, ROW_VERSION},
    storage::*,
};
use std::collections::BTreeMap;

fn backend(message: &str) -> StorageError {
    StorageError::BackendFailure {
        message: message.into(),
    }
}

impl SnapshotStorage {
    pub(super) fn model_catalog(&self) -> Result<ModelCatalog, StorageError> {
        let state = self
            .tables
            .get(MODEL_TABLE)
            .map(|rows| model_state_from_rows(&rows.values().cloned().collect::<Vec<_>>()))
            .transpose()?;
        self.sync_rows()?;
        let catalog = checked_model_catalog(
            state,
            self.tables
                .keys()
                .filter(|name| name.as_str() != MODEL_TABLE && name.as_str() != SYNC_TABLE)
                .cloned()
                .collect(),
        )?;
        for table in &catalog.tables {
            for row in self.tables[table].values() {
                if row.fields.keys().any(|name| {
                    name.starts_with(RESERVED_PREFIX)
                        && (name != ROW_VERSION || catalog.state.is_none())
                }) {
                    return Err(backend("unknown reserved record bookkeeping"));
                }
                if let Some(state) = &catalog.state
                    && row.fields.get(ROW_VERSION)
                        != Some(&Value::Integer(i64::from(state.version)))
                {
                    return Err(backend("managed row version differs from schema"));
                }
            }
        }
        Ok(catalog)
    }

    fn model_rows(&self) -> BTreeMap<String, Vec<Row>> {
        self.tables
            .iter()
            .filter(|(name, _)| name.as_str() != MODEL_TABLE && name.as_str() != SYNC_TABLE)
            .map(|(name, rows)| (name.clone(), rows.values().cloned().collect()))
            .collect()
    }
}

impl SchemaStorage for SnapshotStorage {
    async fn catalog(&self) -> Result<ModelCatalog, StorageError> {
        self.model_catalog()
    }

    async fn scan(&self, table: &str) -> Result<Vec<Row>, StorageError> {
        let rows = self
            .tables
            .get(table)
            .ok_or_else(|| StorageError::TableNotFound {
                table: table.into(),
            })?;
        crate::storage::count_work!(memory_rows_visited, rows.len() as u64);
        Ok(rows.values().cloned().collect())
    }

    async fn apply_schema_plan(&mut self, plan: SchemaPlan) -> Result<(), StorageError> {
        validate_schema_plan(&plan)?;
        if let Some(rows) = self.sync_rows()? {
            validate_sync_schema_transition(&rows, &plan.next)?;
        }
        if self.model_catalog()?.state != plan.expected {
            return Err(schema_conflict("schema state changed before transition"));
        }
        let mut candidate = self.whole_store_clone();
        for operation in plan.operations {
            match operation {
                SchemaMutation::CreateTable(name) => {
                    if candidate.tables.contains_key(&name) {
                        return Err(StorageError::InvalidSchema {
                            message: "schema table already exists".into(),
                        });
                    }
                    candidate.create_table(&name).await?;
                }
                SchemaMutation::DropTable(name) => {
                    if candidate.tables.remove(&name).is_none() {
                        return Err(StorageError::TableNotFound {
                            table: name,
                        });
                    }
                    candidate.indexes.remove(&name);
                }
                SchemaMutation::Write(mutation) => match mutation {
                    Mutation::Insert {
                        table,
                        row,
                    } => candidate.insert(&table, row).await?,
                    Mutation::Replace {
                        table,
                        row,
                    } => candidate.replace(&table, row).await?,
                    Mutation::Remove {
                        table,
                        id,
                    } => {
                        candidate.remove(&table, &id).await?;
                    }
                },
                SchemaMutation::Validate {
                    schema,
                    version,
                } => validate_model_inventory(&schema, version, &candidate.model_rows())?,
            }
        }
        validate_model_inventory(&plan.schema, plan.next.version, &candidate.model_rows())?;
        let row = model_state_row(&plan.next)?;
        if plan.expected.is_none() {
            candidate.create_table(MODEL_TABLE).await?;
            candidate.insert(MODEL_TABLE, row).await?;
        } else {
            candidate.replace(MODEL_TABLE, row).await?;
        }
        *self = candidate;
        Ok(())
    }
}
