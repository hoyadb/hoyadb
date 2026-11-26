#[path = "../../../tests/conformance/command_sync.rs"]
mod cases;
#[test]
fn c09_c12_c16_c19_dispatch_delivery() {
    let path =
        std::env::temp_dir().join(format!("hoya-sync-dispatch-{}.sqlite", std::process::id()));
    cases::dispatch(hoyadb_storage_sqlite::SqliteStorage::open_unprotected(&path).unwrap());
    std::fs::remove_file(&path).unwrap();
    cases::correction_inverse(
        hoyadb_storage_sqlite::SqliteStorage::open_unprotected(&path).unwrap(),
    );
    std::fs::remove_file(&path).unwrap();
    cases::graph(hoyadb_storage_sqlite::SqliteStorage::open_unprotected(&path).unwrap());
    std::fs::remove_file(&path).unwrap();
    cases::zero_history(hoyadb_storage_sqlite::SqliteStorage::open_unprotected(&path).unwrap());
    std::fs::remove_file(&path).unwrap();
    cases::capacity_boundary(
        hoyadb_storage_sqlite::SqliteStorage::open_unprotected(&path).unwrap(),
    );
    std::fs::remove_file(&path).unwrap();
    terminal_failure(&path);
    std::fs::remove_file(path).unwrap();
}

fn terminal_failure(path: &std::path::Path) {
    use hoyadb_core::{command::*, storage::*};
    // SPEC-0037 R2.3: the owner holds the file, so install the guarded trigger while
    // closed; it aborts only while this fixture arms it on the live owner.
    drop(hoyadb_storage_sqlite::SqliteStorage::open_unprotected(path).unwrap());
    // DEF-0062: history-zero reconciliation deletes the terminal command;
    // unchanged header packets need no reinsertion. Fail that actual deletion
    // after the application mapping and model writes, inside the same transaction.
    // The trigger predicate proves the mapped row already exists at the fault.
    rusqlite::Connection::open(path).unwrap().execute_batch("CREATE TRIGGER sync_correction_fail BEFORE DELETE ON __hoya_rows WHEN OLD.table_name='__hoya_sync' AND OLD.id LIKE 'command:%' AND EXISTS (SELECT 1 FROM __hoya_rows WHERE table_name='parents' AND id='server:fault') AND hoya_test_fault('sync_correction_fail') BEGIN SELECT RAISE(ABORT,'fixture correction failure'); END").unwrap();
    let mut db = cases::run(CommandDatabase::open(
        hoyadb_storage_sqlite::SqliteStorage::open_unprotected(path).unwrap(),
        cases::schema(),
        vec![],
        [21; 16],
        SyncOptions {
            history: 0,
            ..SyncOptions::default()
        },
    ))
    .unwrap();
    let (key, row) = cases::enqueue(&mut db, "faulted correction");
    let query = db.query("parents");
    let data = cases::run(db.observe(&query)).unwrap();
    let sub = db.observe_outcome(&key).unwrap();
    db.prepare_push(100, 0).unwrap();
    db.receive(&key, Some(RemoteOutcome::Applied(Json::Integer(7))))
        .unwrap();
    db.drain_events(100);
    let queue_before = db.queue_status();
    let epoch_before = db.ledger_work().epoch;
    let fault = hoyadb_storage_sqlite::test_hooks::arm(path, "sync_correction_fail");
    let plan = ReconcilePlan {
        mappings: std::collections::BTreeMap::from([(row.id.clone(), "server:fault".into())]),
        writes: vec![],
    };
    let result = cases::run(db.reconcile(&key, plan.clone()));
    assert!(
        matches!(&result, Err(CommandError::Storage(StorageError::BackendFailure { message }))
            if message == "fixture correction failure"),
        "armed correction result: {result:?}"
    );
    assert!(
        db.queue_status() == queue_before,
        "failed commit changed queue state"
    );
    assert_eq!(db.ledger_work().epoch, epoch_before);
    assert_eq!(cases::run(query.fetch(&db)).unwrap(), vec![row.clone()]);
    assert!(
        db.drain_events(100).is_empty(),
        "failed commit must publish neither data nor terminal outcome"
    );
    assert_eq!(
        db.pending_reconciliations(),
        vec![(key.clone(), RemoteOutcome::Applied(Json::Integer(7)))]
    );
    drop(fault);
    cases::run(db.reconcile(&key, plan)).unwrap();
    let events = db.drain_events(100);
    assert!(matches!(&events[0],CommandEvent::Data(n) if n.subscription==data));
    assert_eq!(events.iter().filter(|e|matches!(e,CommandEvent::Outcome{subscription,value:CommandOutcome::Applied(Json::Integer(7))} if *subscription==sub)).count(),1);
    assert!(db.queue_status().commands.is_empty());
    let mut mapped = row;
    mapped.id = "server:fault".into();
    assert_eq!(cases::run(query.fetch(&db)).unwrap(), vec![mapped.clone()]);
    db.close();
    let store = hoyadb_storage_sqlite::SqliteStorage::open_unprotected(path).unwrap();
    let ledger =
        hoyadb_core::command::records::SyncLedger::from_rows(&store.load_sync().unwrap().unwrap())
            .unwrap();
    assert!(ledger.records.is_empty());
    let reopened = cases::run(CommandDatabase::open(
        store,
        cases::schema(),
        vec![],
        [21; 16],
        SyncOptions {
            history: 0,
            ..SyncOptions::default()
        },
    ))
    .unwrap();
    assert!(reopened.queue_status().commands.is_empty());
    assert_eq!(cases::run(query.fetch(&reopened)).unwrap(), vec![mapped]);
}

#[test]
fn terminal_correction_failure_is_atomic() {
    let path =
        std::env::temp_dir().join(format!("hoya-sync-terminal-{}.sqlite", std::process::id()));
    terminal_failure(&path);
    std::fs::remove_file(path).unwrap();
}
