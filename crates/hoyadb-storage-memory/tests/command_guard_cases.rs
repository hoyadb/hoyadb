#[path = "../../../tests/conformance/command_guard.rs"]
mod cases;
use hoyadb_storage_memory::MemoryStorage;

// TC-0043 on memory: one test per case, run with --test-threads=1 (shared global coordinator).
#[test]
fn tc0043_gw01() {
    cases::gw01(MemoryStorage::default());
}

#[test]
fn tc0043_gw02() {
    cases::gw02(MemoryStorage::default());
}
