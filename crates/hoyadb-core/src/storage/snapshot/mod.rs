//! SPEC-0019 / API-0002: isolated, in-process storage with no I/O.
use crate::storage::{Mutation, Row, Storage, StorageError, TransactionalStorage};
use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};
#[cfg(test)]
mod delta_tests;
mod lifecycle;
mod projection;
mod sync;
mod typed;
mod undo;
use undo::UndoGuard;
type FieldIndex = BTreeMap<crate::storage::Value, BTreeSet<String>>;

type TableIndexes = BTreeMap<String, FieldIndex>;

#[derive(Default, Clone)]
pub struct SnapshotStorage {
    tables: BTreeMap<String, BTreeMap<String, Row>>,
    indexes: BTreeMap<String, TableIndexes>,
}

#[cfg(test)]
thread_local! {
    static FAULT: std::cell::Cell<Option<(&'static str, u32)>> =
        const { std::cell::Cell::new(None) };
}

/// TC-0041 SI-M03: panic at the named point after skipping `skip` earlier hits
/// on the current thread (tests only).
#[cfg(test)]
pub(crate) fn arm_fault(name: &'static str, skip: u32) {
    FAULT.with(|fault| fault.set(Some((name, skip))));
}

/// A test-only panic point inside a mutation primitive; nothing outside tests.
fn fault_point(_name: &'static str) {
    #[cfg(test)]
    FAULT.with(|fault| match fault.get() {
        Some((name, 0)) if name == _name => {
            fault.set(None);
            panic!("injected fault at {_name}");
        }
        Some((name, skip)) if name == _name => fault.set(Some((name, skip - 1))),
        _ => {}
    });
}

impl Storage for SnapshotStorage {
    async fn create_table(&mut self, name: &str) -> Result<(), StorageError> {
        self.tables.entry(name.into()).or_default();
        Ok(())
    }

    async fn insert(&mut self, table: &str, row: Row) -> Result<(), StorageError> {
        let rows = self
            .tables
            .get_mut(table)
            .ok_or_else(|| StorageError::TableNotFound {
                table: table.into(),
            })?;
        match rows.entry(row.id.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(row.clone());
                fault_point("map-written");
                self.index_insert(table, &row);
                Ok(())
            }
            Entry::Occupied(_) => Err(StorageError::RowAlreadyExists {
                table: table.into(),
                id: row.id,
            }),
        }
    }

    async fn get(&self, table: &str, id: &str) -> Result<Option<Row>, StorageError> {
        let rows = self
            .tables
            .get(table)
            .ok_or_else(|| StorageError::TableNotFound {
                table: table.into(),
            })?;
        Ok(rows.get(id).cloned())
    }

    async fn replace(&mut self, table: &str, row: Row) -> Result<(), StorageError> {
        let rows = self
            .tables
            .get_mut(table)
            .ok_or_else(|| StorageError::TableNotFound {
                table: table.into(),
            })?;
        let stored = rows
            .get_mut(&row.id)
            .ok_or_else(|| StorageError::RowNotFound {
                table: table.into(),
                id: row.id.clone(),
            })?;
        let before = std::mem::replace(stored, row.clone());
        fault_point("map-written");
        self.index_remove(table, &before);
        fault_point("index-removed");
        self.index_insert(table, &row);
        Ok(())
    }

    async fn remove(&mut self, table: &str, id: &str) -> Result<bool, StorageError> {
        let rows = self
            .tables
            .get_mut(table)
            .ok_or_else(|| StorageError::TableNotFound {
                table: table.into(),
            })?;
        let removed = rows.remove(id);
        if let Some(row) = &removed {
            fault_point("map-written");
            self.index_remove(table, row);
        }
        Ok(removed.is_some())
    }
}

impl TransactionalStorage for SnapshotStorage {
    async fn apply_batch(&mut self, mutations: Vec<Mutation>) -> Result<(), StorageError> {
        // SPEC-0037 R7: mutate in place; on any error or unwinding panic the
        // guard restores every touched entry, so untouched rows are never copied.
        let mut guard = UndoGuard::new(self);
        for mutation in mutations {
            match mutation {
                Mutation::Insert {
                    table,
                    row,
                } => guard.insert(&table, row)?,
                Mutation::Replace {
                    table,
                    row,
                } => guard.replace(&table, row)?,
                Mutation::Remove {
                    table,
                    id,
                } => {
                    guard.remove(&table, &id)?;
                }
            }
        }
        guard.commit();
        Ok(())
    }
}

impl SnapshotStorage {
    /// SPEC-0037 R4: a whole-store copy counts every row it clones.
    fn whole_store_clone(&self) -> Self {
        crate::storage::count_work!(
            rows_cloned,
            self.tables
                .values()
                .map(|rows| rows.len() as u64)
                .sum::<u64>()
        );
        self.clone()
    }

