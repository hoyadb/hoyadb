//! TC-0041 SI-W03 on native SQLite.
#![cfg(not(target_arch = "wasm32"))]
#[path = "../../../tests/conformance/work_counters.rs"]
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
            "hoya-work-counters-{}-{}",
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
        std::fs::remove_dir_all(&self.0).expect("remove only the owned counter fixture");
    }
}

#[test]
fn si_y01_entity_patch_records_bounded_first_images() {
    let directory = Directory::new();
    for m in [10, 1000] {
        cases::patch_first_images(directory.store(&format!("first-images-{m}.sqlite")), m);
    }
}

#[test]
fn si_y01_entity_patch_clones_are_bounded() {
    let directory = Directory::new();
    let small = cases::patch_work(directory.store("bounded-10.sqlite"), 10);
    let large = cases::patch_work(directory.store("bounded-1000.sqlite"), 1000);
    cases::assert_patch_clones_bounded(&small, &large);
}

#[test]
fn si_y01_commit_advances_epoch() {
    let directory = Directory::new();
    cases::epoch_advances(directory.store("epoch.sqlite"));
}

#[test]
fn si_y01_commit_maintains_tombstone_cache() {
    let directory = Directory::new();
    cases::tombstone_cache(directory.store("cache.sqlite"));
}

#[test]
fn si_y03_receipt_counts_exact_delta_packet_images() {
    let directory = Directory::new();
    cases::receipt_packet_copies(directory.store("receipt-packets.sqlite"), false);
}

#[test]
fn si_w03_reset_counts_both_sync_packet_images() {
    let directory = Directory::new();
    cases::reset_packet_copies(directory.store("reset-packets.sqlite"));
}

#[test]
fn si_y09_empty_merge_retains_no_packet_inventory() {
    let directory = Directory::new();
    for m in [10, 1000] {
        cases::merge_packet_copies(
            directory.store(&format!("merge-packets-{m}.sqlite")),
            m,
            false,
        );
    }
}

#[test]
fn si_y01_exclusive_patch_work_is_bounded_at_m() {
    let directory = Directory::new();
    let small = cases::patch_work(directory.store("patch-10.sqlite"), 10);
    let large = cases::patch_work(directory.store("patch-1000.sqlite"), 1000);
    cases::assert_patch_bounded(&small, &large);
}

#[test]
fn si_y01_progress_reads_cached_tombstones_without_metadata_visits() {
    let directory = Directory::new();
    for m in [10, 1000] {
        let work = cases::progress_work(directory.store(&format!("progress-{m}.sqlite")), m);
        assert_eq!(work.metadata_entries_visited, 0, "{work:?}");
    }
}

#[test]
fn si_y06_reset_keeps_full_plan_work() {
    let directory = Directory::new();
    let small = cases::reset_work(directory.store("reset-10.sqlite"), 10);
    let large = cases::reset_work(directory.store("reset-1000.sqlite"), 1000);
    eprintln!("SI-Y06 reset M=10 {small:?}\nM=1000 {large:?}");
    assert!(small.packets_parsed > 0, "{small:?}");
    assert!(
        large.packets_parsed >= small.packets_parsed + 990,
        "reset parses the full prior ledger: {small:?} {large:?}"
    );
}

#[test]
fn si_y07_reconcile_and_inverse_read_only_touched_rows() {
    let directory = Directory::new();
    for rollback in [false, true] {
        let small = cases::resolution_work(
            directory.store(&format!("resolution-{rollback}-10.sqlite")),
            10,
            rollback,
        );
        let large = cases::resolution_work(
            directory.store(&format!("resolution-{rollback}-1000.sqlite")),
            1000,
            rollback,
        );
        cases::assert_resolution_bounded(&small, &large);
    }
}

#[test]
fn si_y04_unproven_patch_keeps_full_validation_work() {
    let _owners = cases::UNPROVEN_OWNERS.lock().unwrap();
    let directory = Directory::new();
    let small = cases::patch_work(cases::Unproven(directory.store("unproven-10.sqlite")), 10);
    let large = cases::patch_work(
        cases::Unproven(directory.store("unproven-1000.sqlite")),
        1000,
    );
    cases::assert_unproven_patch_counters_grow(&small, &large);
    assert!(large.sqlite_rows_written >= small.sqlite_rows_written + 990);
}

#[test]
fn si_y04_receipt_counts_both_full_packet_images() {
    let _owners = cases::UNPROVEN_OWNERS.lock().unwrap();
    let directory = Directory::new();
    cases::receipt_packet_copies(
        cases::Unproven(directory.store("receipt-full.sqlite")),
        true,
    );
}

#[test]
fn si_y04_empty_merge_retains_full_packet_inventory() {
    let _owners = cases::UNPROVEN_OWNERS.lock().unwrap();
    let directory = Directory::new();
    for m in [10, 1000] {
        cases::merge_packet_copies(
            cases::Unproven(directory.store(&format!("full-merge-{m}.sqlite"))),
            m,
            true,
        );
    }
}

#[test]
fn si_y01_cache_matches_after_rollback_reset_and_late_page_error() {
    let directory = Directory::new();
    cases::cache_lifecycle(directory.store("cache-lifecycle.sqlite"));
}
