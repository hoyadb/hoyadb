//! SPEC-0037 R6: delta-overlay staging. A command scope reads its base store
//! plus the scope's own delta and writes the delta only, so staging copies and
//! validates nothing it does not touch. The base is either the owner's store
//! (Exclusive, validated at open) or the validated snapshot copy taken at begin
//! (Unproven); in both cases it is immutable for the scope's life.
use crate::storage::*;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    future::{Future, ready},
    task::{Context, Poll, Waker},
};

type Key = (String, String);

/// The scope's staged changes: `None` is a staged removal.
#[derive(Default)]
pub(crate) struct Delta {
    pub(crate) rows: BTreeMap<Key, Option<Row>>,
    /// The base image of each staged key, recorded when first written.
    pub(crate) first_images: BTreeMap<Key, Option<Row>>,
}

pub(crate) enum Base<'a, S> {
    Store(&'a S),
    Snapshot(&'a SnapshotStorage),
}

pub(crate) enum DeltaRef<'a> {
    Read(&'a Delta),
    Write(&'a mut Delta),
}

pub(crate) struct Overlay<'a, S> {
    pub(crate) base: Base<'a, S>,
    pub(crate) delta: DeltaRef<'a>,
    /// SPEC-0037 R8.0: the base belongs to a validated Exclusive owner.
    pub(crate) validated: bool,
}

fn failure(message: &str) -> StorageError {
    StorageError::BackendFailure {
        message: message.into(),
    }
}

/// Base reads of both supported adapters resolve on their first poll.
fn now<T>(future: impl Future<Output = Result<T, StorageError>>) -> Result<T, StorageError> {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(result) => result,
        Poll::Pending => Err(failure("staged base read unexpectedly suspended")),
    }
}

fn ids_query(table: &str, ids: impl IntoIterator<Item = String>) -> Query {
    Query {
        table: table.into(),
        predicates: vec![Predicate {
            field: "id".into(),
            operator: Operator::In,
            values: ids.into_iter().map(Value::String).collect(),
        }],
        order: None,
        limit: None,
    }
}

/// API-0003 order: Null lowest, reversed for Desc, then ascending id.
fn compare(left: &Value, right: &Value) -> Ordering {
    match (left, right) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,
        _ => left.cmp(right),
    }
}

impl<S: SyncStorage> Base<'_, S> {
    fn get(&self, table: &str, id: &str) -> Result<Option<Row>, StorageError> {
        match self {
            Self::Store(store) => now(store.get(table, id)),
            Self::Snapshot(store) => now(store.get(table, id)),
        }
    }

    fn query(&self, schema: &Schema, query: &Query) -> Result<Vec<Row>, StorageError> {
        match self {
            Self::Store(store) => now(store.query(schema, query)),
            Self::Snapshot(store) => now(store.query(schema, query)),
        }
    }

    fn query_validated(&self, schema: &Schema, query: &Query) -> Result<Vec<Row>, StorageError> {
        match self {
            Self::Store(store) => now(store.query_validated(ValidatedQuery::new(schema, query))),
            Self::Snapshot(_) => Err(failure("a snapshot base is never validated")),
        }
    }

    fn project(&self, schema: &Schema, query: &Query) -> Result<Vec<ProjectedRow>, StorageError> {
        match self {
            Self::Store(store) => now(store.project(schema, query, &[])),
            Self::Snapshot(store) => now(store.project(schema, query, &[])),
        }
    }

    fn catalog(&self) -> Result<ModelCatalog, StorageError> {
        match self {
            Self::Store(store) => now(store.catalog()),
            Self::Snapshot(store) => now(store.catalog()),
        }
    }

    fn scan(&self, table: &str) -> Result<Vec<Row>, StorageError> {
        match self {
            Self::Store(store) => now(store.scan(table)),
            Self::Snapshot(store) => now(store.scan(table)),
        }
    }
}

impl DeltaRef<'_> {
    fn read(&self) -> &Delta {
        match self {
            Self::Read(delta) => delta,
            Self::Write(delta) => delta,
        }
    }
}

