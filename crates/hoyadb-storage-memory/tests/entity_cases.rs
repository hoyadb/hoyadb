//! TC-0034: actual memory adapter, same generic assertions as SQLite.
#[path = "../../../tests/conformance/entity_sync.rs"]
mod cases;
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn e01_e03_e05_automatic_entity_contract() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    cases::automatic_entities(hoyadb_storage_memory::MemoryStorage::default);
}

#[test]
fn e02_e04_e06_entity_admission_inverse() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    cases::admission_and_inverse(hoyadb_storage_memory::MemoryStorage::default);
}

#[test]
fn e07_e10_two_clients_and_custom_resolution() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    cases::two_client_resolution(hoyadb_storage_memory::MemoryStorage::default);
}

#[test]
fn e01_e05_managed_links_and_mapped_rows() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    cases::managed_links_and_mapped_rows(hoyadb_storage_memory::MemoryStorage::default);
}

#[test]
fn e19_explicit_empty_reconfiguration() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    cases::reconfiguration_guards(hoyadb_storage_memory::MemoryStorage::default);
}
