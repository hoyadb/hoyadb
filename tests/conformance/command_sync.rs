//! API-0006 section 5 / TC-0032: public durable dispatch and owned delivery.
use hoyadb_core::model::*;
use hoyadb_core::{command::*, storage::*};
use std::{
    collections::BTreeMap,
    future::Future,
    task::{Context, Poll, Waker},
};

pub fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("immediate adapter suspended"),
    }
}

pub fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([(
            "parents".into(),
            TableDefinition {
                columns: BTreeMap::from([(
                    "name".into(),
                    Column {
                        kind: ColumnType::String,
                        optional: false,
                    },
                )]),
                relations: BTreeMap::new(),
            },
        )]),
    }
}

pub fn definition() -> CommandDefinition {
    CommandDefinition {
        name: "create-parent-v1".into(),
        arguments: BTreeMap::from([(
            "label".into(),
            Argument {
                kind: ArgumentType::String,
                optional: false,
            },
        )]),
        reference_paths: vec![],
    }
}

pub fn args(label: &str) -> Json {
    Json::Object(BTreeMap::from([(
        "label".into(),
        Json::String(label.into()),
    )]))
}

pub fn fields(name: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([("name".into(), Value::String(name.into()))])
}

pub fn dispatch<S: SyncStorage>(storage: S) {
    let mut db = run(CommandDatabase::open(
        storage,
        schema(),
        vec![],
        [17; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    assert!(db.prepare_push(0, 0).unwrap().envelopes.is_empty());
    assert_eq!(db.queue_status().live, 0);
    let status = db
        .observe_queue_status()
        .expect("initial owned queue registration");
    assert!(
        matches!(db.drain_events(0).as_slice(),[CommandEvent::Status{subscription,value}] if *subscription==status && value.live==0)
    );
    let request = db.request().unwrap();
    assert!(
        run(db.begin(
            request,
            definition(),
            args("dispatch"),
            Some(Json::Null),
            100
        ))
        .unwrap()
    );
    run(db.scope_insert(request, "parents", fields("local"))).unwrap();
    let key = run(db.finish(request, Json::Null)).unwrap().key;
    let outcome = db.observe_outcome(&key).unwrap();
    let events = db.drain_events(100);
    assert!(events.iter().any(|e|matches!(e,CommandEvent::Outcome{subscription,value:CommandOutcome::Queued} if *subscription==outcome)));
    let first = db.prepare_push(100, 2500).unwrap();
    assert_eq!(first.envelopes.len(), 1);
    assert_eq!(first.next_due_ms, Some(1350));
    assert_eq!(
        Json::parse(&first.envelopes[0])
            .unwrap()
            .canonical()
            .unwrap(),
        first.envelopes[0]
    );
    assert_eq!(db.queue_status().sending, 1);
    assert!(db.prepare_push(1349, 0).unwrap().envelopes.is_empty());
    let retry = db.prepare_push(1350, 0).unwrap();
    assert_eq!(retry.envelopes, first.envelopes);
    assert_eq!(retry.next_due_ms, Some(3350));
    db.receive(&key, None).unwrap();
    assert_eq!(db.queue_status().awaiting_outcome, 1);
    assert!(db.prepare_push(9999, 0).unwrap().envelopes.is_empty());
    assert!(db.pending_reconciliations().is_empty());
    let result = RemoteOutcome::Corrected(Json::Integer(i64::MAX));
    db.receive(&key, Some(result.clone())).unwrap();
    assert_eq!(
        db.pending_reconciliations(),
        vec![(key.clone(), result.clone())]
    );
    let events = db.drain_events(9999);
    assert!(events.iter().any(|e| matches!(
        e,
        CommandEvent::Outcome {
            value: CommandOutcome::Acknowledged,
            ..
        }
    )));
    assert!(!events.iter().any(|e| matches!(
        e,
        CommandEvent::Outcome {
            value: CommandOutcome::Corrected(_),
            ..
        }
    )));
    db.receive(&key, Some(result)).unwrap();
    assert!(
        db.drain_events(9999).is_empty(),
        "duplicate receipt must not duplicate events"
    );
    assert_eq!(
        db.receive(&key, Some(RemoteOutcome::Rejected("conflict".into()))),
        Err(CommandError::Protocol)
    );
    assert_eq!(db.receive("unknown", None), Err(CommandError::Protocol));
    assert_eq!(db.prepare_push(-1, 0), Err(CommandError::InvalidCommand));
    assert_eq!(db.prepare_push(0, 2501), Err(CommandError::InvalidCommand));
    assert!(db.unsubscribe_sync(status));
    assert!(db.unsubscribe_sync(outcome));
    assert!(!db.unsubscribe_sync(outcome));
    let gone = db.observe_queue_status().unwrap();
    assert!(db.unsubscribe_sync(gone));
    assert!(db.drain_events(9999).is_empty());
    let r = db.request().unwrap();
    run(db.begin(
        r,
        definition(),
        args("backward after idle wake"),
        Some(Json::Null),
        0,
    ))
    .unwrap();
    let later = run(db.finish(r, Json::Null)).unwrap();
    assert!(
        later.key.starts_with("000000000000270f-"),
        "a persisted queue clock must retain idle wake samples"
    );
    db.close();
}

pub fn enqueue<S: SyncStorage>(db: &mut CommandDatabase<S>, label: &str) -> (String, Row) {
    let r = db.request().unwrap();
    assert!(run(db.begin(r, definition(), args(label), Some(Json::Null), 100)).unwrap());
    let row = run(db.scope_insert(r, "parents", fields(label))).unwrap();
    (run(db.finish(r, Json::Null)).unwrap().key, row)
}

pub fn correction_inverse<S: SyncStorage>(storage: S) {
    let mut db = run(CommandDatabase::open(
        storage,
        schema(),
        vec![],
        [18; 16],
        SyncOptions {
            history: 1,
            ..SyncOptions::default()
        },
    ))
    .unwrap();
    let query = db.query("parents");
    let data = run(db.observe(&query)).unwrap();
    db.drain_events(0);
    let (key, row) = enqueue(&mut db, "optimistic");
    let out = db.observe_outcome(&key).unwrap();
    db.drain_events(100);
    db.prepare_push(100, 0).unwrap();
    db.receive(
        &key,
        Some(RemoteOutcome::Corrected(Json::String(
            "arbitrary backend result".into(),
        ))),
    )
    .unwrap();
    assert_eq!(run(query.fetch(&db)).unwrap(), vec![row.clone()]);
    let bad = ReconcilePlan {
        mappings: BTreeMap::from([(row.id.clone(), "local:invalid".into())]),
        writes: vec![],
    };
    assert!(run(db.reconcile(&key, bad)).is_err());
    assert_eq!(db.pending_reconciliations().len(), 1);
    let invalid_id = ReconcilePlan {
        mappings: BTreeMap::from([(row.id.clone(), "server:one".into())]),
        writes: vec![ReconcileWrite::Put {
            table: "parents".into(),
            id: String::new(),
            fields: fields("invalid id"),
        }],
    };
    assert!(
        run(db.reconcile(&key, invalid_id)).is_err(),
        "authoritative writes require nonempty IDs"
    );
    assert_eq!(run(query.fetch(&db)).unwrap(), vec![row.clone()]);
    assert_eq!(db.pending_reconciliations().len(), 1);

    db.drain_events(100);
    let plan = ReconcilePlan {
        mappings: BTreeMap::from([(row.id.clone(), "server:one".into())]),
        writes: vec![ReconcileWrite::Put {
            table: "parents".into(),
            id: "server:one".into(),
            fields: fields("authoritative"),
        }],
    };
    run(db.reconcile(&key, plan)).unwrap();
    assert_eq!(
        run(query.fetch(&db)).unwrap(),
        vec![Row {
            id: "server:one".into(),
            fields: fields("authoritative")
        }]
    );
    assert!(db.pending_reconciliations().is_empty());
    let events = db.drain_events(100);
    let dp = events
        .iter()
        .position(|e| matches!(e,CommandEvent::Data(n) if n.subscription==data))
        .unwrap();
    let op=events.iter().position(|e|matches!(e,CommandEvent::Outcome{subscription,value:CommandOutcome::Corrected(_)} if *subscription==out)).unwrap();
    assert!(dp < op, "complete correction data precedes outcome");
    assert_eq!(db.queue_status().live, 0);
    let (rejected, removed) = enqueue(&mut db, "rejected");
    db.prepare_push(100, 0).unwrap();
    db.receive(
        &rejected,
        Some(RemoteOutcome::Rejected("business rule".into())),
    )
    .unwrap();
    run(db.reconcile(&rejected, ReconcilePlan::default())).unwrap();
    assert!(
        run(query.fetch(&db)).unwrap().contains(&removed),
        "rejection requires explicit inverse"
    );
    run(db.rollback(&rejected)).unwrap();
    run(db.rollback(&rejected)).unwrap();
    assert!(!run(query.fetch(&db)).unwrap().contains(&removed));
    let (cancelled, crow) = enqueue(&mut db, "cancel original");
    db.prepare_push(100, 0).unwrap();
    run(db.replace("parents", &crow.id, fields("independent edit"))).unwrap();
    assert_eq!(
        run(db.cancel(&cancelled)),
        Err(CommandError::RollbackConflict)
    );
    assert_eq!(
        db.queue_status()
            .commands
            .iter()
            .find(|r| r.key == cancelled)
            .unwrap()
            .state,
        SyncState::Cancelled
    );
    assert_eq!(db.queue_status().live, 1);
    assert!(db.prepare_push(10000, 0).unwrap().envelopes.is_empty());
    db.receive(&cancelled, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    assert!(db.forget(&cancelled).is_err());
    assert_eq!(
        run(db.rollback(&cancelled)),
        Err(CommandError::RollbackConflict)
    );
    assert!(
        run(query.fetch(&db))
            .unwrap()
            .iter()
            .any(|r| r.fields == fields("independent edit"))
    );
    run(db.replace("parents", &crow.id, crow.fields.clone())).unwrap();
    run(db.rollback(&cancelled)).unwrap();
    assert_eq!(db.queue_status().live, 1, "sent tombstone remains live");
    db.forget(&cancelled).unwrap();
    assert_eq!(db.receive(&cancelled, None), Err(CommandError::Protocol));
    let (unsent, urow) = enqueue(&mut db, "unsent cancel");
    run(db.cancel(&unsent)).unwrap();
    assert!(!run(query.fetch(&db)).unwrap().contains(&urow));
    assert_eq!(db.queue_status().live, 0);
    assert!(
        db.queue_status().commands.len() <= 1,
        "resolved history is bounded"
    );
    db.close();
}

pub fn graph<S: SyncStorage>(storage: S) {
    let mut schema = schema();
    schema.tables.get_mut("parents").unwrap().relations.insert(
        "parent".into(),
        RelationDefinition::ToOne {
            table: "parents".into(),
            foreign_key: "parent_id".into(),
            optional: true,
        },
    );
    let mut db = run(CommandDatabase::open(
        storage,
        schema,
        vec![],
        [19; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    let (a, arow) = enqueue(&mut db, "A");
    let (b, brow) = enqueue(&mut db, "B");
    let mut definition = definition();
    for field in ["left", "right"] {
        definition.arguments.insert(
            field.into(),
            Argument {
                kind: ArgumentType::String,
                optional: true,
            },
        );
    }
    definition.reference_paths = vec![
        vec![PathPart::Key("left".into())],
        vec![PathPart::Key("right".into())],
    ];
    let value = Json::Object(BTreeMap::from([
        ("label".into(), Json::String(arow.id.clone())),
        ("left".into(), Json::String(arow.id.clone())),
        ("right".into(), Json::String(brow.id.clone())),
    ]));
    let r = db.request().unwrap();
    run(db.begin(r, definition.clone(), value, Some(Json::Null), 100)).unwrap();
    let mut cf = fields(&arow.id);
    cf.insert("parent_id".into(), Value::Reference(arow.id.clone()));
    let child = run(db.scope_insert(r, "parents", cf)).unwrap();
    let c = run(db.finish(r, Json::Null)).unwrap().key;
    let value = Json::Object(BTreeMap::from([
        ("label".into(), Json::String("grandchild".into())),
        ("left".into(), Json::String(child.id.clone())),
        ("right".into(), Json::String(arow.id.clone())),
    ]));
    let r = db.request().unwrap();
    run(db.begin(r, definition, value, Some(Json::Null), 100)).unwrap();
    let mut df = fields("grandchild");
    df.insert("parent_id".into(), Value::Reference(child.id.clone()));
    let grandchild = run(db.scope_insert(r, "parents", df)).unwrap();
    let d = run(db.finish(r, Json::Null)).unwrap().key;
    assert_eq!(
        db.queue_status()
            .commands
            .iter()
            .find(|e| e.key == c)
            .unwrap()
            .blocked_on
            .len(),
        2
    );
    assert_eq!(db.prepare_push(100, 0).unwrap().envelopes.len(), 2);
    db.receive(&a, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    run(db.reconcile(
        &a,
        ReconcilePlan {
            mappings: BTreeMap::from([(arow.id.clone(), "server:A".into())]),
            writes: vec![],
        },
    ))
    .unwrap();
    let rows = run(db.query("parents").fetch(&db)).unwrap();
    let current = rows.iter().find(|r| r.id == child.id).unwrap();
    assert_eq!(
        current.fields["parent_id"],
        Value::Reference("server:A".into())
    );
    assert_eq!(current.fields["name"], Value::String(arow.id.clone()));
    assert_eq!(
        db.queue_status()
            .commands
            .iter()
            .find(|e| e.key == c)
            .unwrap()
            .blocked_on,
        vec![brow.id.clone()]
    );
    assert!(db.prepare_push(100, 0).unwrap().envelopes.is_empty());
    db.receive(&b, Some(RemoteOutcome::Rejected("reject B only".into())))
        .unwrap();
    run(db.reconcile(&b, ReconcilePlan::default())).unwrap();
    run(db.rollback(&b)).unwrap();
    let rows = run(db.query("parents").fetch(&db)).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "server:A");
    let status = db.queue_status();
    assert!(
        status
            .commands
            .iter()
            .find(|e| e.key == c)
            .unwrap()
            .rolled_back
    );
    assert!(
        status
            .commands
            .iter()
            .find(|e| e.key == d)
            .unwrap()
            .rolled_back
    );
    assert!(!rows.iter().any(|r| r.id == grandchild.id));
    let (_, next) = enqueue(&mut db, "counter after inverse");
    assert_eq!(next.id, "local:5");
    db.close();
}

pub fn zero_history<S: SyncStorage>(storage: S) {
    let mut db = run(CommandDatabase::open(
        storage,
        schema(),
        vec![],
        [20; 16],
        SyncOptions {
            history: 0,
            ..SyncOptions::default()
        },
    ))
    .unwrap();
    for corrected in [false, true] {
        let r = db.request().unwrap();
        run(db.begin(r, definition(), args("no-op"), Some(Json::Null), 100)).unwrap();
        let key = run(db.finish(r, Json::Null)).unwrap().key;
        let subscription = db.observe_outcome(&key).unwrap();
        db.prepare_push(100, 0).unwrap();
        let value = Json::Integer(42);
        db.receive(
            &key,
            Some(if corrected {
                RemoteOutcome::Corrected(value.clone())
            } else {
                RemoteOutcome::Applied(value.clone())
            }),
        )
        .unwrap();
        db.drain_events(100);
        run(db.reconcile(&key, ReconcilePlan::default())).unwrap();
        let expected = if corrected {
            CommandOutcome::Corrected(value)
        } else {
            CommandOutcome::Applied(value)
        };
        let events = db.drain_events(100);
        assert_eq!(
            events,
            vec![CommandEvent::Outcome {
                subscription,
                value: expected
            }],
            "history eviction must retain committed live delivery"
        );
        assert!(db.queue_status().commands.is_empty());
        assert!(db.drain_events(100).is_empty());
        db.unsubscribe_sync(subscription);
    }
    db.close();
}

pub fn capacity_boundary<S: SyncStorage>(storage: S) {
    let mut db = run(CommandDatabase::open(
        storage,
        schema(),
        vec![],
        [22; 16],
        SyncOptions {
            capacity: 500,
            ..SyncOptions::default()
        },
    ))
    .unwrap();
    for _ in 0..500 {
        let r = db.request().unwrap();
        assert!(run(db.begin(r, definition(), args("capacity"), Some(Json::Null), 100)).unwrap());
        run(db.finish(r, Json::Null)).unwrap();
    }
    let status = db.queue_status();
    assert_eq!(status.live, 500);
    assert!(status.blocked);
    let r = db.request().unwrap();
    assert_eq!(
        run(db.begin(
            r,
            definition(),
            args("must not admit"),
            Some(Json::Null),
            100
        )),
        Err(CommandError::Capacity)
    );
    assert!(db.scope_args(r).is_err());
    assert_eq!(db.queue_status(), status);
    db.close();
}
