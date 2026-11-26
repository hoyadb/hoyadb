//! TC-0041 SI-W03 on the memory adapter.
#[path = "../../../tests/conformance/work_counters.rs"]
mod cases;
use hoyadb_storage_memory::MemoryStorage;

#[test]
fn si_y01_entity_patch_records_bounded_first_images() {
    for m in [10, 1000] {
        cases::patch_first_images(MemoryStorage::default(), m);
    }
}

#[test]
fn si_y01_entity_patch_clones_are_bounded() {
    let small = cases::patch_work(MemoryStorage::default(), 10);
    let large = cases::patch_work(MemoryStorage::default(), 1000);
    cases::assert_patch_clones_bounded(&small, &large);
}

#[test]
fn si_y01_commit_advances_epoch() {
    cases::epoch_advances(MemoryStorage::default());
}

#[test]
fn si_y01_commit_maintains_tombstone_cache() {
    cases::tombstone_cache(MemoryStorage::default());
}

#[test]
fn si_y03_receipt_counts_exact_delta_packet_images() {
    cases::receipt_packet_copies(MemoryStorage::default(), false);
}

#[test]
fn si_w03_reset_counts_both_sync_packet_images() {
    cases::reset_packet_copies(MemoryStorage::default());
}

#[test]
fn si_y09_empty_merge_retains_no_packet_inventory() {
    for m in [10, 1000] {
        cases::merge_packet_copies(MemoryStorage::default(), m, false);
    }
}

#[test]
fn si_y01_exclusive_patch_work_is_bounded_at_m() {
    let small = cases::patch_work(MemoryStorage::default(), 10);
    let large = cases::patch_work(MemoryStorage::default(), 1000);
    cases::assert_patch_bounded(&small, &large);
}

#[test]
fn si_y01_progress_reads_cached_tombstones_without_metadata_visits() {
    for m in [10, 1000] {
        let work = cases::progress_work(MemoryStorage::default(), m);
        assert_eq!(work.metadata_entries_visited, 0, "{work:?}");
    }
}

#[test]
fn si_y06_reset_keeps_full_plan_work() {
    let small = cases::reset_work(MemoryStorage::default(), 10);
    let large = cases::reset_work(MemoryStorage::default(), 1000);
    eprintln!("SI-Y06 reset M=10 {small:?}\nM=1000 {large:?}");
    assert!(small.packets_parsed > 0, "{small:?}");
    assert!(
        large.packets_parsed >= small.packets_parsed + 990,
        "reset parses the full prior ledger: {small:?} {large:?}"
    );
}

#[test]
fn si_y07_reconcile_and_inverse_read_only_touched_rows() {
    for rollback in [false, true] {
        let small = cases::resolution_work(MemoryStorage::default(), 10, rollback);
        let large = cases::resolution_work(MemoryStorage::default(), 1000, rollback);
        cases::assert_resolution_bounded(&small, &large);
    }
}

#[test]
fn si_y04_unproven_patch_keeps_full_validation_work() {
    let _owners = cases::UNPROVEN_OWNERS.lock().unwrap();
    let small = cases::patch_work(cases::Unproven(MemoryStorage::default()), 10);
    let large = cases::patch_work(cases::Unproven(MemoryStorage::default()), 1000);
    cases::assert_unproven_patch_counters_grow(&small, &large);
}

#[test]
fn si_y04_receipt_counts_both_full_packet_images() {
    let _owners = cases::UNPROVEN_OWNERS.lock().unwrap();
    cases::receipt_packet_copies(cases::Unproven(MemoryStorage::default()), true);
}

#[test]
fn si_y04_empty_merge_retains_full_packet_inventory() {
    let _owners = cases::UNPROVEN_OWNERS.lock().unwrap();
    for m in [10, 1000] {
        cases::merge_packet_copies(cases::Unproven(MemoryStorage::default()), m, true);
    }
}

#[test]
fn si_y01_cache_matches_after_rollback_reset_and_late_page_error() {
    cases::cache_lifecycle(MemoryStorage::default());
}
