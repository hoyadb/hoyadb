#[path = "../../../tests/conformance/si_y03_adapter_characterization.rs"]
mod cases;
use hoyadb_core::storage::StoreOwnership;
use hoyadb_storage_memory::MemoryStorage;
#[test]
fn untouched_exclusive() {
    cases::untouched_exclusive(MemoryStorage::default);
}
#[test]
fn untouched_unproven() {
    cases::untouched_unproven(MemoryStorage::default);
}
#[test]
fn guarded_exclusive() {
    cases::guarded_races(MemoryStorage::default, StoreOwnership::Exclusive);
}
#[test]
fn guarded_unproven() {
    cases::guarded_races(MemoryStorage::default, StoreOwnership::Unproven);
}

#[test]
fn default_delta_matches_override() {
    cases::default_delta_matches_override(MemoryStorage::default);
}
#[test]
fn default_delta_rejects_valid_untouched_race() {
    cases::default_delta_rejects_valid_untouched_race(MemoryStorage::default);
}
