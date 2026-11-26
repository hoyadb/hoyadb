#[path = "../../../tests/conformance/command_clock.rs"]
mod cases;
use hoyadb_core::{command::*, storage::*};
use hoyadb_storage_sqlite::SqliteStorage;

fn path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("hoya-clock-{name}-{}.sqlite", std::process::id()))
}

/// Read-only, lock-free inspection of a live owner's committed pages, for task
/// fixtures only (SPEC-0037 R1 excludes every locking reader). Preconditions: the
/// same owner is idle between synchronous calls, with no open, queued or
/// concurrent write, so the main file holds only committed pages. The fixture path
/// is a known temp path of an unprotected task fixture; it is checked to need no
/// URI escaping. The connection is read-only and query-only, a write attempt is
/// proven to fail, no `immutable` flag is used, and every error is fatal. Each
/// inspection opens a fresh connection that drops before the next owner call. This
/// supplements, and never replaces, production close/reopen assertions.
fn inspect_idle_owner(path: &std::path::Path) -> rusqlite::Connection {
    let text = path.to_str().unwrap();
    assert!(
        text.bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/._-".contains(&byte))
    );
    let sql = rusqlite::Connection::open_with_flags(
        format!("file:{text}?mode=ro&nolock=1"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .unwrap();
    sql.execute_batch("PRAGMA query_only=1").unwrap();
    let write = sql.execute_batch("CREATE TABLE inspection_write_probe(x)");
    assert_eq!(
        write.err().and_then(|error| error.sqlite_error_code()),
        Some(rusqlite::ErrorCode::ReadOnly)
    );
    sql
}

/// One persisted field: row id, name, kind, text payload and integer payload.
type LedgerField = (String, String, i64, Option<String>, Option<i64>);

/// Exact persisted ledger fields of a closed store or an idle live owner.
fn durable_ledger(path: &std::path::Path) -> Vec<LedgerField> {
    let rows = inspect_idle_owner(path)
        .prepare("SELECT row_id,name,kind,text_value,int_value FROM __hoya_fields WHERE table_name='__hoya_sync' ORDER BY row_id,name")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(!rows.is_empty());
    rows
}

// Cases run sequentially in one test; native SQLite owners admit per database (SPEC-0037 R3).
#[test]
fn tc0039_clock_recovery() {
    cs01_cs02_cs08_corrected_jump();
    cs09_backward_sample();
    cs03_restart_retains_cap();
    cs07_statement_abort_inside_durable_transaction_preserves_prior_bytes();
}

fn cs01_cs02_cs08_corrected_jump() {
    let path = path("jump");
    cases::corrected_jump(SqliteStorage::open_unprotected(&path).unwrap());
    std::fs::remove_file(path).unwrap();
}

fn cs09_backward_sample() {
    let path = path("backward");
    cases::backward_sample(SqliteStorage::open_unprotected(&path).unwrap());
    std::fs::remove_file(path).unwrap();
}

// CS03: a cap persisted without sending survives close/reopen and retries with identical bytes.
fn cs03_restart_retains_cap() {
    let path = path("restart");
    let mut db = cases::open(SqliteStorage::open_unprotected(&path).unwrap());
    let (key, envelope) = cases::sent_at_forward_jump(&mut db);
    assert!(db.prepare_push(cases::R, 0).unwrap().envelopes.is_empty());
    db.close();
    let mut db = cases::open(SqliteStorage::open_unprotected(&path).unwrap());
    let restored = cases::entry(&db, &key);
    assert_eq!(
        (
            restored.key.as_str(),
            restored.attempts,
            restored.next_due_ms
        ),
        (key.as_str(), 1, Some(cases::R + cases::C)),
        "exact capped deadline and key survive restart"
    );
    assert_eq!(
        db.prepare_push(cases::R + cases::C, 0).unwrap().envelopes,
        vec![envelope]
    );
    db.close();
    std::fs::remove_file(path).unwrap();
}

// CS07: a statement abort (RAISE(ABORT) before COMMIT) inside the durable cap transaction
// persists nothing.
fn cs07_statement_abort_inside_durable_transaction_preserves_prior_bytes() {
    let path = path("fault");
    let mut db = cases::open(SqliteStorage::open_unprotected(&path).unwrap());
    let (key, envelope) = cases::sent_at_forward_jump(&mut db);
    // SPEC-0037 R1/R2.3: the owner holds the file, so read the prior bytes through a
    // production reopen and install the guarded trigger while it is closed. CS03
    // shows the Sent cap state survives this reopen; the trigger aborts only while
    // armed on the live owner below.
    db.close();
    let before = SqliteStorage::open_unprotected(&path)
        .unwrap()
        .load_sync()
        .unwrap();
    let before_fields = durable_ledger(&path);
    rusqlite::Connection::open(&path).unwrap().execute_batch("CREATE TRIGGER clock_cap_fail BEFORE INSERT ON __hoya_fields WHEN NEW.table_name='__hoya_sync' AND hoya_test_fault('clock_cap_fail') BEGIN SELECT RAISE(ABORT,'fixture cap failure'); END").unwrap();
    let mut db = cases::open(SqliteStorage::open_unprotected(&path).unwrap());
    let fault = hoyadb_storage_sqlite::test_hooks::arm(&path, "clock_cap_fail");
    assert!(matches!(
        db.prepare_push(cases::R, 0),
        Err(CommandError::Storage(StorageError::BackendFailure { .. }))
    ));
    let entry = cases::entry(&db, &key);
    assert_eq!(
        (entry.attempts, entry.next_due_ms),
        (1, Some(cases::F + 1000)),
        "owner state unchanged"
    );
    // Supplement: the idle live owner's committed ledger fields are unchanged.
    assert_eq!(durable_ledger(&path), before_fields);
    drop(fault);
    db.close();
    let after = SqliteStorage::open_unprotected(&path)
        .unwrap()
        .load_sync()
        .unwrap();
    assert_eq!(
        before, after,
        "no partial cap or attempt change after reopen"
    );
    let mut db = cases::open(SqliteStorage::open_unprotected(&path).unwrap());
    assert!(db.prepare_push(cases::R, 0).unwrap().envelopes.is_empty());
    assert_eq!(
        db.prepare_push(cases::R + cases::C, 0).unwrap().envelopes,
        vec![envelope]
    );
    db.close();
    std::fs::remove_file(path).unwrap();
}
