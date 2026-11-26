//! TC-0032: public command owner against real native SQLite.
#[path = "../../../tests/conformance/command_owner.rs"]
mod cases;
#[test]
fn c01_c06_c08_public_owner_contract() {
    let directory = std::env::temp_dir().join(format!("hoya-command-owner-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let mut sequence = 0;
    cases::owner_contract(|| {
        sequence += 1;
        hoyadb_storage_sqlite::SqliteStorage::open_unprotected(
            directory.join(format!("store-{sequence}.sqlite")),
        )
        .unwrap()
    });
    verify_reopen(&directory.join("store-1.sqlite"));
    verify_atomic_failure(&directory.join("fault.sqlite"));
    std::fs::remove_dir_all(directory).unwrap();
}

fn verify_reopen(path: &std::path::Path) {
    use hoyadb_core::{command::records::*, command::*, storage::*};
    let store = hoyadb_storage_sqlite::SqliteStorage::open_unprotected(path).unwrap();
    let original = SyncLedger::from_rows(&store.load_sync().unwrap().unwrap()).unwrap();
    assert_eq!(original.records.len(), 2);
    assert_eq!(original.header.namespace, "01".repeat(16));
    assert_eq!(original.header.sequence, 3);
    assert_eq!(original.header.last_ms, 1000);
    assert_eq!(
        cases::run(store.catalog()).unwrap().state.unwrap().next_id,
        4
    );
    let first = original
        .records
        .values()
        .min_by_key(|record| record.order)
        .unwrap();
    assert_eq!(first.created.len(), 2);
    assert_eq!(first.undo.len(), 2);
    assert!(
        matches!(&first.context,Json::Object(context) if context["capturedAt"]==Json::String("1970-01-01T00:00:01.000Z".into()))
    );
    let dependent = original
        .records
        .values()
        .max_by_key(|record| record.order)
        .unwrap();
    assert_eq!(dependent.dependencies.len(), 1);
    assert!(dependent.dependencies.contains("local:2"));
    let mut db = cases::run(CommandDatabase::open(
        store,
        cases::schema(),
        vec![],
        [99; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    let request = db.request().unwrap();
    assert!(
        cases::run(db.begin(
            request,
            cases::definition(),
            cases::args("restart"),
            Some(Json::Null),
            0
        ))
        .unwrap()
    );
    let result = cases::run(db.finish(request, Json::Null)).unwrap();
    assert!(
        result
            .key
            .starts_with("00000000000003e8-01010101010101010101010101010101-")
    );
    assert!(result.key.ends_with("0000000000000003"));
    db.close();
    let store = hoyadb_storage_sqlite::SqliteStorage::open_unprotected(path).unwrap();
    let after = SyncLedger::from_rows(&store.load_sync().unwrap().unwrap()).unwrap();
    assert_eq!(after.header.sequence, 4);
    assert_eq!(after.header.namespace, original.header.namespace);
    assert!(after.records.contains_key(&result.key));
    assert_eq!(
        cases::run(store.catalog()).unwrap().state.unwrap().next_id,
        4
    );
    drop(store);
    let options = SyncOptions {
        capacity: 7,
        ..SyncOptions::default()
    };
    assert!(matches!(
        cases::run(CommandDatabase::open(
            hoyadb_storage_sqlite::SqliteStorage::open_unprotected(path).unwrap(),
            cases::schema(),
            vec![],
            [88; 16],
            options
        )),
        Err(CommandError::InvalidCommand)
    ));
    let store = hoyadb_storage_sqlite::SqliteStorage::open_unprotected(path).unwrap();
    assert_eq!(
        SyncLedger::from_rows(&store.load_sync().unwrap().unwrap()).unwrap(),
        after
    );
}

fn verify_atomic_failure(path: &std::path::Path) {
    use hoyadb_core::{command::records::*, command::*, storage::*};
    // SPEC-0037 R1/R2.3: the owner holds the file, so install the guarded trigger
    // while closed; it aborts only while armed on the live owner below.
    drop(hoyadb_storage_sqlite::SqliteStorage::open_unprotected(path).unwrap());
    rusqlite::Connection::open(path).unwrap().execute_batch("CREATE TRIGGER owner_commit_fail BEFORE INSERT ON __hoya_fields WHEN NEW.table_name='__hoya_sync' AND hoya_test_fault('owner_commit_fail') BEGIN SELECT RAISE(ABORT,'fixture atomic enqueue failure'); END").unwrap();
    let mut db = cases::run(CommandDatabase::open(
        hoyadb_storage_sqlite::SqliteStorage::open_unprotected(path).unwrap(),
        cases::schema(),
        vec![],
        [5; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    // Each count runs between synchronous owner calls, so the owner is idle.
    let sync_count = || {
        inspect_idle_owner(path)
            .query_row(
                "SELECT count(*) FROM __hoya_tables WHERE name='__hoya_sync'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
    };
    assert_eq!(sync_count(), 0);
    let query = db.query("parents");
    cases::run(db.observe(&query)).unwrap();
    db.drain_notifications(0);
    let fault = hoyadb_storage_sqlite::test_hooks::arm(path, "owner_commit_fail");
    let request = db.request().unwrap();
    cases::run(db.begin(
        request,
        cases::definition(),
        cases::args("fault"),
        Some(Json::Null),
        11,
    ))
    .unwrap();
    let staged =
        cases::run(db.scope_insert(request, "parents", cases::fields("not committed"))).unwrap();
    assert_eq!(staged.id, "local:1");
    assert!(matches!(
        cases::run(db.finish(request, Json::Null)),
        Err(CommandError::Storage(StorageError::BackendFailure { .. }))
    ));
    assert!(cases::run(query.fetch(&db)).unwrap().is_empty());
    assert!(db.drain_notifications(11).is_empty());
    assert!(db.scope_args(request).is_err());
    assert_eq!(sync_count(), 0);
    drop(fault);
    let request = db.request().unwrap();
    cases::run(db.begin(
        request,
        cases::definition(),
        cases::args("retry"),
        Some(Json::Null),
        11,
    ))
    .unwrap();
    let committed =
        cases::run(db.scope_insert(request, "parents", cases::fields("committed"))).unwrap();
    assert_eq!(committed.id, staged.id);
    let result = cases::run(db.finish(request, Json::Null)).unwrap();
    assert!(result.key.ends_with("0000000000000001"));
    assert_eq!(db.drain_notifications(11).len(), 1);
    assert_eq!(sync_count(), 1);
    db.close();
    let store = hoyadb_storage_sqlite::SqliteStorage::open_unprotected(path).unwrap();
    let ledger = SyncLedger::from_rows(&store.load_sync().unwrap().unwrap()).unwrap();
    assert_eq!(ledger.records.len(), 1);
    assert_eq!(ledger.header.sequence, 2);
    assert_eq!(
        cases::run(store.catalog()).unwrap().state.unwrap().next_id,
        2
    );
    assert!(ledger.records.contains_key(&result.key));
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
