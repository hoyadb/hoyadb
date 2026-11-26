//! TC-0042 C1a (DEF-0039) on SQLite. Every case, RED and control, closes its owner and reopens the
//! file through a fresh connection, which must read the identical ledger rows. The adapter is
//! Exclusive, so the default parallel runner applies.
#[path = "../../../tests/conformance/entity_promotion.rs"]
mod cases;
use hoyadb_core::storage::{Row, SyncStorage};
use hoyadb_storage_sqlite::SqliteStorage;
use std::{
    cell::RefCell,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

/// Runs a case in a directory this run creates; creation fails if the path already exists, and
/// only a directory this run created is removed afterwards (also when the case panics). `fresh`
/// opens a new store file and records its path, in order.
fn in_directory(case: impl FnOnce(&dyn Fn() -> SqliteStorage, &RefCell<Vec<PathBuf>>)) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "hoya-c1a-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).expect("a new directory owned by this run");
    let paths = RefCell::new(vec![]);
    let fresh = || {
        let path = directory.join(format!("{}.sqlite", paths.borrow().len()));
        paths.borrow_mut().push(path.clone());
        SqliteStorage::open_unprotected(&path).unwrap()
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| case(&fresh, &paths)));
    std::fs::remove_dir_all(&directory).unwrap();
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

/// After the case returns its storage, a fresh connection must read the identical ledger rows.
fn restart(storage: SqliteStorage, path: &PathBuf, rows: Vec<Row>) {
    drop(storage);
    let reopened = SqliteStorage::open_unprotected(path).unwrap();
    assert_eq!(
        reopened.load_sync().unwrap(),
        Some(rows),
        "ledger identical after restart"
    );
}

/// One store per case: restart it.
fn single(case: impl FnOnce(SqliteStorage) -> (SqliteStorage, Vec<Row>)) {
    in_directory(|fresh, paths| {
        let (storage, rows) = case(fresh());
        let path = paths.borrow()[0].clone();
        restart(storage, &path, rows);
    });
}

#[test]
fn tc0042_c1a_a_equal_clock_confirm_a_first() {
    single(cases::c1a_a);
}

#[test]
fn tc0042_c1a_b_equal_clock_confirm_e_first() {
    single(cases::c1a_b);
}

#[test]
fn tc0042_c1a_null_removal_then_null() {
    single(|storage| cases::c1a_null(storage, false));
}

#[test]
fn tc0042_c1a_null_null_then_removal() {
    single(|storage| cases::c1a_null(storage, true));
}

#[test]
fn tc0042_c1a_corrected_keeps_selected_stamp() {
    single(cases::c1a_corrected);
}

#[test]
fn tc0042_c1a_delete_promotes_matching_tombstone() {
    single(cases::c1a_delete);
}

#[test]
fn tc0042_c1a_ctrl_distinct_clocks_and_present_zero() {
    in_directory(|fresh, paths| {
        let stores = cases::c1a_ctrl(fresh);
        let paths = paths.borrow().clone();
        assert_eq!(
            stores.len(),
            paths.len(),
            "every control store is restarted"
        );
        for ((storage, rows), path) in stores.into_iter().zip(paths) {
            restart(storage, &path, rows);
        }
    });
}

#[test]
fn tc0042_c1a_no_retroactive_repair() {
    single(cases::c1a_no_repair);
}

#[test]
fn tc0042_c1a_mapping_rekeys_and_promotes_only_the_accepted_value() {
    single(cases::c1a_mapping);
}
