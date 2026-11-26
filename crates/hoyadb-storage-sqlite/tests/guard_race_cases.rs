//! TC-0043 GW14, GW05 (identical Put) and GW17 on SQLite. The racing wrapper is Unproven
//! (global coordinator); the shared fixture isolates whole cases under the default runner.
#[path = "../../../tests/conformance/guard_race.rs"]
mod cases;
use hoyadb_storage_sqlite::SqliteStorage;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Fresh files in one directory per test, removed afterwards (also when the case panics).
fn in_directory(name: &str, case: impl FnOnce(&dyn Fn() -> SqliteStorage)) {
    let directory =
        std::env::temp_dir().join(format!("hoya-guard-race-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir(&directory).unwrap();
    let next = AtomicUsize::new(0);
    let fresh = || {
        let file = directory.join(format!("{}.sqlite", next.fetch_add(1, Ordering::Relaxed)));
        SqliteStorage::open_unprotected(file).unwrap()
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| case(&fresh)));
    std::fs::remove_dir_all(&directory).unwrap();
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

#[test]
fn tc0043_gw14_no_op_expectations_close_the_unproven_race() {
    in_directory("gw14", |fresh| cases::gw14(fresh));
}

#[test]
fn tc0043_gw05_identical_put_is_expectation_only() {
    in_directory("gw05", |fresh| {
        cases::gw05_identical_put_is_expectation_only(fresh)
    });
}

#[test]
fn tc0043_gw17_only_the_exact_guard_conflict_is_translated() {
    in_directory("gw17", |fresh| cases::gw17(fresh));
}
