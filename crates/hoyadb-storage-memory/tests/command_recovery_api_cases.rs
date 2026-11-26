#[path = "../../../tests/conformance/command_recovery_api.rs"]
mod cases;
use hoyadb_storage_memory::MemoryStorage;

// Post-GREEN API-surface cases (see the shared file header); run with --test-threads=1.
#[test]
fn tc0042_awaiting_key() {
    cases::awaiting_key(MemoryStorage::default());
}

#[test]
fn tc0042_cl04_redelivery() {
    cases::cl04(MemoryStorage::default());
}
