#[path = "../../../tests/conformance/platform_core.rs"]
mod cases;

#[test]
fn g07_g18_g22_core_seams() {
    cases::proposed_links_and_recovery(hoyadb_storage_memory::MemoryStorage::default);
    cases::passive_merge(hoyadb_storage_memory::MemoryStorage::default);
    cases::normalization(hoyadb_storage_memory::MemoryStorage::default());
}
