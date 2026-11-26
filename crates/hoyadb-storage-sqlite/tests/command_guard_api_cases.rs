#[path = "../../../tests/conformance/command_guard_api.rs"]
mod cases;
use hoyadb_storage_sqlite::SqliteStorage;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

/// Each case gets its own directory, removed afterwards (also when the case panics).
fn in_directory(case: impl FnOnce(&dyn Fn() -> SqliteStorage)) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "hoya-guard-api-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir(&directory).unwrap();
    let files = AtomicUsize::new(0);
    let fresh = || -> SqliteStorage {
        let path: PathBuf =
            directory.join(format!("{}.sqlite", files.fetch_add(1, Ordering::Relaxed)));
        SqliteStorage::open_unprotected(&path).unwrap()
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| case(&fresh)));
    std::fs::remove_dir_all(&directory).unwrap();
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

// TC-0043 post-GREEN API cases on SQLite; run with --test-threads=1.
#[test]
fn tc0043_gw03() {
    in_directory(|fresh| cases::gw03(fresh));
}

#[test]
fn tc0043_gw04() {
    in_directory(|fresh| cases::gw04(fresh()));
}

#[test]
fn tc0043_gw05() {
    in_directory(|fresh| cases::gw05(fresh));
}

#[test]
fn tc0043_gw06() {
    in_directory(|fresh| cases::gw06(fresh()));
}

#[test]
fn tc0043_gw07() {
    in_directory(|fresh| cases::gw07(fresh));
}

#[test]
fn tc0043_gw10() {
    in_directory(|fresh| cases::gw10(fresh()));
}

#[test]
fn tc0043_gw11() {
    in_directory(|fresh| cases::gw11(fresh()));
}

#[test]
fn tc0043_gw12() {
    in_directory(|fresh| cases::gw12(fresh()));
}

#[test]
fn tc0043_gw12_create_delete() {
    in_directory(|fresh| cases::gw12_create_delete(fresh()));
}

// GW08: a real SQLite commit failure under a valid guarded plan changes nothing (also after
// reopen); the same plan then commits and survives restart. The owner holds the file
// exclusively, so the fault trigger is installed and dropped only while no owner is open.
#[test]
fn tc0043_gw08_guarded_commit_failure_and_restart() {
    use hoyadb_core::{command::*, storage::*};
    in_directory(|_| {
        let path =
            std::env::temp_dir().join(format!("hoya-guard-gw08-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let (db, a) = cases::corrected(SqliteStorage::open_unprotected(&path).unwrap());
        let (status, pending) = (db.queue_status(), db.pending_reconciliations());
        db.close();
        let ledger = || {
            SqliteStorage::open_unprotected(&path)
                .unwrap()
                .load_sync()
                .unwrap()
        };
        let before = ledger();
        let sql = |batch: &str| {
            rusqlite::Connection::open(&path)
                .unwrap()
                .execute_batch(batch)
                .unwrap()
        };
        sql(
            "CREATE TRIGGER guard_fail BEFORE INSERT ON __hoya_fields WHEN NEW.table_name='__hoya_sync' BEGIN SELECT RAISE(ABORT,'fixture guard failure'); END",
        );
        let plan = cases::writes(vec![cases::put("other-note", "backend")]);
        let guards = vec![cases::expect("other-note", Some("other"))];
        let mut db = cases::open(SqliteStorage::open_unprotected(&path).unwrap());
        assert!(matches!(
            cases::run(db.reconcile_guarded(&a, plan.clone(), guards.clone())),
            Err(CommandError::Storage(StorageError::BackendFailure { .. }))
        ));
        assert!(db.queue_status() == status && db.pending_reconciliations() == pending);
        db.close();
        sql("DROP TRIGGER guard_fail");
        assert_eq!(ledger(), before, "no partial write after reopen");
        let mut db = cases::open(SqliteStorage::open_unprotected(&path).unwrap());
        assert!(
            db.pending_reconciliations() == pending,
            "reopened state is the prior state"
        );
        assert_eq!(cases::run(db.reconcile_guarded(&a, plan, guards)), Ok(()));
        db.close();
        let db = cases::open(SqliteStorage::open_unprotected(&path).unwrap());
        assert_eq!(
            cases::rows(&db)
                .into_iter()
                .find(|r| r.id == "other-note")
                .unwrap()
                .fields,
            cases::body("backend")
        );
        assert!(db.pending_reconciliations().is_empty());
        db.close();
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    });
}

#[test]
fn tc0043_gw12b_typed_correction_on_a_non_entity_table() {
    in_directory(|fresh| cases::gw12b(fresh()));
}
