//! TC-0032: shared atomic command recovery cases on the actual memory adapter.
#[path = "../../../tests/conformance/sync_storage.rs"]
mod cases;
use hoyadb_storage_memory::MemoryStorage;

#[test]
fn c06_atomic_initialization_and_order() {
    cases::atomic_initialization_and_order(MemoryStorage::default());
}

#[test]
fn c07_ordinary_failure_is_atomic() {
    cases::ordinary_failure_preserves_absence_data_and_counter(MemoryStorage::default());
}

#[test]
fn c11_complete_cas_and_guards() {
    cases::complete_cas_and_initial_guards(MemoryStorage::default());
}

#[test]
fn c17_future_reserved_packet_rejected() {
    cases::reserved_corrupt_and_future_packets_reject(MemoryStorage::default());
}

#[test]
fn c17_live_lifecycle_guard() {
    cases::live_lifecycle_guard_and_resolved_history(MemoryStorage::default());
}

#[test]
fn c11_identity_and_frozen_guards() {
    cases::identity_and_frozen_transition_guards(MemoryStorage::default());
}

#[test]
fn c11_public_undo_model_validation() {
    cases::live_undo_uses_public_rows_and_checked_model_types(MemoryStorage::default());
}

#[test]
fn c11_live_created_table_validation() {
    cases::live_created_ownership_requires_current_model_tables(MemoryStorage::default());
}
