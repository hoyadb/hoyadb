#[path = "../../../tests/conformance/platform_core.rs"]
mod cases;

#[test]
fn g07_g18_g22_core_seams() {
    let directory = std::env::temp_dir().join(format!("hoya-platform-core-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let mut counter = 0;
    let mut fresh = || {
        counter += 1;
        hoyadb_storage_sqlite::SqliteStorage::open_with_options(
            directory.join(format!("store-{counter}.sqlite")),
            hoyadb_storage_sqlite::SqliteOpenOptions {
                encryption_key: Some(hoyadb_storage_sqlite::EncryptionKey::from_bytes([0x71; 32])),
            },
        )
        .unwrap()
    };
    cases::proposed_links_and_recovery(&mut fresh);
    cases::passive_merge(&mut fresh);
    cases::normalization(fresh());
    passive_commit_failure(&directory.join("commit-fault.sqlite"));
    std::fs::remove_dir_all(directory).unwrap();
}

fn passive_commit_failure(path: &std::path::Path) {
    use hoyadb_core::{command::*, storage::*};
    use std::collections::BTreeSet;
    let open = || {
        hoyadb_storage_sqlite::SqliteStorage::open_with_options(
            path,
            hoyadb_storage_sqlite::SqliteOpenOptions {
                encryption_key: Some(hoyadb_storage_sqlite::EncryptionKey::from_bytes([0x71; 32])),
            },
        )
        .unwrap()
    };
    // SPEC-0037 R1/R2.3: the owner holds the file, so install the guarded trigger
    // while closed; it aborts only while armed on the live owner below.
    drop(open());
    let sql = rusqlite::Connection::open(path).unwrap();
    sql.execute_batch(
        "PRAGMA key=\"x'7171717171717171717171717171717171717171717171717171717171717171'\";",
    )
    .unwrap();
    sql.execute_batch("CREATE TRIGGER passive_commit_fail BEFORE INSERT ON __hoya_fields WHEN NEW.table_name='__hoya_sync' AND hoya_test_fault('passive_commit_fail') BEGIN SELECT RAISE(ABORT,'fixture passive failure'); END").unwrap();
    drop(sql);
    let store = open();
    let mut db = cases::run(CommandDatabase::open(
        store,
        cases::merge_schema(),
        vec![],
        [43; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    db.configure_entities(
        EntityConfig {
            scope: "merge-fault".into(),
            tables: BTreeSet::from(["items".into()]),
        },
        || {
            Ok(EntityContext {
                identity: Json::Null,
                now_ms: 0,
            })
        },
    )
    .unwrap();
    let request = db.begin_pull().unwrap().request;
    cases::run(db.apply_pull(
        request,
        PullPage {
            checkpoint: "seed".into(),
            complete: true,
            changes: vec![cases::change("before", 10)],
        },
    ))
    .unwrap();
    let query = db.query("items");
    let data = cases::run(db.observe_with_columns(&query, vec!["body".into()])).unwrap();
    let progress = db.observe_progress().unwrap();
    db.drain_events(0);
    let before = cases::run(query.fetch(&db)).unwrap();
    let old_progress = db.sync_progress();
    let request = db.begin_pull().unwrap().request;
    let MergeStep::Offer {
        session,
        step,
        ..
    } = cases::run(db.merge_prepare(
        request,
        PullPage {
            checkpoint: "committed".into(),
            complete: true,
            changes: vec![cases::change("after", 20)],
        },
        BTreeSet::from(["items".into()]),
    ))
    .unwrap()
    else {
        panic!("expected offer")
    };
    let row = db.merge_input(session, step).unwrap().default.row;
    assert!(matches!(
        cases::run(db.merge_reply(session, step, row)),
        Ok(MergeStep::Ready { .. })
    ));
    let fault = hoyadb_storage_sqlite::test_hooks::arm(path, "passive_commit_fail");
    assert!(matches!(
        cases::run(db.merge_commit(session)),
        Err(CommandError::Storage(StorageError::BackendFailure { .. }))
    ));
    assert_eq!(cases::run(query.fetch(&db)).unwrap(), before);
    assert_eq!(db.sync_progress(), old_progress);
    assert!(
        db.drain_events(0).is_empty(),
        "failed atomic commit must publish no data or progress"
    );
    drop(fault);
    cases::run(db.merge_commit(session)).unwrap();
    assert_eq!(db.sync_progress().pages, 2);
    assert_eq!(
        cases::run(query.fetch(&db)).unwrap()[0].fields["body"],
        Value::String("after".into())
    );
    let events = db.drain_events(0);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event,CommandEvent::Data(n) if n.subscription==data))
            .count(),
        1
    );
    assert_eq!(events.iter().filter(|event|matches!(event,CommandEvent::Progress {subscription,..} if *subscription==progress)).count(),1);
    let request = db.begin_pull().unwrap();
    assert_eq!(request.since, Some("committed".into()));
    db.abort_pull(request.request).unwrap();
    db.close();
    let store = hoyadb_storage_sqlite::SqliteStorage::open_with_options(
        path,
        hoyadb_storage_sqlite::SqliteOpenOptions {
            encryption_key: Some(hoyadb_storage_sqlite::EncryptionKey::from_bytes([0x71; 32])),
        },
    )
    .unwrap();
    let db = cases::run(CommandDatabase::open(
        store,
        cases::merge_schema(),
        vec![],
        [99; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    assert_eq!(db.sync_progress().pages, 2);
    assert_eq!(
        cases::run(db.query("items").fetch(&db)).unwrap()[0].fields["body"],
        Value::String("after".into())
    );
}
