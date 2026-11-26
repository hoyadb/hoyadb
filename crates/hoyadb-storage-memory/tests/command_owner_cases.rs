//! TC-0032: public command owner against the real memory adapter.
#[path = "../../../tests/conformance/command_owner.rs"]
mod cases;
#[test]
fn c01_c06_c08_public_owner_contract() {
    cases::owner_contract(hoyadb_storage_memory::MemoryStorage::default);
}