impl<S: SyncStorage> Overlay<'_, S> {
    /// The staged image if this key is staged, otherwise the base image.
    fn image(&self, table: &str, id: &str) -> Result<Option<Row>, StorageError> {
        match self
            .delta
            .read()
            .rows
            .get(&(table.to_owned(), id.to_owned()))
        {
            Some(staged) => Ok(staged.clone()),
            None => self.base.get(table, id),
        }
    }

    fn write(
        &mut self,
        schema: Option<&Schema>,
        mutations: Vec<Mutation>,
    ) -> Result<Vec<CommittedChange>, StorageError> {
        if let Some(schema) = schema {
            validate_schema(schema)?;
        }
        let mut undo: Vec<(Key, Option<Option<Row>>, bool)> = Vec::new();
        let result = self.write_all(schema, mutations, &mut undo);
        if result.is_err() {
            let DeltaRef::Write(delta) = &mut self.delta else {
                unreachable!("writes require a writable delta");
            };
            for (key, previous, recorded_first) in undo.into_iter().rev() {
                if recorded_first {
                    delta.first_images.remove(&key);
                }
                match previous {
                    Some(previous) => delta.rows.insert(key, previous),
                    None => delta.rows.remove(&key),
                };
            }
        }
        result
    }

    fn write_all(
        &mut self,
        schema: Option<&Schema>,
        mutations: Vec<Mutation>,
        undo: &mut Vec<(Key, Option<Option<Row>>, bool)>,
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
            if let Some(schema) = schema
                && !schema.tables.contains_key(&table)
            {
                return Err(StorageError::TableNotFound {
                    table,
                });
            }
            let before = self.image(&table, &id)?;
            let validate = |row: &Row| match schema {
                Some(schema) => validate_row(schema, &table, row),
                None => Ok(()),
            };
            let staged = match mutation {
                Mutation::Insert {
                    row,
                    ..
                } => {
                    validate(&row)?;
                    if before.is_some() {
                        return Err(StorageError::RowAlreadyExists {
                            table,
                            id,
                        });
                    }
                    Some(Some(row))
                }
                Mutation::Replace {
                    row,
                    ..
                } => {
                    validate(&row)?;
                    let Some(previous) = &before else {
                        return Err(StorageError::RowNotFound {
                            table,
                            id,
                        });
                    };
                    validate(previous)?;
                    (previous != &row).then_some(Some(row))
                }
                Mutation::Remove {
                    ..
                } => {
                    if let Some(previous) = &before {
                        validate(previous)?;
                    }
                    before.is_some().then_some(None)
                }
            };
            let Some(image) = staged else {
                continue;
            };
            let DeltaRef::Write(delta) = &mut self.delta else {
                return Err(failure("a read-only scope view cannot write"));
            };
            let key = (table.clone(), id.clone());
            let recorded_first = !delta.rows.contains_key(&key);
            if recorded_first {
                if before.is_some() {
                    count_work!(rows_cloned, 1);
                }
                delta.first_images.insert(key.clone(), before.clone());
            }
            if image.is_some() {
                count_work!(rows_cloned, 1);
            }
            let previous = delta.rows.insert(key.clone(), image.clone());
            undo.push((key, previous, recorded_first));
            if before != image {
                changes.push(CommittedChange {
                    table,
                    id,
                    before,
                    after: image,
                });
            }
        }
        Ok(changes)
    }

    /// SPEC-0037 R6.4: an exact query over base plus delta that reads only the
    /// base window, the staged rows and the base rows whose single-hop ToOne
    /// target is staged.
    fn overlay_query(&self, schema: &Schema, query: &Query) -> Result<Vec<Row>, StorageError> {
        validate_query(schema, query)?;
        if query.limit == Some(0) {
            return Ok(Vec::new());
        }
        let table = query.table.as_str();
        let delta = self.delta.read();
        let staged_ids = |name: &str| {
            delta
                .rows
                .keys()
                .filter(|(staged, _)| staged == name)
                .map(|(_, id)| id.clone())
                .collect::<BTreeSet<_>>()
        };
        // D, then R: base rows whose ToOne key in P or O names a staged target.
        let mut affected = staged_ids(table);
        let mut paths = Vec::new();
        for path in query
            .predicates
            .iter()
            .map(|predicate| &predicate.field)
            .chain(query.order.iter().map(|order| &order.field))
        {
            let resolved = resolve_field(schema, table, path)?;
            if let Some(foreign_key) = resolved.foreign_key.clone() {
                let targets = staged_ids(&resolved.table);
                if !targets.is_empty() {
                    let referencing = Query {
                        table: table.into(),
                        predicates: vec![Predicate {
                            field: foreign_key.clone(),
                            operator: Operator::In,
                            values: targets.into_iter().map(Value::Reference).collect(),
                        }],
                        order: None,
                        limit: None,
                    };
                    affected.extend(
                        self.base
                            .project(schema, &referencing)?
                            .into_iter()
                            .map(|row| row.id),
                    );
                }
                paths.push((foreign_key, resolved.table));
            }
        }
        // B: the base window widened by |D ∪ R|, without D ∪ R.
        let window = Query {
            limit: query
                .limit
                .map(|limit| limit.saturating_add(affected.len())),
            ..query.clone()
        };
        let order = query
            .order
            .as_ref()
            .map(|order| resolve_field(schema, table, &order.field))
            .transpose()?;
        let direct_required = order.as_ref().is_some_and(|field| {
            field.foreign_key.is_none() && field.name != "id" && !field.column.optional
        });
        let mut base_rows = BTreeMap::new();
        let mut entries: Vec<(Value, String, bool)> = Vec::new();
        if self.validated
            && query.predicates.is_empty()
            && window.limit.is_some()
            && direct_required
        {
            // A predicate-free bounded base window uses the validated ordered path.
            let field = order.as_ref().expect("direct order");
            for row in self.base.query_validated(schema, &window)? {
                if affected.contains(&row.id) {
                    continue;
                }
                let key = row.fields.get(&field.name).cloned().unwrap_or(Value::Null);
                entries.push((key, row.id.clone(), false));
                base_rows.insert(row.id.clone(), row);
            }
        } else {
            for projected in self.base.project(schema, &window)? {
                if affected.contains(&projected.id) {
                    continue;
                }
                let key = projected
                    .order_key
                    .unwrap_or_else(|| Value::String(projected.id.clone()));
                entries.push((key, projected.id, false));
            }
        }
        // C: P and O over the overlay images of D ∪ R and their overlay targets.
        let mut overlay = SnapshotStorage::default();
        now(overlay.create_table(table))?;
        for (_, target) in &paths {
            now(overlay.create_table(target))?;
        }
        let mut images = BTreeMap::new();
        for id in &affected {
            if let Some(row) = self.image(table, id)? {
                images.insert(id.clone(), row.clone());
                now(overlay.insert(table, row))?;
            }
        }
        // Targets go in after every candidate: a self-referential ToOne shares the
        // candidate table, where a target may itself be a candidate already present.
        for row in images.values() {
            for (foreign_key, target) in &paths {
                if let Some(Value::Reference(target_id)) = row.fields.get(foreign_key)
                    && now(overlay.get(target, target_id))?.is_none()
                    && let Some(target_row) = self.image(target, target_id)?
                {
                    now(overlay.insert(target, target_row))?;
                }
            }
        }
        if !images.is_empty() {
            // Unlimited: a target-only row sharing the candidate table must neither take a
            // limit slot nor be returned; the merged entries are truncated below.
            let unlimited = Query {
                limit: None,
                ..query.clone()
            };
            for projected in now(overlay.project(schema, &unlimited, &[]))? {
                if !images.contains_key(&projected.id) {
                    continue;
                }
                let key = projected
                    .order_key
                    .unwrap_or_else(|| Value::String(projected.id.clone()));
                entries.push((key, projected.id, true));
            }
        }
        // Merge by the Rust comparator; adapter order is not trusted.
        let descending = query
            .order
            .as_ref()
            .is_some_and(|order| order.direction == Direction::Desc);
        entries.sort_by(|(left, left_id, _), (right, right_id, _)| {
            let ordering = compare(left, right);
            (if descending {
                ordering.reverse()
            } else {
                ordering
            })
            .then_with(|| left_id.cmp(right_id))
        });
        entries.truncate(query.limit.unwrap_or(usize::MAX));
        let missing: Vec<String> = entries
            .iter()
            .filter(|(_, id, overlaid)| !overlaid && !base_rows.contains_key(id))
            .map(|(_, id, _)| id.clone())
            .collect();
        if !missing.is_empty() {
            for row in self.base.query(schema, &ids_query(table, missing))? {
                base_rows.insert(row.id.clone(), row);
            }
        }
        let mut result = Vec::with_capacity(entries.len());
        for (_, id, overlaid) in entries {
            let row = if overlaid {
                let row = images
                    .remove(&id)
                    .ok_or_else(|| failure("overlay row disappeared"))?;
                validate_row(schema, table, &row)?;
                row
            } else {
                base_rows
                    .remove(&id)
                    .ok_or_else(|| failure("selected base row disappeared"))?
            };
            count_work!(rows_cloned, 1);
            result.push(row);
        }
        Ok(result)
    }
}

