use hoyadb_doc_examples::*;

fn main() {
    let mut db = owner();
    db.configure_entities(
        EntityConfig {
            scope: "account-fixture".into(),
            tables: BTreeSet::from(["notes".into()]),
        },
        || {
            Ok(EntityContext {
                identity: Json::Null,
                now_ms: 100,
            })
        },
    )
    .unwrap();
    // docs:start request
    let batch = db.begin_pull().unwrap();
    // docs:end request
    assert!(batch.since.is_none());
    // docs:start change
    let change = EntityChange {
        table: "notes".into(),
        record_id: "remote-note".into(),
        op: EntityOperation::Create,
        changes: fields("From backend"),
        removed_fields: BTreeSet::new(),
        timestamp_ms: 10,
        field_timestamps: BTreeMap::from([("title".into(), 10), ("done".into(), 10)]),
    };
    // docs:end change
    // docs:start apply
    run(db.apply_pull(
        batch.request,
        PullPage {
            checkpoint: "page-1".into(),
            complete: true,
            changes: vec![change],
        },
    ))
    .unwrap();
    // docs:end apply
    assert!(db.sync_progress().ready);
    assert_eq!(run(db.query("notes").fetch(&db)).unwrap().len(), 1);
    // docs:start checkpoint
    let batch = db.begin_pull().unwrap();
    // docs:end checkpoint
    assert_eq!(batch.since.as_deref(), Some("page-1"));
    // docs:start prepare
    let step = run(db.merge_prepare(
        batch.request,
        PullPage {
            checkpoint: "page-2".into(),
            complete: true,
            changes: vec![],
        },
        BTreeSet::new(),
    ))
    .unwrap();
    // docs:end prepare
    // docs:start commit
    let MergeStep::Ready {
        session,
        ..
    } = step
    else {
        panic!("empty page has no hook offers")
    };
    run(db.merge_commit(session)).unwrap();
    // docs:end commit
    assert_eq!(db.begin_pull().unwrap().since.as_deref(), Some("page-2"));
    println!("PASS pull/merge: applied page, checkpoint and passive merge commit");
}
