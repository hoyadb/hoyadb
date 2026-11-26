#![cfg(all(feature = "work-counters", not(target_arch = "wasm32")))]
#[path = "../../../tests/conformance/format3_scaling.rs"]
mod cases;
use hoyadb_storage_sqlite::SqliteStorage;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "hoya-format3-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self, name: &str) -> SqliteStorage {
        SqliteStorage::open_unprotected(self.0.join(name)).unwrap()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn si_y02_ten_format3_pages_have_equal_work() {
    let d = Directory::new();
    cases::ten_pages(d.store("pages.sqlite"));
}
#[test]
fn si_f03_format3_middle_identity_insertion_is_bounded() {
    let d = Directory::new();
    cases::assert_middle_new(
        cases::middle(d.store("small.sqlite"), 10, false),
        cases::middle(d.store("large.sqlite"), 1000, false),
    );
}
#[test]
fn si_y05_seeded_v2_middle_insertion_retains_exact_suffix() {
    let d = Directory::new();
    cases::assert_middle_legacy(
        cases::middle(d.store("small.sqlite"), 10, true),
        cases::middle(d.store("large.sqlite"), 1000, true),
    );
}
#[test]
fn si_f03_format3_history_and_references_survive_roundtrip() {
    let d = Directory::new();
    cases::history_and_references(d.store("history.sqlite"), false);
}
#[test]
fn si_f03_seeded_v2_history_and_references_control() {
    let d = Directory::new();
    cases::history_and_references(d.store("history.sqlite"), true);
}