impl<S: SyncStorage> Storage for Overlay<'_, S> {
    fn create_table(
        &mut self,
        _name: &str,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(Err(failure("a command scope cannot create tables")))
    }

    fn insert(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        let table = table.to_owned();
        ready(
            self.write(
                None,
                vec![Mutation::Insert {
                    table,
                    row,
                }],
            )
            .map(|_| ()),
        )
    }

    fn get(
        &self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<Option<Row>, StorageError>> + Send {
        ready(self.image(table, id))
    }

    fn replace(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        let table = table.to_owned();
        ready(
            self.write(
                None,
                vec![Mutation::Replace {
                    table,
                    row,
                }],
            )
            .map(|_| ()),
        )
    }

    fn remove(
        &mut self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send {
        let (table, id) = (table.to_owned(), id.to_owned());
        ready(
            self.write(
                None,
                vec![Mutation::Remove {
                    table,
                    id,
                }],
            )
            .map(|changes| !changes.is_empty()),
        )
    }
}

impl<S: SyncStorage> TransactionalStorage for Overlay<'_, S> {
    fn apply_batch(
        &mut self,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(self.write(None, mutations).map(|_| ()))
    }
}

impl<S: SyncStorage> TypedStorage for Overlay<'_, S> {
    fn query(
        &self,
        schema: &Schema,
        query: &Query,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        ready(self.overlay_query(schema, query))
    }
}

