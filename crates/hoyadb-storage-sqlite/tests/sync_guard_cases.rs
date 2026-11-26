//! SPEC-0039 / TC-0043 storage seam on the SQLite adapter.
#[path = "../../../tests/conformance/sync_guard_storage.rs"]
mod cases;
use hoyadb_storage_sqlite::SqliteStorage;
use std::sync::atomic::{AtomicUsize, Ordering};

fn fixture(test: impl FnOnce(SqliteStorage)) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "hoya-guard-sync-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir(&directory).unwrap();
    let path = directory.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        test(SqliteStorage::open_unprotected(path.join("store.sqlite")).unwrap())
    }));
    std::fs::remove_dir_all(directory).unwrap();
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

#[test]
fn rg1_matching_read_only_guards_commit() {
    fixture(cases::matching_read_only_guards_commit);
}

#[test]
fn rg2_mismatching_read_only_guard_is_atomic() {
    fixture(cases::mismatching_read_only_guard_is_atomic);
}

#[test]
fn rg3_guard_only_absence_expectation() {
    fixture(cases::guard_only_absence_expectation);
}

#[test]
fn rg4_unknown_table_guard_is_refused() {
    fixture(cases::unknown_table_guard_is_refused);
}

#[test]
fn rg5_mutations_still_require_guards() {
    fixture(cases::mutations_still_require_guards);
}
