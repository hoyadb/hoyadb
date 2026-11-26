#[path = "../../../tests/conformance/entity_aliases.rs"]
mod cases;
fn fixture(test: impl FnOnce(&dyn Fn() -> hoyadb_storage_sqlite::SqliteStorage)) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "hoya-entity-alias-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).unwrap();
    let index = std::cell::Cell::new(0);
    test(&|| {
        index.set(index.get() + 1);
        hoyadb_storage_sqlite::SqliteStorage::open_unprotected(
            directory.join(format!("{}.sqlite", index.get())),
        )
        .unwrap()
    });
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn a01_a02_atomic_aliases_and_recovery() {
    fixture(|fresh| cases::atomic_and_recovery(fresh));
}
#[test]
fn a03_a04_alias_failure_is_atomic() {
    fixture(|fresh| cases::rejection_is_atomic(fresh));
}
#[test]
fn a07_partial_batch_and_marker_are_atomic() {
    fixture(|fresh| cases::partial_batches(fresh));
}
#[test]
fn a07_patches_read_preceding_staged_writes() {
    fixture(|fresh| cases::staged_patch_order(fresh));
}
