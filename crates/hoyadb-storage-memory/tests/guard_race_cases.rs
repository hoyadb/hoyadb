//! TC-0043 GW14, GW05 (identical Put) and GW17 on memory. The racing wrapper is Unproven
//! (global coordinator); the shared fixture isolates whole cases under the default runner.
#[path = "../../../tests/conformance/guard_race.rs"]
mod cases;
use hoyadb_storage_memory::MemoryStorage;

#[test]
fn tc0043_gw14_no_op_expectations_close_the_unproven_race() {
    cases::gw14(MemoryStorage::default);
}

#[test]
fn tc0043_gw05_identical_put_is_expectation_only() {
    cases::gw05_identical_put_is_expectation_only(MemoryStorage::default);
}

#[test]
fn tc0043_gw17_only_the_exact_guard_conflict_is_translated() {
    cases::gw17(MemoryStorage::default);
}
