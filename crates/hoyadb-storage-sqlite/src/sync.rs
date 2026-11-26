//! API-0007: real SQLite application and recovery atomic commit.
use super::{SqliteStorage, backend, get, insert, replace, typed_batch};
#[cfg(test)]
use hoyadb_core::command::records::SyncLedger;
use hoyadb_core::storage::*;
use rusqlite::Connection;

pub(super) fn load(connection: &Connection) -> Result<Option<Vec<Row>>, StorageError> {
    let names = super::lifecycle::names(connection)?;
    if !names.iter().any(|name| name == SYNC_TABLE) {
        return Ok(None);
    }
    if !names.iter().any(|name| name == MODEL_TABLE) {
        return Err(schema_conflict("orphan command recovery"));
    }
    let model = model_state_from_rows(&super::lifecycle::scan(connection, MODEL_TABLE)?)?;
    let rows = super::lifecycle::scan(connection, SYNC_TABLE)?;
    validate_sync_inventory(&rows, &model)?;
    Ok(Some(rows))
}

impl SyncStorage for SqliteStorage {
    fn load_sync(&self) -> Result<Option<Vec<Row>>, StorageError> {
        super::lifecycle::catalog(&self.connection)?;
        load(&self.connection)
    }

    /// Native opens hold the exclusive lease for the connection's life (SPEC-0037 R1).
    /// WASM makes no exclusivity claim.
    fn ownership(&self) -> StoreOwnership {
        if cfg!(target_arch = "wasm32") {
            StoreOwnership::Unproven
        } else {
            StoreOwnership::Exclusive
        }
    }

