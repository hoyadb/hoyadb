//! SPEC-0026 / API-0004: schema-owned public writes; no raw engine escape while open.
use super::{Migration, QueryBuilder, RESERVED_PREFIX, ROW_VERSION, SchemaDefinition};
use crate::storage::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

pub(crate) type IdGenerator = Arc<Mutex<Box<dyn FnMut() -> String + Send>>>;

pub struct Database<S> {
    pub(crate) storage: S,
    pub(crate) schema: SchemaDefinition,
    pub(crate) projection: Schema,
    pub(crate) state: ModelState,
    pub(crate) generator: Option<IdGenerator>,
    pub(crate) journal: Option<Vec<CommittedChange>>,
    /// SPEC-0037 R8.0: set only by `CommandDatabase::open` after R5 validation
    /// of an `Exclusive` store; never a public flag.
    pub(crate) validated_exclusive: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Write {
    Insert {
        table: String,
        fields: BTreeMap<String, Value>,
    },
    Replace {
        table: String,
        id: String,
        fields: BTreeMap<String, Value>,
    },
    Remove {
        table: String,
        id: String,
    },
}

impl<S: SchemaStorage> Database<S> {
    pub async fn open(
        storage: S,
        schema: SchemaDefinition,
        migrations: Vec<Migration>,
    ) -> Result<Self, StorageError> {
        Self::open_recoverable(storage, schema, migrations)
            .await
            .map_err(|(error, _)| error)
    }

    pub async fn open_recoverable(
        mut storage: S,
        schema: SchemaDefinition,
        migrations: Vec<Migration>,
    ) -> Result<Self, (StorageError, S)> {
        let prepared = async {
            let schema = schema.normalized()?;
            let projection = schema.storage_schema()?;
            let state = super::migration::open_schema(&mut storage, &schema, &migrations).await?;
            Ok::<_, StorageError>((schema, projection, state))
        }
        .await;
        match prepared {
            Ok((schema, projection, state)) => Ok(Self {
                storage,
                schema,
                projection,
                state,
                generator: None,
                journal: None,
                validated_exclusive: false,
            }),
            Err(error) => Err((error, storage)),
        }
    }

    pub fn with_id_generator(mut self, generator: impl FnMut() -> String + Send + 'static) -> Self {
        self.generator = Some(Arc::new(Mutex::new(Box::new(generator))));
        self
    }

    pub fn schema(&self) -> &SchemaDefinition {
        &self.schema
    }

    pub fn into_storage(self) -> S {
        self.storage
    }

    pub async fn insert(
        &mut self,
        table: &str,
        fields: BTreeMap<String, Value>,
    ) -> Result<Row, StorageError> {
        let changes = self
            .apply_batch(vec![Write::Insert {
                table: table.into(),
                fields,
            }])
            .await?;
        changes
            .into_iter()
            .next()
            .and_then(|change| change.after)
            .ok_or_else(|| StorageError::BackendFailure {
                message: "successful insertion has no committed row".into(),
            })
    }

    pub async fn insert_proposed(
        &mut self,
        table: &str,
        fields: BTreeMap<String, Value>,
        proposed_id: Option<String>,
    ) -> Result<Row, StorageError> {
        self.apply_batch_proposed(vec![(
            Write::Insert {
                table: table.into(),
                fields,
            },
            proposed_id,
        )])
        .await?
        .into_iter()
        .next()
        .and_then(|change| change.after)
        .ok_or_else(|| StorageError::BackendFailure {
            message: "successful insertion has no committed row".into(),
        })
    }

    pub async fn replace(
        &mut self,
        table: &str,
        id: &str,
        fields: BTreeMap<String, Value>,
    ) -> Result<Row, StorageError> {
        self.apply_batch(vec![Write::Replace {
            table: table.into(),
            id: id.into(),
            fields: fields.clone(),
        }])
        .await?;
        Ok(Row {
            id: id.into(),
            fields,
        })
    }

    pub async fn remove(&mut self, table: &str, id: &str) -> Result<bool, StorageError> {
        Ok(!self
            .apply_batch(vec![Write::Remove {
                table: table.into(),
                id: id.into(),
            }])
            .await?
            .is_empty())
    }

    pub async fn apply_batch(
        &mut self,
        writes: Vec<Write>,
    ) -> Result<Vec<CommittedChange>, StorageError> {
        self.apply_batch_proposed(writes.into_iter().map(|write| (write, None)).collect())
            .await
    }

