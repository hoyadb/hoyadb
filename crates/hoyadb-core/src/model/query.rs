//! SPEC-0026 / API-0004: owned lazy query plans, resolved only on fetch.
use super::database::public_row;
use super::{Database, RelationDefinition, Write};
use crate::storage::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryBuilder {
    pub(crate) query: Query,
    pub(crate) navigation: Option<(String, String, String)>,
}

impl QueryBuilder {
    pub(crate) fn new(table: &str, navigation: Option<(String, String, String)>) -> Self {
        Self {
            query: Query {
                table: table.into(),
                predicates: vec![],
                order: None,
                limit: None,
            },
            navigation,
        }
    }

    pub fn where_(mut self, field: &str, operator: Operator, value: Value) -> Self {
        self.query.predicates.push(Predicate {
            field: field.into(),
            operator,
            values: vec![value],
        });
        self
    }

    pub fn where_in(mut self, field: &str, values: Vec<Value>) -> Self {
        self.query.predicates.push(Predicate {
            field: field.into(),
            operator: Operator::In,
            values,
        });
        self
    }

    pub fn order_by(mut self, field: &str, direction: Direction) -> Self {
        self.query.order = Some(OrderBy {
            field: field.into(),
            direction,
        });
        self
    }

    pub fn limit(mut self, count: usize) -> Self {
        self.query.limit = Some(count);
        self
    }

    pub async fn fetch<S: SchemaStorage>(
        &self,
        db: &Database<S>,
    ) -> Result<Vec<Row>, StorageError> {
        db.fetch(self).await
    }
}

fn predicate(field: &str, value: Value) -> Predicate {
    Predicate {
        field: field.into(),
        operator: Operator::Eq,
        values: vec![value],
    }
}

fn row_query(table: &str, id: &str) -> Query {
    Query {
        table: table.into(),
        predicates: vec![predicate("id", Value::String(id.into()))],
        order: None,
        limit: None,
    }
}

struct Through {
    target: String,
    table: String,
    source_key: String,
    target_key: String,
}

impl<S: SchemaStorage> Database<S> {
    fn relation(&self, table: &str, name: &str) -> Result<&RelationDefinition, StorageError> {
        self.schema
            .tables
            .get(table)
            .ok_or_else(|| StorageError::TableNotFound {
                table: table.into(),
            })?
            .relations
            .get(name)
            .ok_or_else(|| StorageError::InvalidQuery {
                message: "undeclared relation".into(),
            })
    }

    fn through(&self, table: &str, name: &str) -> Result<Through, StorageError> {
        let RelationDefinition::ManyToMany {
            table: target,
            through,
            source_relation,
            target_relation,
        } = self.relation(table, name)?
        else {
            return Err(StorageError::InvalidQuery {
                message: "link operations require a many-to-many relation".into(),
            });
        };
        let declarations = &self.schema.tables[through].relations;
        let key = |relation: &str| match &declarations[relation] {
            RelationDefinition::ToOne {
                foreign_key,
                ..
            } => foreign_key.clone(),
            _ => unreachable!("validated through relation"),
        };
        Ok(Through {
            target: target.clone(),
            table: through.clone(),
            source_key: key(source_relation),
            target_key: key(target_relation),
        })
    }

    async fn source_row(&self, table: &str, id: &str) -> Result<Option<Row>, StorageError> {
        self.storage
            .query(&self.projection, &row_query(table, id))
            .await?
            .into_iter()
            .next()
            .map(|row| public_row(row, self.schema.version))
            .transpose()
    }

