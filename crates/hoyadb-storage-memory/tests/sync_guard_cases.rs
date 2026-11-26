//! SPEC-0039 / TC-0043 storage seam on the memory adapter.
#[path = "../../../tests/conformance/sync_guard_storage.rs"]
mod cases;
use hoyadb_storage_memory::MemoryStorage;

#[test]
fn rg1_matching_read_only_guards_commit() {
    cases::matching_read_only_guards_commit(MemoryStorage::default());
}

#[test]
fn rg2_mismatching_read_only_guard_is_atomic() {
    cases::mismatching_read_only_guard_is_atomic(MemoryStorage::default());
}

#[test]
fn rg3_guard_only_absence_expectation() {
    cases::guard_only_absence_expectation(MemoryStorage::default());
}

#[test]
fn rg4_unknown_table_guard_is_refused() {
    cases::unknown_table_guard_is_refused(MemoryStorage::default());
}

#[test]
fn rg5_mutations_still_require_guards() {
    cases::mutations_still_require_guards(MemoryStorage::default());
}
