//! API-0003 / TC-0025: memory executes queries with adapter-owned lookup maps.
use super::{SnapshotStorage, UndoGuard};
use crate::storage::*;
use std::cmp::Ordering;
use std::collections::BTreeSet;

fn compare(left: &Value, right: &Value) -> Ordering {
    match (left, right) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,
        _ => left.cmp(right),
    }
}

fn matches(value: Option<Value>, predicate: &Predicate) -> bool {
    let Some(value) = value else {
        return false;
    };
    if predicate.operator == Operator::In {
        return predicate.values.contains(&value);
    }
    let other = &predicate.values[0];
    match predicate.operator {
        Operator::Eq => value == *other,
        Operator::Ne => value != *other,
        _ if value == Value::Null || other == &Value::Null => false,
        Operator::Lt => compare(&value, other) == Ordering::Less,
        Operator::Le => compare(&value, other) != Ordering::Greater,
        Operator::Gt => compare(&value, other) == Ordering::Greater,
        Operator::Ge => compare(&value, other) != Ordering::Less,
        Operator::In => unreachable!(),
    }
}

impl SnapshotStorage {
    pub(super) fn require_table(
        &self,
        table: &str,
    ) -> Result<&std::collections::BTreeMap<String, Row>, StorageError> {
        self.tables
            .get(table)
            .ok_or_else(|| StorageError::TableNotFound {
                table: table.into(),
            })
    }

    pub(super) fn field(&self, row: &Row, field: &ResolvedField) -> Option<Value> {
        let target = if let Some(foreign) = &field.foreign_key {
            let Some(Value::Reference(id)) = row.fields.get(foreign) else {
                return None;
            };
            self.tables.get(&field.table)?.get(id)?
        } else {
            row
        };
        let value = if field.name == "id" {
            Value::String(target.id.clone())
        } else {
            target
                .fields
                .get(&field.name)
                .cloned()
                .unwrap_or(Value::Null)
        };
        // Raw storage can contain fields incompatible with this supplied schema.
        // Such a consulted field matches no predicate and orders as Null.
        (value_has_type(&value, &field.column.kind)
            || value == Value::Null && field.column.optional)
            .then_some(value)
    }

    fn candidates(&self, query: &Query) -> Result<BTreeSet<String>, StorageError> {
        let rows = self.require_table(&query.table)?;
        let mut best: Option<BTreeSet<String>> = None;
        for predicate in &query.predicates {
            if predicate.field.contains('.')
                || !matches!(predicate.operator, Operator::Eq | Operator::In)
                || predicate.values.contains(&Value::Null)
            {
                continue;
            }
            let mut ids = BTreeSet::new();
            if let Some(field) = self
                .indexes
                .get(&query.table)
                .and_then(|table| table.get(&predicate.field))
            {
                for value in &predicate.values {
                    if let Some(found) = field.get(value) {
                        ids.extend(found.iter().cloned());
                    }
                }
            }
            if best.as_ref().is_none_or(|old| ids.len() < old.len()) {
                best = Some(ids);
            }
        }
        Ok(best.unwrap_or_else(|| rows.keys().cloned().collect()))
    }

    /// Concrete diagnostic: candidate rows visited before predicate/order/limit work.
    pub fn candidate_count(&self, schema: &Schema, query: &Query) -> Result<usize, StorageError> {
        validate_query(schema, query)?;
        Ok(self.candidates(query)?.len())
    }
}

impl SnapshotStorage {
    pub(super) fn select_rows<'a>(
        &'a self,
        schema: &Schema,
        query: &Query,
    ) -> Result<Vec<&'a Row>, StorageError> {
        validate_query(schema, query)?;
        let source = self.require_table(&query.table)?;
        let fields: Vec<_> = query
            .predicates
            .iter()
            .map(|p| resolve_field(schema, &query.table, &p.field))
            .collect::<Result<_, _>>()?;
        let order = query
            .order
            .as_ref()
            .map(|order| resolve_field(schema, &query.table, &order.field))
            .transpose()?;
        for field in fields.iter().chain(order.iter()) {
            self.require_table(&field.table)?;
        }
        if query.limit == Some(0) {
            return Ok(Vec::new());
        }
        let ids = self.candidates(query)?;
        count_work!(memory_rows_visited, ids.len() as u64);
        let mut selected: Vec<&Row> = ids
            .iter()
            .filter_map(|id| source.get(id))
            .filter(|row| {
                query
                    .predicates
                    .iter()
                    .zip(&fields)
                    .all(|(predicate, field)| matches(self.field(row, field), predicate))
            })
            .collect();
        if let Some(field) = order {
            selected.sort_by(|left, right| {
                let comparison = compare(
                    &self.field(left, &field).unwrap_or(Value::Null),
                    &self.field(right, &field).unwrap_or(Value::Null),
                );
                (if query.order.as_ref().unwrap().direction == Direction::Desc {
                    comparison.reverse()
                } else {
                    comparison
                })
                .then_with(|| left.id.cmp(&right.id))
            });
        }
        if let Some(limit) = query.limit {
            selected.truncate(limit);
        }
        Ok(selected)
    }
}

