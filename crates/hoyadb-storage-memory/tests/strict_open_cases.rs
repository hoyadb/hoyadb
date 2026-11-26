//! TC-0041 SI-V01-SI-V03: strict command-capable open on memory, Exclusive and Unproven.
#[path = "../../../tests/conformance/strict_open.rs"]
mod cases;
#[path = "support/unproven.rs"]
mod unproven;
use hoyadb_core::{command::Json, storage::counters};
use hoyadb_storage_memory::MemoryStorage;
use std::collections::BTreeMap;
use unproven::Unproven;

#[test]
fn si_v01_untouched_corruption_fails_exclusive_open() {
    cases::untouched_corruption_fails_open(MemoryStorage::default);
}

#[test]
fn si_v02_closed_ledger_edit_fails_open() {
    cases::closed_ledger_edit_fails_open(MemoryStorage::default());
}

// Unproven owners share the process-wide coordinator, so these run in one test.
#[test]
fn si_v01_v03_unproven_open_and_conservative_begin() {
    cases::untouched_corruption_fails_open(|| Unproven(MemoryStorage::default()));
    let (store, _) = cases::seeded(Unproven(MemoryStorage::default()));
    counters::reset();
    let mut db = cases::open(store).unwrap();
    assert_eq!(
        counters::snapshot().inventory_rows_validated,
        3,
        "SI-V03: open validates the full inventory once"
    );
    counters::reset();
    let request = db.request().unwrap();
    assert!(
        cases::run(db.begin(
            request,
            cases::definition(),
            Json::Object(BTreeMap::new()),
            Some(Json::Null),
            20,
        ))
        .unwrap()
    );
    let work = counters::snapshot();
    assert_eq!(
        work.inventory_rows_validated, 3,
        "SI-V03: begin validates all rows"
    );
    assert_eq!(work.rows_cloned, 3, "SI-V03: begin snapshots all rows");
    db.abort(request).unwrap();
}
