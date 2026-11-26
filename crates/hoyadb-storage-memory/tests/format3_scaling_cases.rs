#![cfg(feature = "work-counters")]
#[path = "../../../tests/conformance/format3_scaling.rs"]
mod cases;
use hoyadb_storage_memory::MemoryStorage;
#[test]
fn si_y02_ten_format3_pages_have_equal_work() {
    cases::ten_pages(MemoryStorage::default());
}
#[test]
fn si_f03_format3_middle_identity_insertion_is_bounded() {
    cases::assert_middle_new(
        cases::middle(MemoryStorage::default(), 10, false),
        cases::middle(MemoryStorage::default(), 1000, false),
    );
}
#[test]
fn si_y05_seeded_v2_middle_insertion_retains_exact_suffix() {
    cases::assert_middle_legacy(
        cases::middle(MemoryStorage::default(), 10, true),
        cases::middle(MemoryStorage::default(), 1000, true),
    );
}
#[test]
fn si_f03_format3_history_and_references_survive_roundtrip() {
    cases::history_and_references(MemoryStorage::default(), false);
}
#[test]
fn si_f03_seeded_v2_history_and_references_control() {
    cases::history_and_references(MemoryStorage::default(), true);
}
