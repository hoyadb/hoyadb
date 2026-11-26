#[path = "../../../tests/conformance/command_recovery.rs"]
mod cases;
use hoyadb_core::storage::*;
use hoyadb_storage_sqlite::SqliteStorage;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn fresh() -> SqliteStorage {
    let path = path();
    let _ = std::fs::remove_file(&path);
    SqliteStorage::open_unprotected(&path).unwrap()
}

fn path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "hoya-recovery-{}-{}.sqlite",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

// TC-0042 on SQLite: one test per case, run with --test-threads=1 (shared global coordinator).
// M1 additionally reopens its file through a new connection.
#[test]
fn tc0042_m1_with_real_reopen() {
    let m1_path = path();
    let (storage, rows, ledger) = cases::m1(SqliteStorage::open_unprotected(&m1_path).unwrap());
    drop(storage);
    let reopened = SqliteStorage::open_unprotected(&m1_path).unwrap();
    assert_eq!(
        reopened.load_sync().unwrap(),
        ledger,
        "M1 ledger after a real reopen"
    );
    let db = cases::open(reopened);
    let mut actual = cases::run(db.query("notes").fetch(&db)).unwrap();
    actual.extend(cases::run(db.query("links").fetch(&db)).unwrap());
    assert_eq!(actual, rows, "M1 rows after a real reopen");
    db.close();
    let _ = std::fs::remove_file(m1_path);
}

#[test]
fn tc0042_r1_ctrl() {
    cases::r1_ctrl(fresh);
}

#[test]
fn tc0042_m2() {
    cases::m2(fresh());
}

#[test]
fn tc0042_m3() {
    cases::m3(fresh());
}

#[test]
fn tc0042_m4() {
    cases::m4(fresh());
}

#[test]
fn tc0042_rd1() {
    cases::rd1(fresh);
}

#[test]
fn tc0042_rd2() {
    cases::rd2(fresh());
}

#[test]
fn tc0042_rd3() {
    cases::rd3(fresh());
}

#[test]
fn tc0042_x1() {
    cases::x1(fresh());
}

#[test]
fn tc0042_x2() {
    cases::x2(fresh());
}

#[test]
fn tc0042_ctrl_k() {
    cases::ctrl_k(fresh());
}

#[test]
fn tc0042_et1() {
    cases::et1(fresh);
}

#[test]
fn tc0042_et2() {
    cases::et2(fresh());
}

#[test]
fn tc0042_et3() {
    cases::et3(fresh());
}

#[test]
fn tc0042_et4() {
    cases::et4(fresh());
}