    /// Apply the same atomic write path with optional insertion IDs supplied by a host.
    pub async fn apply_batch_proposed(
        &mut self,
        writes: Vec<(Write, Option<String>)>,
    ) -> Result<Vec<CommittedChange>, StorageError> {
        let mut state = self.state.clone();
        let mut mutations = Vec::new();
        let mut allocated = BTreeSet::new();
        for (write, proposed_id) in writes {
            let table = match &write {
                Write::Insert {
                    table,
                    ..
                }
                | Write::Replace {
                    table,
                    ..
                }
                | Write::Remove {
                    table,
                    ..
                } => table,
            };
            if proposed_id.is_some() && !matches!(&write, Write::Insert { .. }) {
                return Err(StorageError::InvalidRow {
                    table: table.clone(),
                    id: String::new(),
                    message: "ID proposals require an insertion".into(),
                });
            }
            if !self.schema.tables.contains_key(table) {
                return Err(StorageError::TableNotFound {
                    table: table.clone(),
                });
            }
            let (table, id, fields, is_insert) = match write {
                Write::Insert {
                    table,
                    fields,
                } => {
                    let id = if let Some(id) = proposed_id {
                        id
                    } else if let Some(generator) = &mut self.generator {
                        generator
                            .lock()
                            .map_err(|_| StorageError::BackendFailure {
                                message: "ID generator is unavailable".into(),
                            })?
                            .as_mut()()
                    } else {
                        loop {
                            let id = format!("local:{}", state.next_id);
                            state.next_id = state
                                .next_id
                                .checked_add(1)
                                .ok_or_else(|| schema_conflict("ID counter exhausted"))?;
                            if !allocated.contains(&(table.clone(), id.clone()))
                                && self.storage.get(&table, &id).await?.is_none()
                            {
                                break id;
                            }
                        }
                    };
                    if id.is_empty() {
                        return Err(StorageError::InvalidRow {
                            table,
                            id,
                            message: "generated ID must be nonempty".into(),
                        });
                    }
                    allocated.insert((table.clone(), id.clone()));
                    (table, id, Some(fields), true)
                }
                Write::Replace {
                    table,
                    id,
                    fields,
                } => (table, id, Some(fields), false),
                Write::Remove {
                    table,
                    id,
                } => (table, id, None, false),
            };
            if !is_insert && let Some(row) = self.storage.get(&table, &id).await? {
                public_row(row, self.state.version)?;
            }
            let mutation = if let Some(mut fields) = fields {
                if fields
                    .keys()
                    .any(|name| name == "id" || name.starts_with(RESERVED_PREFIX))
                {
                    return Err(StorageError::InvalidRow {
                        table,
                        id,
                        message: "application fields cannot set managed names".into(),
                    });
                }
                fields.insert(
                    ROW_VERSION.into(),
                    Value::Integer(i64::from(self.schema.version)),
                );
                let row = Row {
                    id,
                    fields,
                };
                if is_insert {
                    Mutation::Insert {
                        table,
                        row,
                    }
                } else {
                    Mutation::Replace {
                        table,
                        row,
                    }
                }
            } else {
                Mutation::Remove {
                    table,
                    id,
                }
            };
            mutations.push(mutation);
        }
        if mutations.is_empty() {
            return Ok(vec![]);
        }
        if state != self.state {
            mutations.push(Mutation::Replace {
                table: MODEL_TABLE.into(),
                row: model_state_row(&state)?,
            });
        }
        let mut projection = self.projection.clone();
        projection
            .tables
            .insert(MODEL_TABLE.into(), model_metadata_schema());
        let changes = self
            .storage
            .apply_typed_batch(&projection, mutations)
            .await?;
        self.state = state;
        let changes: Vec<CommittedChange> = changes
            .into_iter()
            .filter(|change| change.table != MODEL_TABLE)
            .map(|mut change| {
                change.before = change
                    .before
                    .map(|row| public_row(row, self.schema.version))
                    .transpose()?;
                change.after = change
                    .after
                    .map(|row| public_row(row, self.schema.version))
                    .transpose()?;
                Ok(change)
            })
            .collect::<Result<_, StorageError>>()?;
        if let Some(journal) = &mut self.journal {
            journal.extend(changes.iter().cloned());
        }
        Ok(changes)
    }

    pub fn query(&self, table: &str) -> QueryBuilder {
        QueryBuilder::new(table, None)
    }

    pub fn related(&self, table: &str, id: &str, relation: &str) -> QueryBuilder {
        QueryBuilder::new("", Some((table.into(), id.into(), relation.into())))
    }

    pub(crate) fn check_public_query(&self, query: &Query) -> Result<(), StorageError> {
        if !self.schema.tables.contains_key(&query.table) {
            return Err(StorageError::TableNotFound {
                table: query.table.clone(),
            });
        }
        for path in query
            .predicates
            .iter()
            .map(|p| p.field.as_str())
            .chain(query.order.iter().map(|order| order.field.as_str()))
        {
            if path
                .split('.')
                .any(|name| name.starts_with(RESERVED_PREFIX))
            {
                return Err(StorageError::InvalidQuery {
                    message: "managed fields are not public query fields".into(),
                });
            }
        }
        validate_query(&self.projection, query)
    }
}

pub(crate) fn public_row(mut row: Row, version: u32) -> Result<Row, StorageError> {
    if row.fields.remove(ROW_VERSION) != Some(Value::Integer(i64::from(version))) {
        return Err(StorageError::BackendFailure {
            message: "managed row version differs from open schema".into(),
        });
    }
    Ok(row)
}
