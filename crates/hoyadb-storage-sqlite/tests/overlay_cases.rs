//! TC-0041 SI-O01/SI-O02 on native SQLite.
#![cfg(not(target_arch = "wasm32"))]
#[path = "../../../tests/conformance/overlay.rs"]
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
            "hoya-overlay-{}-{}",
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
        std::fs::remove_dir_all(&self.0).expect("remove only the owned overlay fixture");
    }
}

#[test]
fn si_o01_one_row_handler_is_equal_at_n_fullscan_free() {
    let directory = Directory::new();
    let work: Vec<_> = [10, 1000]
        .into_iter()
        .map(|filler| {
            cases::one_row_handler_work(directory.store(&format!("o01-{filler}.sqlite")), filler)
        })
        .collect();
    let context = format!("{work:?}");
    assert!(work[0].rows_cloned <= 3, "{context}");
    assert_eq!(work[0].inventory_rows_validated, 0, "{context}");
    assert_eq!(work[0].sqlite_fullscan_steps, 0, "{context}");
    assert_eq!(work[0], work[1], "work depends on N: {context}");
}

#[test]
fn si_o02_overlay_transcript_equals_snapshot_staging() {
    let directory = Directory::new();
    let lines = cases::handler_script(directory.store("script.sqlite"));
    cases::assert_pinned("sqlite", &lines);
}

#[test]
fn si_o02_predicate_free_overlay_read_uses_the_bounded_walk() {
    let directory = Directory::new();
    let mut runs = Vec::new();
    for filler in [10, 1000] {
        runs.push(cases::fast_path_overlay_read(
            directory.store(&format!("fast-{filler}.sqlite")),
            filler,
        ));
    }
    assert_eq!(runs[0].0, ["o4", "o3", "o2"]);
    assert_eq!(runs[0].1.sqlite_sort, 0, "{runs:?}");
    assert_eq!(runs[0].1.sqlite_fullscan_steps, 0, "{runs:?}");
    assert_eq!(runs[0], runs[1], "work depends on N: {runs:?}");
}

#[test]
fn si_o02_self_relation_scope_queries_equal_the_committed_store() {
    let directory = Directory::new();
    // Node names; a null parent sorts lowest, then ascending id (local ids before t, u).
    let expected: [&[&str]; 4] = [&["A"], &["C", "U"], &["B", "T", "A", "C", "U"], &["B", "T"]];
    assert_eq!(
        cases::self_relation_scope_queries(directory.store("self-relation.sqlite")),
        expected
    );
}