    pub async fn fetch(&self, builder: &QueryBuilder) -> Result<Vec<Row>, StorageError> {
        let mut query = builder.query.clone();
        if let Some((source, id, name)) = &builder.navigation {
            let relation = self.relation(source, name)?.clone();
            query.table = match &relation {
                RelationDefinition::ToOne {
                    table,
                    ..
                }
                | RelationDefinition::HasMany {
                    table,
                    ..
                }
                | RelationDefinition::ManyToMany {
                    table,
                    ..
                } => table.clone(),
            };
            self.check_public_query(&query)?;
            let Some(source_row) = self.source_row(source, id).await? else {
                return Ok(vec![]);
            };
            match relation {
                RelationDefinition::ToOne {
                    foreign_key,
                    ..
                } => {
                    let Some(Value::Reference(target)) = source_row.fields.get(&foreign_key) else {
                        return Ok(vec![]);
                    };
                    query
                        .predicates
                        .push(predicate("id", Value::String(target.clone())));
                }
                RelationDefinition::HasMany {
                    foreign_key,
                    ..
                } => query
                    .predicates
                    .push(predicate(&foreign_key, Value::Reference(id.clone()))),
                RelationDefinition::ManyToMany {
                    ..
                } => {
                    let through = self.through(source, name)?;
                    let memberships = self
                        .storage
                        .query(
                            &self.projection,
                            &Query {
                                table: through.table,
                                predicates: vec![predicate(
                                    &through.source_key,
                                    Value::Reference(id.clone()),
                                )],
                                order: None,
                                limit: None,
                            },
                        )
                        .await?;
                    let ids: BTreeSet<_> = memberships
                        .into_iter()
                        .filter_map(|row| match row.fields.get(&through.target_key) {
                            Some(Value::Reference(id)) => Some(id.clone()),
                            _ => None,
                        })
                        .collect();
                    query.predicates.push(Predicate {
                        field: "id".into(),
                        operator: Operator::In,
                        values: ids.into_iter().map(Value::String).collect(),
                    });
                }
            }
        } else {
            self.check_public_query(&query)?;
        }
        let rows = if self.validated_exclusive {
            // SPEC-0037 R8.0: the only place a proof is constructed.
            self.storage
                .query_validated(ValidatedQuery::new(&self.projection, &query))
                .await?
        } else {
            self.storage.query(&self.projection, &query).await?
        };
        rows.into_iter()
            .map(|row| public_row(row, self.schema.version))
            .collect()
    }

    async fn memberships(
        &self,
        source: &str,
        id: &str,
        name: &str,
        target: &str,
    ) -> Result<(Through, Vec<Row>), StorageError> {
        let through = self.through(source, name)?;
        for (table, id) in [(source, id), (through.target.as_str(), target)] {
            if self.source_row(table, id).await?.is_none() {
                return Err(StorageError::RowNotFound {
                    table: table.into(),
                    id: id.into(),
                });
            }
        }
        let rows = self
            .storage
            .query(
                &self.projection,
                &Query {
                    table: through.table.clone(),
                    predicates: vec![
                        predicate(&through.source_key, Value::Reference(id.into())),
                        predicate(&through.target_key, Value::Reference(target.into())),
                    ],
                    order: None,
                    limit: None,
                },
            )
            .await?;
        Ok((through, rows))
    }

    pub async fn link(
        &mut self,
        table: &str,
        id: &str,
        relation: &str,
        target: &str,
        fields: BTreeMap<String, Value>,
    ) -> Result<Row, StorageError> {
        self.link_proposed(table, id, relation, target, fields, None)
            .await
    }

    pub async fn link_proposed(
        &mut self,
        table: &str,
        id: &str,
        relation: &str,
        target: &str,
        fields: BTreeMap<String, Value>,
        proposed_id: Option<String>,
    ) -> Result<Row, StorageError> {
        let (through, rows) = self.memberships(table, id, relation, target).await?;
        if fields.keys().any(|key| {
            key == &through.source_key
                || key == &through.target_key
                || key == "id"
                || key.starts_with(super::RESERVED_PREFIX)
        }) {
            return Err(StorageError::InvalidRow {
                table: through.table,
                id: String::new(),
                message: "link payload cannot set managed through keys".into(),
            });
        }
        if let Some(row) = rows.into_iter().next() {
            let row = public_row(row, self.schema.version)?;
            let mut payload = row.fields.clone();
            payload.remove(&through.source_key);
            payload.remove(&through.target_key);
            if payload != fields {
                return Err(StorageError::InvalidRow {
                    table: through.table,
                    id: row.id,
                    message: "existing link has different payload".into(),
                });
            }
            return Ok(row);
        }
        let mut fields = fields;
        fields.insert(through.source_key, Value::Reference(id.into()));
        fields.insert(through.target_key, Value::Reference(target.into()));
        self.insert_proposed(&through.table, fields, proposed_id)
            .await
    }

    pub async fn unlink(
        &mut self,
        table: &str,
        id: &str,
        relation: &str,
        target: &str,
    ) -> Result<usize, StorageError> {
        let (through, rows) = self.memberships(table, id, relation, target).await?;
        let count = rows.len();
        self.apply_batch(
            rows.into_iter()
                .map(|row| Write::Remove {
                    table: through.table.clone(),
                    id: row.id,
                })
                .collect(),
        )
        .await?;
        Ok(count)
    }
}
