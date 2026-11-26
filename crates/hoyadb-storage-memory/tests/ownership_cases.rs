//! TC-0041 SI-A01–A05: per-database admission on the real memory adapter.
#[path = "../../../tests/conformance/ownership.rs"]
mod cases;
use hoyadb_core::storage::{StoreOwnership, SyncStorage};
use hoyadb_storage_memory::MemoryStorage;

#[test]
fn si_a01_queued_request_does_not_block() {
    cases::queued_request_does_not_block(MemoryStorage::default);
}

#[test]
fn si_a02_active_handler_does_not_block() {
    cases::active_handler_does_not_block(MemoryStorage::default);
}

#[test]
fn si_a03_same_database_serialization() {
    cases::same_database_serialization(MemoryStorage::default());
}

#[test]
fn si_a04_a05_tickets_stay_with_their_owner() {
    cases::tickets_stay_with_their_owner(MemoryStorage::default);
}

#[test]
fn si_a07_owned_memory_reports_exclusive() {
    assert_eq!(
        MemoryStorage::default().ownership(),
        StoreOwnership::Exclusive
    );
}