impl<S: SyncStorage> TypedTransactionalStorage for Overlay<'_, S> {
    fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send {
        ready(self.write(Some(schema), mutations))
    }
}

impl<S: SyncStorage> SchemaStorage for Overlay<'_, S> {
    fn catalog(&self) -> impl Future<Output = Result<ModelCatalog, StorageError>> + Send {
        ready((|| {
            let mut catalog = self.base.catalog()?;
            if let Some(staged) = self
                .delta
                .read()
                .rows
                .get(&(MODEL_TABLE.to_owned(), "state".to_owned()))
            {
                catalog.state = staged
                    .as_ref()
                    .map(|row| model_state_from_rows(std::slice::from_ref(row)))
                    .transpose()?;
            }
            Ok(catalog)
        })())
    }

    fn scan(&self, table: &str) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        ready((|| {
            let mut rows: BTreeMap<String, Row> = self
                .base
                .scan(table)?
                .into_iter()
                .map(|row| (row.id.clone(), row))
                .collect();
            for ((staged, id), image) in &self.delta.read().rows {
                if staged == table {
                    match image {
                        Some(row) => rows.insert(id.clone(), row.clone()),
                        None => rows.remove(id),
                    };
                }
            }
            Ok(rows.into_values().collect())
        })())
    }

    fn apply_schema_plan(
        &mut self,
        _plan: SchemaPlan,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(Err(failure("a command scope cannot change the schema")))
    }
}

#[cfg(test)]
mod self_relation_tests {
    use super::*;
    use crate::model::{ROW_VERSION, RelationDefinition, SchemaDefinition, TableDefinition};

