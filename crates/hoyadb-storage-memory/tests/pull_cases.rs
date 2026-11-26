#[path = "../../../tests/conformance/entity_pull.rs"]
mod cases;
#[test]
fn e11_e19_atomic_pull_reset() {
    cases::pull_atomicity_bootstrap_and_reset(hoyadb_storage_memory::MemoryStorage::default());
}

#[test]
fn e17_reset_total_inventory() {
    cases::reset_inventory(hoyadb_storage_memory::MemoryStorage::default());
}

#[test]
fn e19_lifecycle_comparison_guard() {
    cases::lifecycle_comparison_guard(hoyadb_storage_memory::MemoryStorage::default());
}

#[test]
fn e14_live_recovery_pull_protection() {
    cases::pull_recovery_protection(hoyadb_storage_memory::MemoryStorage::default());
}

#[test]
fn e13_progress_transition_guard() {
    cases::progress_transition_guard(hoyadb_storage_memory::MemoryStorage::default());
}

#[test]
fn e16_e18_reset_modes_and_fifo_guard() {
    cases::reset_modes_and_coordinator_guard(hoyadb_storage_memory::MemoryStorage::default);
}

#[test]
fn e06_typed_correction_clocks() {
    cases::typed_correction_clocks(hoyadb_storage_memory::MemoryStorage::default());
}

#[test]
fn e19_reconfigure_actual_inventory() {
    cases::reconfigure_inventory(hoyadb_storage_memory::MemoryStorage::default());
}

#[test]
fn e14_pulled_tombstone_cannot_occupy_a_pending_mapping_target() {
    cases::pull_tombstone_mapping_target(hoyadb_storage_memory::MemoryStorage::default());
}

#[test]
fn e14_rolled_back_sent_cancellation_does_not_block_pulled_insertion() {
    cases::pull_after_rolled_back_sent_cancellation(hoyadb_storage_memory::MemoryStorage::default());
}
