//! TC-0041 SI-V01/SI-V02: strict command-capable open on native SQLite.
#![cfg(not(target_arch = "wasm32"))]
#[path = "../../../tests/conformance/strict_open.rs"]
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
            "hoya-strict-open-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn fresh(&self) -> impl FnMut() -> SqliteStorage + '_ {
        let mut sequence = 0;
        move || {
            sequence += 1;
            SqliteStorage::open_unprotected(self.0.join(format!("store-{sequence}.sqlite")))
                .unwrap()
        }
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("remove only the owned strict-open fixture");
    }
}

#[test]
fn si_v01_untouched_corruption_fails_exclusive_open() {
    let directory = Directory::new();
    cases::untouched_corruption_fails_open(directory.fresh());
}

#[test]
fn si_v02_closed_ledger_edit_fails_open() {
    let directory = Directory::new();
    cases::closed_ledger_edit_fails_open(directory.fresh()());
}