    /// Native reads run synchronously and return ready futures (SPEC-0037 R6).
    /// WASM claims nothing until its readiness is proven.
    fn reads_ready(&self) -> bool {
        !cfg!(target_arch = "wasm32")
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn apply_sync_delta(&mut self, delta: SyncDelta) -> Result<Vec<CommittedChange>, StorageError> {
        #[cfg(test)]
        delta_tests::record_apply();
        self.transaction(|connection| {
            let schema = validate_sync_delta(&delta)?;
            let parts = delta.into_parts();
            let expected_model = model_state_row(&parts.expected_model)?;
            if scoped_row(connection, MODEL_TABLE, "state", false)?.as_ref()
                != Some(&expected_model)
            {
                return Err(schema_conflict("command model changed before commit"));
            }
            for (id, expected) in &parts.expected_packets {
                if scoped_row(connection, SYNC_TABLE, id, true)?.as_ref() != expected.as_ref() {
                    return Err(schema_conflict("command recovery changed before commit"));
                }
            }
            for guard in &parts.guards {
                if scoped_row(connection, &guard.table, &guard.id, false)?.as_ref()
                    != guard.expected.as_ref()
                {
                    return Err(schema_conflict(GUARD_CONFLICT));
                }
            }
            // Keep only identities for the scoped post-write catalog. Row payloads
            // move into typed_batch; unrelated application/ledger rows are untouched.
            let touched = parts
                .mutations
                .iter()
                .map(|mutation| match mutation {
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
                })
                .collect::<std::collections::BTreeSet<_>>();
            let changes = typed_batch(connection, &schema, parts.mutations)?;
            replace(connection, MODEL_TABLE, model_state_row(&parts.next_model)?)?;
            super::written(
                connection
                    .execute(
                        "INSERT INTO __hoya_tables(name) VALUES (?1) ON CONFLICT(name) DO NOTHING",
                        [SYNC_TABLE],
                    )
                    .map_err(backend)?,
            );
            for id in &parts.deletes {
                if !super::remove(connection, SYNC_TABLE, id)? {
                    return Err(schema_conflict("command recovery changed during commit"));
                }
            }
            for row in &parts.upserts {
                write_packet(connection, row)?;
            }
            for (table, id) in touched {
                if let Some(row) = scoped_row(connection, &table, &id, false)? {
                    validate_row(&schema, &table, &row)?;
                    if row.fields.get(hoyadb_core::model::ROW_VERSION)
                        != Some(&Value::Integer(i64::from(parts.next_model.version)))
                    {
                        return Err(backend("managed row version differs from schema"));
                    }
                }
            }
            if scoped_row(connection, MODEL_TABLE, "state", false)?.as_ref()
                != Some(&model_state_row(&parts.next_model)?)
            {
                return Err(backend("stored model differs from committed delta"));
            }
            let upserts = parts
                .upserts
                .iter()
                .map(|row| (row.id.as_str(), row))
                .collect::<std::collections::BTreeMap<_, _>>();
            let deletes = parts
                .deletes
                .iter()
                .collect::<std::collections::BTreeSet<_>>();
            for (id, previous) in &parts.expected_packets {
                let next = if deletes.contains(id) {
                    None
                } else {
                    upserts.get(id.as_str()).copied().or(previous.as_ref())
                };
                // Equality with the shape-validated packet also validates the
                // stored packet schema without reparsing a whole ledger.
                if scoped_row(connection, SYNC_TABLE, id, false)?.as_ref() != next {
                    return Err(backend("stored recovery differs from committed delta"));
                }
            }
            Ok(changes)
        })
    }

    fn apply_sync_plan(&mut self, plan: SyncPlan) -> Result<Vec<CommittedChange>, StorageError> {
        let schema = validate_sync_plan(&plan)?;
        self.transaction(|connection| {
            if super::lifecycle::catalog(connection)?.state.as_ref() != Some(&plan.expected_model) {
                return Err(schema_conflict("command model changed before commit"));
            }
            let actual = load(connection)?;
            check_sync_rows(actual.as_deref(), plan.expected_sync.as_deref())?;
            if sync_inventory_required(&plan)? {
                let mut actual = Vec::new();
                for table in schema.tables.keys() {
                    for row in super::lifecycle::scan(connection, table)? {
                        actual.push((table.clone(), row.id));
                    }
                }
                validate_reset_inventory(&plan, actual)?;
            }
            for guard in &plan.guards {
                if get(connection, &guard.table, &guard.id)?.as_ref() != guard.expected.as_ref() {
                    return Err(schema_conflict(GUARD_CONFLICT));
                }
            }
            let changes = typed_batch(connection, &schema, plan.mutations)?;
            replace(connection, MODEL_TABLE, model_state_row(&plan.next_model)?)?;
            rewrite_sync_rows(connection, actual.is_some(), plan.next_sync)?;
            super::lifecycle::catalog(connection)?;
            Ok(changes)
        })
    }
}

/// Replaces the complete persisted recovery inventory inside the caller's
/// transaction: registers the sync table on first persist, deletes every stored
/// packet and inserts the supplied rows. Shared by the full plan and the offline
/// converter (SPEC-0037 R11); neither validation nor the transaction lives here.
pub(super) fn rewrite_sync_rows(
    connection: &Connection,
    registered: bool,
    rows: Vec<Row>,
) -> Result<(), StorageError> {
    if !registered {
        connection
            .execute("INSERT INTO __hoya_tables(name) VALUES (?1)", [SYNC_TABLE])
            .map_err(backend)?;
    }
    super::written(
        connection
            .execute(
                "DELETE FROM __hoya_fields WHERE table_name=?1",
                [SYNC_TABLE],
            )
            .map_err(backend)?,
    );
    super::written(
        connection
            .execute("DELETE FROM __hoya_rows WHERE table_name=?1", [SYNC_TABLE])
            .map_err(backend)?,
    );
    for row in rows {
        insert(connection, SYNC_TABLE, row)?;
    }
    Ok(())
}

/// A primary-key catalog check. Strict open owns the complete orphan scan;
/// an Exclusive delta checks only the identities protected or written by it.
#[cfg(not(target_arch = "wasm32"))]
fn scoped_row(
    connection: &Connection,
    table: &str,
    id: &str,
    allow_unregistered: bool,
) -> Result<Option<Row>, StorageError> {
    let registered: bool = super::query_one(
        connection,
        "SELECT EXISTS(SELECT 1 FROM __hoya_tables WHERE name=?1)",
        [table],
    )?;
    let exists = super::row_exists(connection, table, id)?;
    let orphan_fields = !exists
        && super::query_one::<bool>(
            connection,
            "SELECT EXISTS(SELECT 1 FROM __hoya_fields WHERE table_name=?1 AND row_id=?2)",
            rusqlite::params![table, id],
        )?;
    if (!registered && exists) || orphan_fields {
        return Err(backend("orphan storage bookkeeping"));
    }
    if !registered && allow_unregistered {
        return Ok(None);
    }
    get(connection, table, id)
}

/// Borrow the owned packet while writing so the final scoped check can compare
/// its exact validated bytes without copying the packet or any unrelated row.
#[cfg(not(target_arch = "wasm32"))]
fn write_packet(connection: &Connection, row: &Row) -> Result<(), StorageError> {
    super::require_table(connection, SYNC_TABLE)?;
    if super::row_exists(connection, SYNC_TABLE, &row.id)? {
        super::written(
            connection
                .execute(
                    "DELETE FROM __hoya_fields WHERE table_name=?1 AND row_id=?2",
                    rusqlite::params![SYNC_TABLE, row.id],
                )
                .map_err(backend)?,
        );
    } else {
        super::written(
            connection
                .execute(
                    "INSERT INTO __hoya_rows(table_name,id) VALUES (?1,?2)",
                    rusqlite::params![SYNC_TABLE, row.id],
                )
                .map_err(backend)?,
        );
    }
    super::write_fields(connection, SYNC_TABLE, row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoyadb_core::{
        command::records::{SyncHeader, SyncOptions},
        model::{Database, ROW_VERSION, SchemaDefinition, TableDefinition},
    };
    use std::{
        collections::BTreeMap,
        future::Future,
        sync::atomic::{AtomicUsize, Ordering},
        task::{Context, Poll, Waker},
    };

    fn run<F: Future>(future: F) -> F::Output {
        match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("SQLite operation suspended"),
        }
    }

    #[test]
    fn c07_real_commit_fault_preserves_absence_and_reopen_identity() {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "hoya-sync-commit-{}-{}.sqlite",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let definition = SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([("items".into(), TableDefinition::default())]),
        };
        let mut store = run(Database::open(
            SqliteStorage::open_unprotected(&path).unwrap(),
            definition,
            vec![],
        ))
        .unwrap()
        .into_storage();
        let original = run(store.catalog()).unwrap().state.unwrap();
        let mut next = original.clone();
        next.next_id = 2;
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
        let row = Row {
            id: "local:1".into(),
            fields: BTreeMap::from([(ROW_VERSION.into(), Value::Integer(1))]),
        };
        let plan = SyncPlan {
            transition: SyncTransition::Normal,
            expected_model: original.clone(),
            next_model: next.clone(),
            expected_sync: None,
            next_sync: ledger.to_rows().unwrap(),
            guards: vec![RowGuard {
                table: "items".into(),
                id: row.id.clone(),
                expected: None,
            }],
            mutations: vec![Mutation::Insert {
                table: "items".into(),
                row: row.clone(),
            }],
        };
        store.connection.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE sync_guard_parent(id INTEGER PRIMARY KEY); CREATE TABLE sync_guard_child(parent INTEGER REFERENCES sync_guard_parent(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER sync_commit_fault AFTER INSERT ON __hoya_rows WHEN NEW.table_name='__hoya_sync' BEGIN INSERT INTO sync_guard_child(parent) VALUES(1); END;").unwrap();
        let error = store.apply_sync_plan(plan.clone()).unwrap_err();
        assert!(matches!(error, StorageError::BackendFailure { .. }));
        assert_eq!(store.load_sync().unwrap(), None);
        assert_eq!(run(store.catalog()).unwrap().state, Some(original));
        assert_eq!(run(store.get("items", "local:1")).unwrap(), None);
        assert_eq!(
            store
                .connection
                .query_row("SELECT COUNT(*) FROM sync_guard_child", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        store.connection.execute_batch("DROP TRIGGER sync_commit_fault; DROP TABLE sync_guard_child; DROP TABLE sync_guard_parent;").unwrap();
        store.apply_sync_plan(plan).unwrap();
        let persisted = store.load_sync().unwrap();
        drop(store);
        let reopened = SqliteStorage::open_unprotected(&path).unwrap();
        assert_eq!(reopened.load_sync().unwrap(), persisted);
        assert_eq!(run(reopened.catalog()).unwrap().state, Some(next));
        assert_eq!(run(reopened.get("items", "local:1")).unwrap(), Some(row));
        let physical = reopened
            .connection
            .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            physical,
            vec![
                "__hoya_fields",
                "__hoya_meta",
                "__hoya_rows",
                "__hoya_tables"
            ]
        );
        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(dead_code)]
#[path = "../../../tests/conformance/si_y03_adapter_characterization.rs"]
mod delta_cases;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(dead_code)]
#[path = "../../../tests/conformance/work_counters.rs"]
mod delta_work_cases;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod delta_tests {
    use super::*;
    use hoyadb_core::{
        command::{CommandDatabase, CommandDefinition, CommandError, Json, records::SyncOptions},
        model::{SchemaDefinition, TableDefinition},
    };
    use std::{
        cell::Cell,
        collections::BTreeMap,
        future::Future,
        path::PathBuf,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        },
        task::{Context, Poll, Waker},
    };

    thread_local! {
        static DELTA_APPLIES: Cell<usize> = const { Cell::new(0) };
    }

    pub(super) fn record_apply() {
        DELTA_APPLIES.set(DELTA_APPLIES.get() + 1);
    }

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "hoya-sync-delta-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> PathBuf {
            self.0.join("store.sqlite")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn run<F: Future>(future: F) -> F::Output {
        match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("native SQLite operation suspended"),
        }
    }

    fn schema() -> SchemaDefinition {
        SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([("items".into(), TableDefinition::default())]),
        }
    }

