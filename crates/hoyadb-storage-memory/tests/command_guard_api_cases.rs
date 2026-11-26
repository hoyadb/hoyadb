#[path = "../../../tests/conformance/command_guard_api.rs"]
mod cases;
use hoyadb_storage_memory::MemoryStorage;

// TC-0043 post-GREEN API cases on memory; run with --test-threads=1.
#[test]
fn tc0043_gw03() {
    cases::gw03(MemoryStorage::default);
}

#[test]
fn tc0043_gw04() {
    cases::gw04(MemoryStorage::default());
}

#[test]
fn tc0043_gw05() {
    cases::gw05(MemoryStorage::default);
}

#[test]
fn tc0043_gw06() {
    cases::gw06(MemoryStorage::default());
}

#[test]
fn tc0043_gw07() {
    cases::gw07(MemoryStorage::default);
}

#[test]
fn tc0043_gw10() {
    cases::gw10(MemoryStorage::default());
}

#[test]
fn tc0043_gw11() {
    cases::gw11(MemoryStorage::default());
}

#[test]
fn tc0043_gw12() {
    cases::gw12(MemoryStorage::default());
}

#[test]
fn tc0043_gw12_create_delete() {
    cases::gw12_create_delete(MemoryStorage::default());
}

#[test]
fn tc0043_gw12b_typed_correction_on_a_non_entity_table() {
    cases::gw12b(MemoryStorage::default());
}
