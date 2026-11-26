//! TC-0032: shared recovery cases and physical failure/reopen on actual SQLite.
#[path = "../../../tests/conformance/sync_storage.rs"]
mod cases;
use hoyadb_storage_sqlite::SqliteStorage;
use std::sync::atomic::{AtomicUsize, Ordering};

fn fixture(test: impl FnOnce(SqliteStorage)) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "hoya-sync-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).unwrap();
    test(SqliteStorage::open_unprotected(directory.join("store.sqlite")).unwrap());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn c06_atomic_initialization_and_order() {
    fixture(cases::atomic_initialization_and_order);
}

#[test]
fn c07_ordinary_failure_is_atomic() {
    fixture(cases::ordinary_failure_preserves_absence_data_and_counter);
}

#[test]
fn c11_complete_cas_and_guards() {
    fixture(cases::complete_cas_and_initial_guards);
}

#[test]
fn c17_future_reserved_packet_rejected() {
    fixture(cases::reserved_corrupt_and_future_packets_reject);
}

#[test]
fn c17_live_lifecycle_guard() {
    fixture(cases::live_lifecycle_guard_and_resolved_history);
}

#[test]
fn c11_identity_and_frozen_guards() {
    fixture(cases::identity_and_frozen_transition_guards);
}

#[test]
fn c11_public_undo_model_validation() {
    fixture(cases::live_undo_uses_public_rows_and_checked_model_types);
}

#[test]
fn c11_live_created_table_validation() {
    fixture(cases::live_created_ownership_requires_current_model_tables);
}
