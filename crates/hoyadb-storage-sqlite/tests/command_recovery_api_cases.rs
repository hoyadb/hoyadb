#[path = "../../../tests/conformance/command_recovery_api.rs"]
mod cases;
use hoyadb_storage_sqlite::SqliteStorage;

fn path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "hoya-recovery-api-{name}-{}.sqlite",
        std::process::id()
    ))
}

// Post-GREEN API-surface cases (see the shared file header); run with --test-threads=1.
#[test]
fn tc0042_awaiting_key() {
    let path = path("awaiting");
    cases::awaiting_key(SqliteStorage::open_unprotected(&path).unwrap());
    std::fs::remove_file(path).unwrap();
}

// CL04, then a real reopen of the SQLite file must reproduce the final queue status.
#[test]
fn tc0042_cl04_redelivery_survives_reopen() {
    let path = path("cl04");
    let (storage, status) = cases::cl04(SqliteStorage::open_unprotected(&path).unwrap());
    drop(storage);
    let db = cases::open(SqliteStorage::open_unprotected(&path).unwrap());
    assert!(db.queue_status() == status, "reopened status differs");
    db.close();
    std::fs::remove_file(path).unwrap();
}

// Only called while no owner holds the store.
fn ledger_bytes(path: &std::path::Path) -> Option<Vec<hoyadb_core::storage::Row>> {
    use hoyadb_core::storage::SyncStorage;
    SqliteStorage::open_unprotected(path)
        .unwrap()
        .load_sync()
        .unwrap()
}

const FAIL: &str = "CREATE TRIGGER recovery_fail BEFORE INSERT ON __hoya_fields WHEN NEW.table_name='__hoya_sync' BEGIN SELECT RAISE(ABORT,'fixture recovery failure'); END";

// Fault: a failed C1 no-write confirmation commit and a failed redelivery persist change
// nothing in memory or on disk, and both succeed once the fault is removed.
#[test]
fn tc0042_fault_confirmation_and_redelivery_commit_atomically() {
    use hoyadb_core::{command::*, storage::*};
    let path = path("fault");
    let mut db = cases::open(SqliteStorage::open_unprotected(&path).unwrap());
    cases::run(db.insert_proposed("notes", cases::body("n0"), Some("n".into()))).unwrap();
    cases::run(db.insert_proposed("notes", cases::body("m0"), Some("m".into()))).unwrap();
    let a = cases::edit(&mut db, "n", "a", 100);
    let b = cases::edit(&mut db, "m", "b", 101);
    assert_eq!(
        cases::keys(&db.prepare_push(102, 0).unwrap()),
        vec![a.clone(), b.clone()]
    );
    db.receive(&a, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    db.receive(&b, None).unwrap();
    let status = db.queue_status();
    db.close();
    let before = ledger_bytes(&path);
    // The owner holds the file exclusively, so the fault trigger is installed and removed
    // only while no owner is open.
    let sql = |batch: &str| {
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute_batch(batch)
            .unwrap()
    };
    sql(FAIL);
    let mut db = cases::open(SqliteStorage::open_unprotected(&path).unwrap());
    assert!(db.queue_status() == status, "reopened under the fault");
    assert!(matches!(
        cases::run(db.reconcile(&a, ReconcilePlan::default())),
        Err(CommandError::Storage(StorageError::BackendFailure { .. }))
    ));
    assert!(matches!(
        db.request_redelivery(&b, 200),
        Err(CommandError::Storage(StorageError::BackendFailure { .. }))
    ));
    assert!(
        db.queue_status() == status,
        "owner state unchanged after both faults"
    );
    assert_eq!(db.pending_reconciliations().len(), 1);
    db.close();
    sql("DROP TRIGGER recovery_fail");
    assert_eq!(ledger_bytes(&path), before, "no partial write after reopen");
    let mut db = cases::open(SqliteStorage::open_unprotected(&path).unwrap());
    assert!(
        db.queue_status() == status,
        "reopened state equals pre-fault state"
    );
    cases::run(db.reconcile(&a, ReconcilePlan::default())).unwrap();
    db.request_redelivery(&b, 200).unwrap();
    assert_eq!(cases::keys(&db.prepare_push(200, 0).unwrap()), vec![b]);
    assert!(db.pending_reconciliations().is_empty());
    db.close();
    std::fs::remove_file(path).unwrap();
}
