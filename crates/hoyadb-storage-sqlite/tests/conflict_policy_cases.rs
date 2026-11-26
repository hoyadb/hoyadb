#[path = "../../../tests/conformance/conflict_policy.rs"]
mod cases;
use hoyadb_storage_sqlite::SqliteStorage;
#[path = "../../../tests/conformance/conflict_policy_race.rs"]
mod race;

fn path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("hoya-policy-{name}-{}.sqlite", std::process::id()))
}

fn fresh(name: &str) -> SqliteStorage {
    let path = path(name);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
    SqliteStorage::open_unprotected(path).unwrap()
}

#[test]
fn tc0044_cp04() {
    cases::cp04(fresh("cp04"));
}
#[test]
fn tc0044_cp06() {
    cases::cp06(fresh("cp06"));
}
#[test]
fn tc0044_cp01_02_03_configuration() {
    cases::configuration(fresh("configuration"));
}
#[test]
fn tc0044_cp05_10_correction_guards() {
    cases::correction_guards(fresh("guards"));
}
#[test]
fn tc0044_cp08_16_cancel() {
    cases::later_work_cancel(fresh("cancel"));
}
#[test]
fn tc0044_cp15a_b_fresh_connection_history() {
    for history in [1, 100] {
        for sent in [false, true] {
            let name = format!("history-{history}-{sent}");
            cases::history(fresh(&name), history, sent, |s| {
                drop(s);
                SqliteStorage::open_unprotected(path(&name)).unwrap()
            });
        }
    }
}
#[test]
fn tc0044_cp15d_e_fresh_connection_dependent() {
    for mapping in [true, false] {
        let name = format!("dependent-{mapping}");
        cases::dependent_only(fresh(&name), mapping, |s| {
            drop(s);
            SqliteStorage::open_unprotected(path(&name)).unwrap()
        });
    }
}
#[test]
fn tc0044_cp07_in_flight() {
    cases::in_flight(fresh("flight"));
}
#[test]
fn tc0044_cp09_entity_refusal() {
    cases::entity_refusal(fresh("entity"));
}
#[test]
fn tc0044_cp11_report_limit() {
    cases::report_limit(fresh("limit"));
}
#[test]
fn tc0044_cp15c_mapped_fresh_connection() {
    cases::mapped_history(fresh("mapped"), |s| {
        drop(s);
        SqliteStorage::open_unprotected(path("mapped")).unwrap()
    });
}
#[test]
fn tc0044_cp10_commit_guard_race() {
    race::cp10(fresh("race"));
}
#[test]
fn tc0044_cp06_transitive_cancelled_closure() {
    cases::transitive(fresh("transitive"));
}

/// CP12: a real SQLite abort must preserve the whole closure, owner and reopened ledger.
#[test]
fn tc0044_cp12_transaction_failure_and_reopen() {
    use cases::*;
    use hoyadb_core::{command::*, storage::*};
    let mut db = open(fresh("fault"));
    let (key, row) = enqueue(&mut db, "root");
    child(&mut db, &row.id).unwrap();
    let kept = rows(&db);
    let status = db.queue_status();
    let store = db.into_storage();
    let before = store.load_sync().unwrap();
    drop(store);
    let sql = |text: &str| {
        rusqlite::Connection::open(path("fault"))
            .unwrap()
            .execute_batch(text)
            .unwrap();
    };
    sql(
        "CREATE TRIGGER policy_fail BEFORE INSERT ON __hoya_fields WHEN NEW.table_name='__hoya_sync' BEGIN SELECT RAISE(ABORT,'task owned policy fixture'); END",
    );
    let mut db = open(SqliteStorage::open_unprotected(path("fault")).unwrap());
    db.observe_queue_status().unwrap();
    db.drain_events(100);
    assert!(matches!(
        run(db.discard(&key)),
        Err(CommandError::Storage(StorageError::BackendFailure { .. }))
    ));
    assert_eq!(rows(&db), kept);
    assert_eq!(db.queue_status(), status);
    assert!(db.drain_events(100).is_empty());
    let store = db.into_storage();
    assert_eq!(store.load_sync().unwrap(), before);
    drop(store);
    sql("DROP TRIGGER policy_fail");
    let mut db = open(SqliteStorage::open_unprotected(path("fault")).unwrap());
    assert_eq!(db.queue_status(), status);
    assert_eq!(rows(&db), kept);
    run(db.discard(&key)).unwrap();
    db.close();
    let db = open(SqliteStorage::open_unprotected(path("fault")).unwrap());
    assert_eq!(db.queue_status().live, 0);
    assert!(
        db.queue_status()
            .commands
            .iter()
            .all(|r| r.state == SyncState::Discarded && !r.rolled_back)
    );
    assert_eq!(rows(&db), kept);
    db.close();
}
