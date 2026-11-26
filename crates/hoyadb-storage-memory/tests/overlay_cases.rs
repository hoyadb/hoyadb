//! TC-0041 SI-O01/SI-O02 on the memory adapter.
#[path = "../../../tests/conformance/overlay.rs"]
mod cases;
#[path = "support/unproven.rs"]
mod unproven;
use hoyadb_storage_memory::MemoryStorage;
use unproven::Unproven;

#[test]
fn si_o01_one_row_handler_is_equal_at_n_without_inventory_validation() {
    let work: Vec<_> = [10, 1000]
        .into_iter()
        .map(|filler| cases::one_row_handler_work(MemoryStorage::default(), filler))
        .collect();
    let context = format!("{work:?}");
    assert!(work[0].rows_cloned <= 3, "{context}");
    assert_eq!(work[0].inventory_rows_validated, 0, "{context}");
    assert_eq!(work[0], work[1], "work depends on N: {context}");
}

// The Unproven twin uses snapshot staging and the process-wide coordinator; it
// runs inside this one test only.
#[test]
fn si_o02_overlay_transcript_equals_snapshot_staging() {
    let exclusive = cases::handler_script(MemoryStorage::default());
    let snapshot = cases::handler_script(Unproven(MemoryStorage::default()));
    assert_eq!(exclusive, snapshot);
    cases::assert_pinned("memory", &exclusive);
}

#[test]
fn si_o02_predicate_free_overlay_read_uses_the_bounded_walk() {
    let mut runs = Vec::new();
    for filler in [10, 1000] {
        runs.push(cases::fast_path_overlay_read(
            MemoryStorage::default(),
            filler,
        ));
    }
    // o4 (650) enters, o1 (500) is removed, o6 (600 -> 10) leaves the window.
    assert_eq!(runs[0].0, ["o4", "o3", "o2"]);
    assert_eq!(runs[0], runs[1], "work depends on N: {runs:?}");
    assert!(
        runs[0].1.ordered_ops > 0,
        "the base window took the index walk: {runs:?}"
    );
    // The base walk reads k + |D ∪ R| = 3 + 3 rows; the overlay engine reads
    // the 2 surviving staged images.
    assert_eq!(runs[0].1.memory_rows_visited, 3 + 3 + 2, "{runs:?}");
}

#[test]
fn si_o02_self_relation_scope_queries_equal_the_committed_store() {
    // Node names; a null parent sorts lowest, then ascending id (local ids before t, u).
    let expected: [&[&str]; 4] = [&["A"], &["C", "U"], &["B", "T", "A", "C", "U"], &["B", "T"]];
    assert_eq!(
        cases::self_relation_scope_queries(MemoryStorage::default()),
        expected
    );
    assert_eq!(
        cases::self_relation_scope_queries(Unproven(MemoryStorage::default())),
        expected
    );
}
