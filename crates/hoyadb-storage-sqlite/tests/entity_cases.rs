//! TC-0034: actual SQLite adapter, no copied owner/queue algorithms.
#[path = "../../../tests/conformance/entity_sync.rs"]
mod cases;
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn e01_e03_e05_automatic_entity_contract() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let directory = std::env::temp_dir().join(format!("hoya-entity-sync-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    cases::automatic_entities(|| {
        hoyadb_storage_sqlite::SqliteStorage::open_unprotected(directory.join("store.sqlite"))
            .unwrap()
    });
    std::fs::remove_dir_all(directory).unwrap();
}

fn fresh_fixture(test: impl FnOnce(Box<dyn FnMut() -> hoyadb_storage_sqlite::SqliteStorage>)) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "hoya-entity-shared-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).unwrap();
    let root = directory.clone();
    let mut n = 0;
    test(Box::new(move || {
        n += 1;
        hoyadb_storage_sqlite::SqliteStorage::open_unprotected(root.join(format!("{n}.sqlite")))
            .unwrap()
    }));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn e02_e04_e06_entity_admission_inverse() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    fresh_fixture(|fresh| {
        let fresh = std::cell::RefCell::new(fresh);
        cases::admission_and_inverse(|| fresh.borrow_mut()());
    });
}

#[test]
fn e07_e10_two_clients_and_custom_resolution() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    fresh_fixture(cases::two_client_resolution);
}

#[test]
fn e01_e05_managed_links_and_mapped_rows() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    fresh_fixture(cases::managed_links_and_mapped_rows);
}

#[test]
fn e19_explicit_empty_reconfiguration() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    fresh_fixture(|fresh| {
        let fresh = std::cell::RefCell::new(fresh);
        cases::reconfiguration_guards(|| fresh.borrow_mut()());
    });
}
