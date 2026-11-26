//! TC-0041 SI-Q04: an Unproven owner never reaches the bounded walk. One test,
//! because Unproven owners share the process-wide coordinator.
// Only the shared opener, fixture and reactive oracle are needed here.
#[allow(dead_code)]
#[path = "../../../tests/conformance/ordered.rs"]
mod cases;
#[path = "support/unproven.rs"]
mod unproven;
use hoyadb_core::storage::{Direction, counters};
use hoyadb_storage_memory::MemoryStorage;
use unproven::Unproven;

#[test]
fn si_q04_unproven_owner_keeps_todays_query_and_exact_windows() {
    let mut db = cases::open(Unproven(MemoryStorage::default()));
    cases::seed(&mut db, 20);
    counters::reset();
    assert_eq!(
        cases::fetch(&db, &cases::ordered("rank", Direction::Desc, Some(10))).len(),
        10
    );
    assert_eq!(
        counters::snapshot().memory_rows_visited,
        28,
        "an Unproven owner keeps today's full candidate scan"
    );
    cases::reactive_windows_match_queries_after_updates(&mut db);
}
