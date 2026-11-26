//! TC-0041 SI-Q01: bounded SQLite ordered limits for a validated Exclusive owner.
#![cfg(not(target_arch = "wasm32"))]
#[path = "../../../tests/conformance/ordered.rs"]
mod cases;
use hoyadb_core::storage::{Direction, Operator, Predicate, Query, Value, counters};
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
            "hoya-ordered-{}-{}",
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
        std::fs::remove_dir_all(&self.0).expect("remove only the owned ordered fixture");
    }
}

fn measured(n: i64, query: &Query, directory: &Directory) -> (usize, counters::WorkSnapshot) {
    let mut db = cases::open(directory.store(&format!("measured-{n}.sqlite")));
    cases::seed(&mut db, n);
    counters::reset();
    let rows = cases::fetch(&db, query).len();
    (rows, counters::snapshot())
}

#[test]
fn si_q01_ordered_limits_match_the_oracle() {
    let directory = Directory::new();
    for n in [10, 1000] {
        cases::ordered_limits_match_the_oracle(directory.store(&format!("oracle-{n}.sqlite")), n);
    }
}

#[test]
fn si_q01_bounded_order_is_sort_free_and_equal_at_n() {
    // rank DESC: d=3 values (a tie group straddles k); rank ASC: all distinct;
    // label DESC then into, and label ASC inside, the giant "aaa" tie group.
    for (field, direction) in [
        ("rank", Direction::Desc),
        ("rank", Direction::Asc),
        ("label", Direction::Desc),
        ("label", Direction::Asc),
        ("score", Direction::Desc),
    ] {
        let query = cases::ordered(field, direction.clone(), Some(5));
        let mut work = Vec::new();
        for n in [10, 1000] {
            let directory = Directory::new();
            let (rows, snapshot) = measured(n, &query, &directory);
            assert_eq!(rows, 5);
            work.push(snapshot);
        }
        let context = format!("{field} {direction:?}: {:?}", work);
        assert_eq!(work[0].sqlite_sort, 0, "{context}");
        assert_eq!(work[0].sqlite_fullscan_steps, 0, "{context}");
        assert!(work[0].sqlite_rows_read <= 5, "{context}");
        assert_eq!(
            (work[0].sqlite_vm_steps, work[0].sqlite_rows_read),
            (work[1].sqlite_vm_steps, work[1].sqlite_rows_read),
            "{context}: work depends on N"
        );
    }
}

#[test]
fn si_q01_ineligible_shapes_keep_todays_query() {
    let directory = Directory::new();
    let filtered = Query {
        predicates: vec![Predicate {
            field: "label".into(),
            operator: Operator::Eq,
            values: vec![Value::String("aaa".into())],
        }],
        ..cases::ordered("rank", Direction::Desc, Some(5))
    };
    for query in [cases::ordered("note", Direction::Desc, Some(5)), filtered] {
        let (_, snapshot) = measured(10, &query, &directory);
        assert!(
            snapshot.sqlite_sort > 0,
            "today's CASE path sorts for {query:?}: {snapshot:?}"
        );
        drop(std::fs::remove_file(directory.0.join("measured-10.sqlite")));
    }
}

#[test]
fn si_q04_raw_seams_keep_case_for_wrong_kind_payloads() {
    let directory = Directory::new();
    cases::raw_wrong_kind_payload_sorts_as_null(directory.store("raw.sqlite"));
}

#[test]
fn si_q04_validated_reactive_windows_match_queries() {
    let directory = Directory::new();
    let mut db = cases::open(directory.store("validated.sqlite"));
    cases::seed(&mut db, 6);
    cases::reactive_windows_match_queries_after_updates(&mut db);
}

#[test]
fn si_q04_plain_reactive_windows_match_queries() {
    let directory = Directory::new();
    let mut db = cases::plain_with_wrong_kind(directory.store("plain.sqlite"));
    cases::reactive_windows_match_queries_after_updates(&mut db);
}