    fn definition() -> CommandDefinition {
        CommandDefinition {
            name: "delta-insert-v1".into(),
            arguments: BTreeMap::new(),
            reference_paths: vec![],
        }
    }

    type RawSnapshot = Vec<Vec<Vec<rusqlite::types::Value>>>;

    fn raw_snapshot(store: &SqliteStorage) -> RawSnapshot {
        [
            "SELECT version FROM __hoya_meta ORDER BY version",
            "SELECT name FROM __hoya_tables ORDER BY name",
            "SELECT table_name,id FROM __hoya_rows ORDER BY table_name,id",
            "SELECT table_name,row_id,name,kind,text_value,int_value FROM __hoya_fields ORDER BY table_name,row_id,name",
        ]
        .iter()
        .map(|sql| {
            let mut statement = store.connection.prepare(sql).unwrap();
            let columns = statement.column_count();
            statement
                .query_map([], |row| {
                    (0..columns)
                        .map(|index| row.get::<_, rusqlite::types::Value>(index))
                        .collect::<rusqlite::Result<Vec<_>>>()
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        })
        .collect()
    }

    #[test]
    fn si_y07_delta_write_counter_includes_first_registration() {
        for registered in [false, true] {
            let fixture = Fixture::new();
            let armed = Arc::new(AtomicBool::new(false));
            let hook_armed = armed.clone();
            let before = Arc::new(AtomicU64::new(0));
            let hook_before = before.clone();
            let store = super::delta_cases::hooked(
                SqliteStorage::open_unprotected(fixture.path()).unwrap(),
                move |store| {
                    if hook_armed.swap(false, Ordering::Relaxed) {
                        hook_before.store(store.connection.total_changes(), Ordering::Relaxed);
                        counters::reset();
                    }
                },
            );
            let mut owner = run(CommandDatabase::open(
                store,
                schema(),
                vec![],
                [0x30; 16],
                SyncOptions::default(),
            ))
            .unwrap();
            for setup in (0..=usize::from(registered)).rev() {
                let request = owner.request().unwrap();
                run(owner.begin(
                    request,
                    definition(),
                    Json::Object(BTreeMap::new()),
                    Some(Json::Null),
                    1,
                ))
                .unwrap();
                run(owner.scope_insert(request, "items", BTreeMap::new())).unwrap();
                if setup == 0 {
                    DELTA_APPLIES.set(0);
                    armed.store(true, Ordering::Relaxed);
                }
                run(owner.finish(request, Json::Null)).unwrap();
            }
            let measured = counters::snapshot().sqlite_rows_written;
            let store = owner.into_storage().into_inner();
            let actual = store.connection.total_changes() - before.load(Ordering::Relaxed);
            assert_eq!(DELTA_APPLIES.get(), 1, "actual adapter override ran");
            assert!(!armed.load(Ordering::Relaxed), "apply boundary measured");
            assert!(actual > 0);
            assert_eq!(measured, actual, "previously registered: {registered}");
        }
    }

    #[test]
    fn si_y07_actual_delta_sql_work_is_bounded_by_touched_rows() {
        let work = |m| {
            let fixture = Fixture::new();
            let armed = Arc::new(AtomicBool::new(false));
            let hook_armed = armed.clone();
            let store = super::delta_cases::hooked(
                SqliteStorage::open_unprotected(fixture.path()).unwrap(),
                move |_| {
                    if hook_armed.swap(false, Ordering::Relaxed) {
                        counters::reset();
                    }
                },
            );
            let mut owner = super::delta_work_cases::owner(store, m);
            DELTA_APPLIES.set(0);
            // Isolate the actual adapter apply boundary; this is deliberately
            // not a claim that the still-converting core planner is bounded.
            armed.store(true, Ordering::Relaxed);
            run(owner.patch(
                "items",
                &super::delta_work_cases::id(0),
                BTreeMap::from([("body".into(), Value::String("patched".into()))]),
            ))
            .unwrap();
            let measured = counters::snapshot();
            assert_eq!(DELTA_APPLIES.get(), 1, "actual adapter override ran");
            assert!(
                !armed.load(Ordering::Relaxed),
                "apply boundary was measured"
            );
            measured
        };
        let small = work(10);
        let large = work(1000);
        eprintln!("SI-Y07 SQLite apply M=10 {small:?}\nM=1000 {large:?}");
        assert!(small.sqlite_rows_written > 0);
        assert_eq!(small.sqlite_rows_written, large.sqlite_rows_written);
        assert_eq!(small.sqlite_rows_read, large.sqlite_rows_read);
        assert_eq!(small.sqlite_fullscan_steps, 0);
        assert_eq!(large.sqlite_fullscan_steps, 0);
        assert_eq!(small.sqlite_sort, 0);
        assert_eq!(large.sqlite_sort, 0);
    }

    #[test]
    fn si_y03_scoped_orphans_and_missing_registration_preserve_racing_state() {
        for (name, corrupt, repair) in [
            (
                "application orphan",
                "INSERT INTO __hoya_fields(table_name,row_id,name,kind,int_value) VALUES('items','local:1','__hoya_schema_version',2,1)",
                "DELETE FROM __hoya_fields WHERE table_name='items' AND row_id='local:1'",
            ),
            (
                "packet orphan",
                "INSERT INTO __hoya_fields(table_name,row_id,name,kind,text_value) VALUES('__hoya_sync','state','data',1,'{}')",
                "DELETE FROM __hoya_fields WHERE table_name='__hoya_sync' AND row_id='state'",
            ),
            (
                "missing table registration",
                "DELETE FROM __hoya_tables WHERE name='items'",
                "INSERT INTO __hoya_tables(name) VALUES('items')",
            ),
        ] {
            DELTA_APPLIES.set(0);
            let fixture = Fixture::new();
            let path = fixture.path();
            let armed = Arc::new(AtomicBool::new(false));
            let armed_hook = armed.clone();
            let raced = Arc::new(Mutex::new(None));
            let raced_hook = raced.clone();
            let store = super::delta_cases::hooked(
                SqliteStorage::open_unprotected(&path).unwrap(),
                move |store| {
                    if armed_hook.swap(false, Ordering::Relaxed) {
                        let before = raw_snapshot(store);
                        assert_eq!(store.execute_test_sql(corrupt).unwrap(), 1);
                        let after = raw_snapshot(store);
                        assert_ne!(before, after, "fixture changes the actual adapter");
                        *raced_hook.lock().unwrap() = Some(after);
                    }
                },
            );
            let mut owner = run(CommandDatabase::open(
                store,
                schema(),
                vec![],
                [0x29; 16],
                SyncOptions::default(),
            ))
            .unwrap();
            let request = owner.request().unwrap();
            run(owner.begin(
                request,
                definition(),
                Json::Object(BTreeMap::new()),
                Some(Json::Null),
                1,
            ))
            .unwrap();
            let row = run(owner.scope_insert(request, "items", BTreeMap::new())).unwrap();
            assert_eq!(row.id, "local:1");
            armed.store(true, Ordering::Relaxed);
            assert!(
                matches!(
                    run(owner.finish(request, Json::Null)),
                    Err(CommandError::Storage(_))
                ),
                "{name} must reject inside the commit boundary"
            );
            let store = owner.into_storage().into_inner();
            assert_eq!(DELTA_APPLIES.get(), 1, "genuine proof reached SQLite");
            assert_eq!(
                raw_snapshot(&store),
                raced
                    .lock()
                    .unwrap()
                    .take()
                    .expect("real pre-apply hook ran"),
                "{name}: the attempted delta added no partial write"
            );
            store.execute_test_sql(repair).unwrap();
            drop(store);
            let reopened = SqliteStorage::open_unprotected(&path).unwrap();
            assert_eq!(reopened.load_sync().unwrap(), None, "{name}");
            assert_eq!(run(reopened.catalog()).unwrap().state.unwrap().next_id, 1);
        }
    }

    #[test]
    fn si_y03_real_commit_failure_preserves_owner_and_initial_ledger() {
        DELTA_APPLIES.set(0);
        let fixture = Fixture::new();
        let path = fixture.path();
        let store = SqliteStorage::open_unprotected(&path).unwrap();
        // The trigger reaches SQLite's write phase, but its deferred foreign key
        // fails only at COMMIT. The external counter proves the trigger ran even
        // though its SQL changes are subsequently rolled back.
        let hits = Arc::new(AtomicUsize::new(0));
        let trigger_hits = hits.clone();
        store
            .connection
            .create_scalar_function(
                "hoya_delta_write_reached",
                0,
                rusqlite::functions::FunctionFlags::SQLITE_UTF8,
                move |_| {
                    trigger_hits.fetch_add(1, Ordering::Relaxed);
                    Ok(0_i64)
                },
            )
            .unwrap();
        store.connection.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE delta_parent(id INTEGER PRIMARY KEY); CREATE TABLE delta_child(parent INTEGER REFERENCES delta_parent(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER delta_commit_fault AFTER INSERT ON __hoya_rows WHEN NEW.table_name='__hoya_sync' AND hoya_test_fault('delta_commit_fault') BEGIN SELECT hoya_delta_write_reached(); INSERT INTO delta_child(parent) VALUES(1); END;").unwrap();
        let mut owner = run(CommandDatabase::open(
            store,
            schema(),
            vec![],
            [0x27; 16],
            SyncOptions::default(),
        ))
        .unwrap();
        let query = owner.query("items");
        run(owner.observe(&query)).unwrap();
        owner.drain_notifications(0);
        let request = owner.request().unwrap();
        run(owner.begin(
            request,
            definition(),
            Json::Object(BTreeMap::new()),
            Some(Json::Null),
            1,
        ))
        .unwrap();
        let staged = run(owner.scope_insert(request, "items", BTreeMap::new())).unwrap();
        let fault = crate::test_hooks::arm(&path, "delta_commit_fault");
        assert!(matches!(
            run(owner.finish(request, Json::Null)),
            Err(CommandError::Storage(StorageError::BackendFailure { .. }))
        ));
        assert!(hits.load(Ordering::Relaxed) > 0, "writes reached SQLite");
        assert!(run(query.fetch(&owner)).unwrap().is_empty());
        assert!(owner.drain_notifications(1).is_empty());
        assert!(owner.scope_args(request).is_err());
        drop(fault);

        let retry = owner.request().unwrap();
        run(owner.begin(
            retry,
            definition(),
            Json::Object(BTreeMap::new()),
            Some(Json::Null),
            1,
        ))
        .unwrap();
        let committed = run(owner.scope_insert(retry, "items", BTreeMap::new())).unwrap();
        assert_eq!(committed.id, staged.id, "failed commit restores next_id");
        let result = run(owner.finish(retry, Json::Null)).unwrap();
        assert!(result.key.ends_with("0000000000000001"));
        assert_eq!(owner.drain_notifications(1).len(), 1);
        let store = owner.into_storage();
        assert_eq!(DELTA_APPLIES.get(), 2, "both actual delta attempts ran");
        let ledger = store.load_sync().unwrap().unwrap();
        let parsed = SyncLedger::from_rows(&ledger).unwrap();
        assert_eq!(parsed.header.sequence, 2);
        assert_eq!(parsed.records.len(), 1);
        assert_eq!(run(store.catalog()).unwrap().state.unwrap().next_id, 2);
        assert_eq!(
            store
                .connection
                .query_row("SELECT count(*) FROM delta_child", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0,
            "failed transaction's trigger changes rolled back"
        );
        store
            .connection
            .execute_batch(
                "DROP TRIGGER delta_commit_fault; DROP TABLE delta_child; DROP TABLE delta_parent;",
            )
            .unwrap();
        drop(store);
        let reopened = SqliteStorage::open_unprotected(&path).unwrap();
        assert_eq!(reopened.load_sync().unwrap(), Some(ledger));
        assert_eq!(
            run(reopened.get("items", &committed.id))
                .unwrap()
                .unwrap()
                .id,
            committed.id
        );
    }

    #[test]
    fn si_y03_touched_storage_corruption_during_write_is_atomic() {
        for (name, trigger) in [
            (
                "row_version",
                "CREATE TRIGGER delta_touched_fault AFTER INSERT ON __hoya_fields WHEN NEW.table_name='items' AND NEW.name='__hoya_schema_version' AND hoya_test_fault('delta_touched_fault') BEGIN UPDATE __hoya_fields SET int_value=0 WHERE table_name=NEW.table_name AND row_id=NEW.row_id AND name=NEW.name; END;",
            ),
            (
                "packet",
                "CREATE TRIGGER delta_touched_fault AFTER INSERT ON __hoya_fields WHEN NEW.table_name='__hoya_sync' AND NEW.name='data' AND hoya_test_fault('delta_touched_fault') BEGIN UPDATE __hoya_fields SET text_value='{' WHERE table_name=NEW.table_name AND row_id=NEW.row_id AND name=NEW.name; END;",
            ),
        ] {
            DELTA_APPLIES.set(0);
            let fixture = Fixture::new();
            let path = fixture.path();
            let store = SqliteStorage::open_unprotected(&path).unwrap();
            store.connection.execute_batch(trigger).unwrap();
            let mut owner = run(CommandDatabase::open(
                store,
                schema(),
                vec![],
                [0x28; 16],
                SyncOptions::default(),
            ))
            .unwrap();
            let query = owner.query("items");
            run(owner.observe(&query)).unwrap();
            owner.drain_notifications(0);
            let request = owner.request().unwrap();
            run(owner.begin(
                request,
                definition(),
                Json::Object(BTreeMap::new()),
                Some(Json::Null),
                1,
            ))
            .unwrap();
            run(owner.scope_insert(request, "items", BTreeMap::new())).unwrap();
            let fault = crate::test_hooks::arm(&path, "delta_touched_fault");
            assert!(
                matches!(
                    run(owner.finish(request, Json::Null)),
                    Err(CommandError::Storage(_))
                ),
                "{name} corrupts an identity written by this commit"
            );
            assert!(run(query.fetch(&owner)).unwrap().is_empty(), "{name}");
            assert!(owner.drain_notifications(1).is_empty(), "{name}");
            let store = owner.into_storage();
            assert_eq!(DELTA_APPLIES.get(), 1, "genuine proof reached SQLite");
            assert_eq!(store.load_sync().unwrap(), None, "{name}");
            assert_eq!(
                run(store.catalog()).unwrap().state.unwrap().next_id,
                1,
                "{name}"
            );
            drop(fault);
            store
                .connection
                .execute_batch("DROP TRIGGER delta_touched_fault")
                .unwrap();
            drop(store);
            let reopened = SqliteStorage::open_unprotected(&path).unwrap();
            assert_eq!(reopened.load_sync().unwrap(), None, "{name}");
            assert!(run(reopened.scan("items")).unwrap().is_empty(), "{name}");
        }
    }
}
