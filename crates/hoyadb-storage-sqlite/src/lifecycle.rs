//! SPEC-0026 / API-0004: logical schema and metadata in one real transaction.
use super::{SqliteStorage, backend, get, insert, remove, replace, require_table};
use hoyadb_core::{
    model::{RESERVED_PREFIX, ROW_VERSION},
    storage::*,
};
use rusqlite::{Connection, params};
use std::{
    collections::BTreeMap,
    future::{Future, ready},
};

pub(super) fn names(connection: &Connection) -> Result<Vec<String>, StorageError> {
    connection
        .prepare("SELECT name FROM __hoya_tables ORDER BY name")
        .map_err(backend)?
        .query_map([], |row| row.get(0))
        .map_err(backend)?
        .collect::<Result<_, _>>()
        .map_err(backend)
}

pub(super) fn scan(connection: &Connection, table: &str) -> Result<Vec<Row>, StorageError> {
    require_table(connection, table)?;
    let mut statement = connection
        .prepare("SELECT id FROM __hoya_rows WHERE table_name=?1 ORDER BY id")
        .map_err(backend)?;
    let ids = statement
        .query_map([table], |row| row.get::<_, String>(0))
        .map_err(backend)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(backend)?;
    super::measure(&statement);
    ids.into_iter()
        .map(|id| {
            get(connection, table, &id)?.ok_or_else(|| backend("schema scan row disappeared"))
        })
        .collect()
}

pub(super) fn catalog(connection: &Connection) -> Result<ModelCatalog, StorageError> {
    let orphan = super::query_one::<bool>(
        connection,
        "SELECT EXISTS(SELECT 1 FROM __hoya_rows r LEFT JOIN __hoya_tables t ON t.name=r.table_name WHERE t.name IS NULL) OR EXISTS(SELECT 1 FROM __hoya_fields f LEFT JOIN __hoya_rows r ON r.table_name=f.table_name AND r.id=f.row_id WHERE r.id IS NULL)",
        [],
    )?;
    if orphan {
        return Err(backend("orphan storage bookkeeping"));
    }
    let mut tables = names(connection)?;
    let state = if let Some(index) = tables.iter().position(|name| name == MODEL_TABLE) {
        tables.remove(index);
        Some(model_state_from_rows(&scan(connection, MODEL_TABLE)?)?)
    } else {
        None
    };
    if super::sync::load(connection)?.is_some() {
        tables.retain(|name| name != SYNC_TABLE);
    }
    let catalog = checked_model_catalog(state, tables)?;
    let unknown = super::query_one::<bool>(
        connection,
        "SELECT EXISTS(SELECT 1 FROM __hoya_fields WHERE table_name<>?1 AND table_name<>?2 AND name GLOB ?3 AND (?4 OR name<>?5))",
        params![
            MODEL_TABLE,
            SYNC_TABLE,
            format!("{RESERVED_PREFIX}*"),
            catalog.state.is_none(),
            ROW_VERSION
        ],
    )?;
    if unknown {
        return Err(backend("unknown reserved record bookkeeping"));
    }
    if let Some(state) = &catalog.state {
        let invalid = super::query_one::<bool>(
            connection,
            "SELECT EXISTS(SELECT 1 FROM __hoya_rows r WHERE r.table_name<>?1 AND r.table_name<>?2 AND NOT EXISTS(SELECT 1 FROM __hoya_fields f WHERE f.table_name=r.table_name AND f.row_id=r.id AND f.name=?3 AND f.kind=2 AND f.text_value IS NULL AND typeof(f.int_value)='integer' AND f.int_value=?4))",
            params![
                MODEL_TABLE,
                SYNC_TABLE,
                ROW_VERSION,
                i64::from(state.version)
            ],
        )?;
        if invalid {
            return Err(backend("managed row version differs from schema"));
        }
    }
    Ok(catalog)
}

fn validate_candidate(
    connection: &Connection,
    schema: &Schema,
    version: u32,
) -> Result<(), StorageError> {
    let mut rows = BTreeMap::new();
    for name in names(connection)? {
        if name != MODEL_TABLE && name != SYNC_TABLE {
            rows.insert(name.clone(), scan(connection, &name)?);
        }
    }
    validate_model_inventory(schema, version, &rows)
}

impl SchemaStorage for SqliteStorage {
    fn catalog(&self) -> impl Future<Output = Result<ModelCatalog, StorageError>> + Send {
        ready(catalog(&self.connection))
    }

    fn scan(&self, table: &str) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        ready(scan(&self.connection, table))
    }

    fn apply_schema_plan(
        &mut self,
        plan: SchemaPlan,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(validate_schema_plan(&plan).and_then(|()| {
            self.transaction(|connection| {
                if let Some(rows) = super::sync::load(connection)? {
                    validate_sync_schema_transition(&rows, &plan.next)?;
                }
                if catalog(connection)?.state != plan.expected {
                    return Err(schema_conflict("schema state changed before transition"));
                }
                for operation in plan.operations {
                    match operation {
                        SchemaMutation::CreateTable(name) => {
                            if names(connection)?.contains(&name) {
                                return Err(StorageError::InvalidSchema {
                                    message: "schema table already exists".into(),
                                });
                            }
                            connection
                                .execute("INSERT INTO __hoya_tables(name) VALUES (?1)", [name])
                                .map_err(backend)?;
                        }
                        SchemaMutation::DropTable(name) => {
                            require_table(connection, &name)?;
                            for sql in [
                                "DELETE FROM __hoya_fields WHERE table_name=?1",
                                "DELETE FROM __hoya_rows WHERE table_name=?1",
                            ] {
                                super::written(connection.execute(sql, [&name]).map_err(backend)?);
                            }
                            connection
                                .execute("DELETE FROM __hoya_tables WHERE name=?1", [&name])
                                .map_err(backend)?;
                        }
                        SchemaMutation::Write(mutation) => match mutation {
                            Mutation::Insert {
                                table,
                                row,
                            } => insert(connection, &table, row)?,
                            Mutation::Replace {
                                table,
                                row,
                            } => replace(connection, &table, row)?,
                            Mutation::Remove {
                                table,
                                id,
                            } => {
                                remove(connection, &table, &id)?;
                            }
                        },
                        SchemaMutation::Validate {
                            schema,
                            version,
                        } => validate_candidate(connection, &schema, version)?,
                    }
                }
                validate_candidate(connection, &plan.schema, plan.next.version)?;
                let row = model_state_row(&plan.next)?;
                if plan.expected.is_none() {
                    connection
                        .execute("INSERT INTO __hoya_tables(name) VALUES (?1)", [MODEL_TABLE])
                        .map_err(backend)?;
                    insert(connection, MODEL_TABLE, row)?;
                } else {
                    replace(connection, MODEL_TABLE, row)?;
                }
                Ok(())
            })
        }))
    }
}