    fn index_insert(&mut self, table: &str, row: &Row) {
        insert_entries(&mut self.indexes, table, row);
    }

    fn index_remove(&mut self, table: &str, row: &Row) {
        remove_entries(&mut self.indexes, table, row);
    }
}

fn insert_entries(indexes: &mut BTreeMap<String, TableIndexes>, table: &str, row: &Row) {
    let index = indexes.entry(table.into()).or_default();
    index
        .entry("id".into())
        .or_default()
        .entry(crate::storage::Value::String(row.id.clone()))
        .or_default()
        .insert(row.id.clone());
    fault_point("index-partial-add");
    for (name, value) in &row.fields {
        if name != "id" {
            index
                .entry(name.clone())
                .or_default()
                .entry(value.clone())
                .or_default()
                .insert(row.id.clone());
        }
    }
}

/// Removes this row's entries; entries already absent are left absent.
fn remove_entries(indexes: &mut BTreeMap<String, TableIndexes>, table: &str, row: &Row) {
    let Some(index) = indexes.get_mut(table) else {
        return;
    };
    let values = std::iter::once((
        "id".to_owned(),
        crate::storage::Value::String(row.id.clone()),
    ))
    .chain(
        row.fields
            .iter()
            .filter(|(name, _)| name.as_str() != "id")
            .map(|(name, value)| (name.clone(), value.clone())),
    );
    for (position, (name, value)) in values.enumerate() {
        if position == 1 {
            fault_point("index-partial-remove");
        }
        if let Some(field) = index.get_mut(&name)
            && let Some(ids) = field.get_mut(&value)
        {
            ids.remove(&row.id);
            if ids.is_empty() {
                field.remove(&value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        command::records::{SyncHeader, SyncLedger, SyncOptions},
        model::{Database, ROW_VERSION, SchemaDefinition, TableDefinition},
        storage::{
            Column, ColumnType, RowGuard, Schema, SyncPlan, SyncStorage, SyncTransition,
            TypedTransactionalStorage, Value, counters,
        },
    };
    use std::{
        future::Future,
        panic::{AssertUnwindSafe, catch_unwind},
        task::{Context, Poll, Waker},
    };

    fn run<F: Future>(future: F) -> F::Output {
        match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("memory operation suspended"),
        }
    }

    fn definition() -> SchemaDefinition {
        SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([(
                "items".into(),
                TableDefinition {
                    columns: BTreeMap::from([(
                        "label".into(),
                        Column {
                            kind: ColumnType::String,
                            optional: false,
                        },
                    )]),
                    ..Default::default()
                },
            )]),
        }
    }

    fn schema() -> Schema {
        definition().storage_schema().unwrap()
    }

    fn row(id: &str, label: Value) -> Row {
        Row {
            id: id.into(),
            fields: BTreeMap::from([
                ("label".into(), label),
                (ROW_VERSION.into(), Value::Integer(1)),
            ]),
        }
    }

    fn text(value: &str) -> Value {
        Value::String(value.into())
    }

    fn insert(id: &str, label: Value) -> Mutation {
        Mutation::Insert {
            table: "items".into(),
            row: row(id, label),
        }
    }

    /// A store of n application rows plus model metadata and committed sync packets.
    fn seeded(n: usize) -> SnapshotStorage {
        let mut store = run(Database::open(
            SnapshotStorage::default(),
            definition(),
            vec![],
        ))
        .unwrap()
        .into_storage();
        for index in 0..n {
            run(store.insert("items", row(&format!("seed:{index:04}"), text("seed")))).unwrap();
        }
        store.apply_sync_plan(plan(&store, vec![])).unwrap();
        store
    }

    fn plan(store: &SnapshotStorage, mutations: Vec<Mutation>) -> SyncPlan {
        let expected_model = store.model_catalog().unwrap().state.unwrap();
        let mut next_model = expected_model.clone();
        next_model.next_id += mutations.len() as i64;
        let ledger = SyncLedger {
            entity: None,
            header: SyncHeader {
                namespace: "a".repeat(32),
                sequence: 1,
                last_ms: 100,
                options: SyncOptions::default(),
            },
            records: BTreeMap::new(),
        };
        SyncPlan {
            transition: SyncTransition::Normal,
            expected_model,
            next_model,
            expected_sync: store.load_sync().unwrap(),
            next_sync: ledger.to_rows().unwrap(),
            guards: mutations
                .iter()
                .map(|mutation| match mutation {
                    Mutation::Insert {
                        table,
                        row,
                    } => RowGuard {
                        table: table.clone(),
                        id: row.id.clone(),
                        expected: None,
                    },
                    _ => unreachable!("sync fixtures insert only"),
                })
                .collect(),
            mutations,
        }
    }

    /// k = 4 mutations touching an insert, a replace and a remove.
    fn batch() -> Vec<Mutation> {
        vec![
            insert("new:1", text("new")),
            Mutation::Replace {
                table: "items".into(),
                row: row("seed:0000", text("edited")),
            },
            Mutation::Remove {
                table: "items".into(),
                id: "seed:0001".into(),
            },
            insert("new:2", text("new")),
        ]
    }

    fn assert_same(actual: &SnapshotStorage, expected: &SnapshotStorage, context: &str) {
        assert!(actual.tables == expected.tables, "{context}: rows differ");
        assert!(
            actual.indexes == expected.indexes,
            "{context}: index entries differ"
        );
    }

    /// TC-0041 SI-M01: a batch of k mutations clones at most k rows, equal at N.
    #[test]
    fn si_m01_batch_of_k_clones_at_most_k_rows_equal_at_n() {
        let mut clones = Vec::new();
        for n in [10, 1000] {
            let mut store = seeded(n);
            counters::reset();
            run(store.apply_batch(batch())).unwrap();
            clones.push(counters::snapshot().rows_cloned);
        }
        assert!(clones[0] <= 4, "rows_cloned {clones:?} exceeds k");
        assert_eq!(clones[0], clones[1], "rows_cloned depends on N");
    }

    /// TC-0041 SI-M02: an error at mutation j of k restores every row, index
    /// entry and sync packet, and a selective index query keeps its result.
    #[test]
    fn si_m02_error_at_mutation_j_restores_rows_indexes_and_packets() {
        let mut store = seeded(10);
        let before = store.clone();
        let selective = crate::storage::Query {
            table: "items".into(),
            predicates: vec![crate::storage::Predicate {
                field: "label".into(),
                operator: crate::storage::Operator::Eq,
                values: vec![text("seed")],
            }],
            order: None,
            limit: None,
        };
        let seeds = store.candidate_count(&schema(), &selective).unwrap();
        let mut failing = batch();
        failing.push(insert("seed:0002", text("duplicate")));
        assert!(run(store.apply_batch(failing.clone())).is_err());
        assert_same(&store, &before, "apply_batch");
        failing.pop();
        failing.push(insert("new:3", Value::Integer(7)));
        assert!(run(store.apply_typed_batch(&schema(), failing)).is_err());
        assert_same(&store, &before, "apply_typed_batch");
        let invalid = plan(
            &store,
            vec![
                insert("new:1", text("new")),
                insert("new:2", Value::Integer(7)),
            ],
        );
        assert!(store.apply_sync_plan(invalid).is_err());
        assert_same(&store, &before, "apply_sync_plan");
        assert_eq!(store.load_sync().unwrap(), before.load_sync().unwrap());
        assert_eq!(store.candidate_count(&schema(), &selective).unwrap(), seeds);
    }

    /// TC-0041 SI-M03: a panic at any point inside a primitive unwinds to the
    /// pre-batch state, and the store stays usable.
    #[test]
    fn si_m03_panics_inside_primitives_are_undone() {
        type Operation = fn(&mut SnapshotStorage);
        let operations: [(&str, Operation); 3] = [
            ("apply_batch", |store| {
                run(store.apply_batch(batch())).unwrap()
            }),
            ("apply_typed_batch", |store| {
                run(store.apply_typed_batch(&schema(), batch())).unwrap();
            }),
            ("apply_sync_plan", |store| {
                let plan = plan(store, vec![insert("new:1", text("new"))]);
                store.apply_sync_plan(plan).unwrap();
            }),
        ];
        // Points after mutation j are reached by skipping the earlier hits.
        let points = [
            ("map-written", 0),
            ("map-written", 2),
            ("index-partial-add", 0),
            ("index-partial-add", 1),
            ("index-removed", 0),
            ("index-partial-remove", 0),
            ("index-partial-remove", 1),
            ("table-created", 0),
            ("before-commit", 0),
        ];
        for (label, operation) in operations {
            for (point, skip) in points {
                let reachable = match point {
                    "index-removed" | "index-partial-remove" => label != "apply_sync_plan",
                    "table-created" => label == "apply_sync_plan",
                    _ => label != "apply_sync_plan" || skip == 0,
                };
                if !reachable {
                    continue;
                }
                let context = format!("{label} at {point}+{skip}");
                let mut store = seeded(10);
                let before = store.clone();
                arm_fault(point, skip);
                let unwound = catch_unwind(AssertUnwindSafe(|| operation(&mut store)));
                assert!(
                    unwound.is_err(),
                    "{context}: the fault point was not reached"
                );
                assert_same(&store, &before, &context);
                let mut expected = before.clone();
                operation(&mut expected);
                operation(&mut store);
                assert_same(&store, &expected, &format!("{context}: reuse"));
            }
        }
    }
}
