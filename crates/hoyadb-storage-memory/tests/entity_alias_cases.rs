#[path = "../../../tests/conformance/entity_aliases.rs"]
mod cases;
#[test]
fn a01_a02_atomic_aliases_and_recovery() {
    cases::atomic_and_recovery(hoyadb_storage_memory::MemoryStorage::default);
}
#[test]
fn a03_a04_alias_failure_is_atomic() {
    cases::rejection_is_atomic(hoyadb_storage_memory::MemoryStorage::default);
}
#[test]
fn a07_partial_batch_and_marker_are_atomic() {
    cases::partial_batches(hoyadb_storage_memory::MemoryStorage::default);
}
#[test]
fn a07_patches_read_preceding_staged_writes() {
    cases::staged_patch_order(hoyadb_storage_memory::MemoryStorage::default);
}
