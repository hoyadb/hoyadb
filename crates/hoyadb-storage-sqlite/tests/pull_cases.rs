//! TC-0034: same production pull/reset assertions against real SQLite.
#[path = "../../../tests/conformance/entity_pull.rs"]
mod cases;
use hoyadb_storage_sqlite::SqliteStorage;
use std::sync::atomic::{AtomicUsize, Ordering};
fn fixture(test: impl FnOnce(SqliteStorage)) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "hoya-entity-pull-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).unwrap();
    test(SqliteStorage::open_unprotected(directory.join("store.sqlite")).unwrap());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn e11_e19_atomic_pull_reset() {
    fixture(cases::pull_atomicity_bootstrap_and_reset);
}

#[test]
fn e17_reset_total_inventory() {
    fixture(cases::reset_inventory);
}

#[test]
fn e19_lifecycle_comparison_guard() {
    fixture(cases::lifecycle_comparison_guard);
}

#[test]
fn e14_live_recovery_pull_protection() {
    fixture(cases::pull_recovery_protection);
}

#[test]
fn e12_e18_real_commit_failure_and_reopen_preserve_page_and_reset() {
    let _serial = cases::OWNERS.lock().unwrap();
    use hoyadb_core::{command::*, storage::*};
    use rusqlite::Connection;
    let directory = std::env::temp_dir().join(format!("hoya-pull-fault-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("store.sqlite");
    // SPEC-0037 R2.3: the owner holds the file, so install the guarded fault through
    // the store before the owner takes it (open accepts only the exact storage
    // tables). The owner connection enables foreign_keys at open; a deferred
    // violation fails COMMIT itself, and only while this fixture arms it.
    let store = SqliteStorage::open_unprotected(&path).unwrap();
    for sql in [
        "CREATE TABLE pull_parent(id INTEGER PRIMARY KEY)",
        "CREATE TABLE pull_child(parent INTEGER REFERENCES pull_parent(id) DEFERRABLE INITIALLY DEFERRED)",
        "CREATE TRIGGER pull_commit_fault AFTER INSERT ON __hoya_rows WHEN NEW.table_name='__hoya_sync' AND hoya_test_fault('pull_commit_fault') BEGIN INSERT INTO pull_child(parent) VALUES(1); END",
    ] {
        store.execute_test_sql(sql).unwrap();
    }
    let mut db = cases::configured(store);
    let progress = db.observe_progress().unwrap();
    let query = db.query("items");
    let data = cases::run(db.observe(&query)).unwrap();
    db.drain_events(0);
    let request = db.begin_pull().unwrap().request;
    let fault = hoyadb_storage_sqlite::test_hooks::arm(&path, "pull_commit_fault");
    let page = cases::page(
        "page:1",
        false,
        vec![cases::change("items", "remote", Some("first"))],
    );
    assert!(matches!(
        cases::run(db.apply_pull(request, page.clone())),
        Err(CommandError::Storage(StorageError::BackendFailure { .. }))
    ));
    assert_eq!(db.sync_progress().pages, 0);
    assert!(cases::run(db.fetch(&query)).unwrap().is_empty());
    assert!(db.drain_events(0).is_empty());
    drop(fault);
    cases::run(db.apply_pull(request, page)).unwrap();
    assert_eq!(db.sync_progress().pages, 1);
    let events = db.drain_events(0);
    assert!(
        events.iter().any(
            |e| matches!(e,CommandEvent::Progress {subscription,..} if *subscription==progress)
        )
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e,CommandEvent::Data(event) if event.subscription==data))
    );
    let reset_ticket = db.begin_pull().unwrap().request;
    let fault = hoyadb_storage_sqlite::test_hooks::arm(&path, "pull_commit_fault");
    assert!(matches!(
        cases::run(db.reset(ResetMode::PreserveRecovery)),
        Err(CommandError::Storage(StorageError::BackendFailure { .. }))
    ));
    assert_eq!(db.sync_progress().pages, 1);
    assert_eq!(cases::run(db.fetch(&query)).unwrap().len(), 1);
    assert!(db.drain_events(0).is_empty());
    db.abort_pull(reset_ticket).unwrap();
    drop(fault);
    drop(db);
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "DROP TRIGGER pull_commit_fault;DROP TABLE pull_child;DROP TABLE pull_parent;",
        )
        .unwrap();
    let mut reopened = cases::run(CommandDatabase::open(
        SqliteStorage::open_unprotected(&path).unwrap(),
        cases::schema(),
        vec![],
        [0; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    assert_eq!(reopened.sync_progress().pages, 1);
    assert!(!reopened.sync_progress().ready);
    let request = reopened.begin_pull().unwrap();
    assert_eq!(request.since, Some("page:1".into()));
    cases::run(reopened.apply_pull(request.request, cases::page("page:2", true, vec![]))).unwrap();
    assert!(reopened.sync_progress().ready);
    cases::run(reopened.reset(ResetMode::PreserveRecovery)).unwrap();
    assert!(reopened.drain_events(0).is_empty());
    assert!(!reopened.unsubscribe_sync(progress));
    assert!(!reopened.unsubscribe(data));
    drop(reopened);
    // SPEC-0037 R1: inspect the file before its next owner opens it.
    let physical = Connection::open(&path)
        .unwrap()
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
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
    let store = SqliteStorage::open_unprotected(&path).unwrap();
    assert!(cases::run(store.scan("items")).unwrap().is_empty());
    drop(store);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn e13_progress_transition_guard() {
    fixture(cases::progress_transition_guard);
}

#[test]
fn e16_e18_reset_modes_and_fifo_guard() {
    static NEXT_RESET: AtomicUsize = AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!("hoya-reset-mode-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    cases::reset_modes_and_coordinator_guard(|| {
        SqliteStorage::open_unprotected(directory.join(format!(
            "store-{}.sqlite",
            NEXT_RESET.fetch_add(1, Ordering::Relaxed)
        )))
        .unwrap()
    });
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn e06_typed_correction_clocks() {
    fixture(cases::typed_correction_clocks);
}

#[test]
fn e19_reconfigure_actual_inventory() {
    fixture(cases::reconfigure_inventory);
}

#[test]
fn e14_pulled_tombstone_cannot_occupy_a_pending_mapping_target() {
    fixture(cases::pull_tombstone_mapping_target);
}

#[test]
fn e14_rolled_back_sent_cancellation_does_not_block_pulled_insertion() {
    fixture(cases::pull_after_rolled_back_sent_cancellation);
}