    fn schema() -> Schema {
        SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([(
                "nodes".into(),
                TableDefinition {
                    columns: BTreeMap::from([(
                        "name".into(),
                        Column {
                            kind: ColumnType::String,
                            optional: false,
                        },
                    )]),
                    relations: BTreeMap::from([(
                        "parent".into(),
                        RelationDefinition::ToOne {
                            table: "nodes".into(),
                            foreign_key: "parent_id".into(),
                            optional: true,
                        },
                    )]),
                },
            )]),
        }
        .storage_schema()
        .unwrap()
    }

    fn node(id: &str, name: &str, parent: Option<&str>) -> Row {
        let mut fields = BTreeMap::from([
            ("name".into(), Value::String(name.into())),
            (ROW_VERSION.into(), Value::Integer(1)),
        ]);
        if let Some(parent) = parent {
            fields.insert("parent_id".into(), Value::Reference(parent.into()));
        }
        Row {
            id: id.into(),
            fields,
        }
    }

    fn store(rows: &[Row]) -> SnapshotStorage {
        let mut store = SnapshotStorage::default();
        now(store.create_table("nodes")).unwrap();
        for row in rows {
            now(store.insert("nodes", row.clone())).unwrap();
        }
        store
    }

    /// The staged query must equal the same query over the materialized base plus delta.
    fn assert_matches_materialized(base: &[Row], staged: &[Row], query: &Query) {
        let schema = schema();
        let base_store = store(base);
        let mut delta = Delta::default();
        let mut merged: BTreeMap<String, Row> = base
            .iter()
            .map(|row| (row.id.clone(), row.clone()))
            .collect();
        for row in staged {
            delta
                .rows
                .insert(("nodes".into(), row.id.clone()), Some(row.clone()));
            merged.insert(row.id.clone(), row.clone());
        }
        let expected =
            now(store(&merged.into_values().collect::<Vec<_>>()).query(&schema, query)).unwrap();
        let overlay = Overlay::<SnapshotStorage> {
            base: Base::Snapshot(&base_store),
            delta: DeltaRef::Read(&delta),
            validated: false,
        };
        assert_eq!(
            overlay.overlay_query(&schema, query),
            Ok(expected),
            "staged self-relation query differs from the materialized store"
        );
    }

    fn by_parent_name(name: &str) -> Query {
        Query {
            table: "nodes".into(),
            predicates: vec![Predicate {
                field: "parent.name".into(),
                operator: Operator::Eq,
                values: vec![Value::String(name.into())],
            }],
            order: None,
            limit: None,
        }
    }

    fn ordered_by_parent_name(limit: Option<usize>) -> Query {
        Query {
            table: "nodes".into(),
            predicates: vec![],
            order: Some(OrderBy {
                field: "parent.name".into(),
                direction: Direction::Asc,
            }),
            limit,
        }
    }

    // Both ends of a self-referential edge are staged: the target is also a candidate row.
    #[test]
    fn ov_self_relation_staged_child_and_staged_parent() {
        let staged = [node("a", "A", Some("b")), node("b", "B", None)];
        assert_matches_materialized(&[], &staged, &by_parent_name("B"));
        assert_matches_materialized(&[], &staged, &ordered_by_parent_name(None));
    }

    // The staged child's parent is an unstaged base row of the same table: it must be
    // evaluated as a relation target only, and returned once from the base window.
    #[test]
    fn ov_self_relation_staged_child_of_unstaged_parent() {
        let base = [node("t", "T", None), node("u", "U", Some("t"))];
        let staged = [node("a", "A", Some("t"))];
        assert_matches_materialized(&base, &staged, &ordered_by_parent_name(None));
        assert_matches_materialized(&base, &staged, &by_parent_name("T"));
        // A limit must count candidate rows only, never a target-only row.
        assert_matches_materialized(&base, &staged, &ordered_by_parent_name(Some(1)));
        assert_matches_materialized(&base, &staged, &ordered_by_parent_name(Some(2)));
    }

    // A staged change to a parent must reach unstaged children through the base R set.
    #[test]
    fn ov_self_relation_staged_parent_of_unstaged_child() {
        let base = [node("t", "T", None), node("u", "U", Some("t"))];
        let staged = [node("t", "renamed", None)];
        assert_matches_materialized(&base, &staged, &by_parent_name("renamed"));
        assert_matches_materialized(&base, &staged, &ordered_by_parent_name(None));
    }
}
