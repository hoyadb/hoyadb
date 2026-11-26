#[path = "../../../tests/conformance/command_guard.rs"]
mod cases;
use hoyadb_storage_sqlite::SqliteStorage;
use std::path::{Path, PathBuf};

fn path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("hoya-guard-{name}-{}.sqlite", std::process::id()))
}

/// Runs a case on a fresh file and removes it afterwards, also when the case panics.
fn on_file(name: &str, case: impl FnOnce(SqliteStorage) + std::panic::UnwindSafe) {
    let path = path(name);
    let _ = std::fs::remove_file(&path);
    let storage = SqliteStorage::open_unprotected(&path).unwrap();
    let result = std::panic::catch_unwind(move || case(storage));
    remove(&path);
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

fn remove(path: &Path) {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

// TC-0043 on SQLite: one test per case, run with --test-threads=1 (shared global coordinator).
#[test]
fn tc0043_gw01() {
    on_file("gw01", cases::gw01);
}

#[test]
fn tc0043_gw02() {
    on_file("gw02", cases::gw02);
}
