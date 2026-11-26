#[path = "../../../tests/conformance/command_clock.rs"]
mod cases;

// TC-0039 CS01/CS02/CS08/CS09 on the memory adapter, sequential because owners share
// the process-global coordinator. CS03 (restart) and CS07 (injected commit failure)
// need persistence and fault injection, which this adapter does not provide; they run
// on SQLite.
#[test]
fn tc0039_clock_recovery() {
    cases::corrected_jump(hoyadb_storage_memory::MemoryStorage::default());
    cases::backward_sample(hoyadb_storage_memory::MemoryStorage::default());
}