impl TypedStorage for SnapshotStorage {
    async fn query(&self, schema: &Schema, query: &Query) -> Result<Vec<Row>, StorageError> {
        self.select_rows(schema, query)?
            .into_iter()
            .map(|row| {
                validate_row(schema, &query.table, row)?;
                Ok(row.clone())
            })
            .collect()
    }

    async fn query_validated(&self, proof: ValidatedQuery<'_>) -> Result<Vec<Row>, StorageError> {
        let (schema, query) = (proof.schema(), proof.query());
        match self.ordered_walk(schema, query)? {
            Some(rows) => Ok(rows),
            None => self.query(schema, query).await,
        }
    }
}

impl SnapshotStorage {
    /// SPEC-0037 R8.2: a finite, predicate-free limit ordered by a direct
    /// non-optional column walks that field's index in value order (reversed
    /// for Desc, ids ascending within a value) and stops at k. An unordered
    /// finite limit takes the first k rows in id order. Only a validated
    /// Exclusive owner reaches this, so every row holds a correctly kinded value.
    /// `None` means the shape is ineligible and today's query applies.
    fn ordered_walk(
        &self,
        schema: &Schema,
        query: &Query,
    ) -> Result<Option<Vec<Row>>, StorageError> {
        let (Some(limit), true) = (query.limit, query.predicates.is_empty()) else {
            return Ok(None);
        };
        let Some(order) = query.order.as_ref() else {
            // Unordered results are in id order: take the first k rows.
            validate_query(schema, query)?;
            let source = self.require_table(&query.table)?;
            let mut rows = Vec::with_capacity(limit.min(source.len()));
            for row in source.values().take(limit) {
                count_work!(memory_rows_visited, 1);
                validate_row(schema, &query.table, row)?;
                rows.push(row.clone());
            }
            return Ok(Some(rows));
        };
        validate_query(schema, query)?;
        let field = resolve_field(schema, &query.table, &order.field)?;
        if field.foreign_key.is_some() || field.name == "id" || field.column.optional {
            return Ok(None);
        }
        let source = self.require_table(&query.table)?;
        let mut rows = Vec::with_capacity(limit.min(source.len()));
        if limit == 0 {
            return Ok(Some(rows));
        }
        let Some(index) = self
            .indexes
            .get(&query.table)
            .and_then(|table| table.get(&field.name))
        else {
            return Ok(Some(rows));
        };
        count_work!(ordered_ops, 1);
        let groups: Box<dyn Iterator<Item = &BTreeSet<String>>> = match order.direction {
            Direction::Asc => Box::new(index.values()),
            Direction::Desc => Box::new(index.values().rev()),
        };
        'walk: for ids in groups {
            count_work!(ordered_ops, 1);
            for id in ids {
                let row = source.get(id).ok_or_else(|| StorageError::BackendFailure {
                    message: "index entry without a row".into(),
                })?;
                count_work!(memory_rows_visited, 1);
                validate_row(schema, &query.table, row)?;
                rows.push(row.clone());
                if rows.len() == limit {
                    break 'walk;
                }
            }
        }
        Ok(Some(rows))
    }
}

impl TypedTransactionalStorage for SnapshotStorage {
    async fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        mutations: Vec<Mutation>,
    ) -> Result<Vec<CommittedChange>, StorageError> {
        validate_schema(schema)?;
        let mut guard = UndoGuard::new(self);
        let changes = typed_batch(&mut guard, schema, mutations)?;
        guard.commit();
        Ok(changes)
    }
}

fn image(guard: &UndoGuard<'_>, table: &str, id: &str) -> Result<Option<Row>, StorageError> {
    let row = guard.store().require_table(table)?.get(id).cloned();
    if row.is_some() {
        count_work!(rows_cloned, 1);
    }
    Ok(row)
}

/// SPEC-0037 R7: typed mutations applied in place under the caller's guard.
pub(super) fn typed_batch(
    guard: &mut UndoGuard<'_>,
    schema: &Schema,
    mutations: Vec<Mutation>,
) -> Result<Vec<CommittedChange>, StorageError> {
    let mut changes = Vec::new();
    for mutation in mutations {
        let (table, id) = match &mutation {
            Mutation::Insert {
                table,
                row,
            }
            | Mutation::Replace {
                table,
                row,
            } => (table.clone(), row.id.clone()),
            Mutation::Remove {
                table,
                id,
            } => (table.clone(), id.clone()),
        };
        if !schema.tables.contains_key(&table) {
            return Err(StorageError::TableNotFound {
                table,
            });
        }
        let before = image(guard, &table, &id)?;
        match mutation {
            Mutation::Insert {
                row,
                ..
            } => {
                validate_row(schema, &table, &row)?;
                guard.insert(&table, row)?;
            }
            Mutation::Replace {
                row,
                ..
            } => {
                validate_row(schema, &table, &row)?;
                if let Some(previous) = &before {
                    validate_row(schema, &table, previous)?;
                }
                if before.as_ref() != Some(&row) {
                    guard.replace(&table, row)?;
                }
            }
            Mutation::Remove {
                ..
            } => {
                if let Some(previous) = &before {
                    validate_row(schema, &table, previous)?;
                }
                guard.remove(&table, &id)?;
            }
        }
        let after = image(guard, &table, &id)?;
        if before != after {
            changes.push(CommittedChange {
                table,
                id,
                before,
                after,
            });
        }
    }
    Ok(changes)
}
