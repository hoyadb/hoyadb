//! TC-0041 SI-Q02: bounded memory ordered limits for a validated Exclusive owner.
#[path = "../../../tests/conformance/ordered.rs"]
mod cases;
use hoyadb_core::storage::{Direction, counters};
use hoyadb_storage_memory::MemoryStorage;

#[test]
fn si_q02_ordered_limits_match_the_oracle() {
    for n in [10, 1000] {
        cases::ordered_limits_match_the_oracle(MemoryStorage::default(), n);
    }
}

#[test]
fn si_q02_limit_10_without_predicate_visits_10_rows_equal_at_n() {
    for direction in [Direction::Asc, Direction::Desc] {
        let mut work = Vec::new();
        for n in [10, 1000] {
            let mut db = cases::open(MemoryStorage::default());
            cases::seed(&mut db, n);
            let query = cases::ordered("rank", direction.clone(), Some(10));
            counters::reset();
            assert_eq!(cases::fetch(&db, &query).len(), 10);
            let snapshot = counters::snapshot();
            work.push((snapshot.memory_rows_visited, snapshot.ordered_ops));
        }
        assert_eq!(work[0].0, 10, "{direction:?}: memory_rows_visited {work:?}");
        assert!(work[0].1 <= 11, "{direction:?}: ordered_ops {work:?}");
        assert_eq!(work[0], work[1], "{direction:?}: work depends on N");
    }
}

#[test]
fn si_q04_raw_seams_keep_case_for_wrong_kind_payloads() {
    cases::raw_wrong_kind_payload_sorts_as_null(MemoryStorage::default());
}

#[test]
fn si_q04_validated_reactive_windows_match_queries() {
    let mut db = cases::open(MemoryStorage::default());
    cases::seed(&mut db, 6);
    cases::reactive_windows_match_queries_after_updates(&mut db);
}

#[test]
fn si_q04_plain_reactive_windows_match_queries() {
    let mut db = cases::plain_with_wrong_kind(MemoryStorage::default());
    cases::reactive_windows_match_queries_after_updates(&mut db);
}
