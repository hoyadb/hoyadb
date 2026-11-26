//! TC-0032 C01–C06/C08: public owner on actual transactional adapters.
use hoyadb_core::{command::records::*, command::*, model::*, storage::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    task::{Context, Poll, Waker},
};

pub fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("synchronous adapter unexpectedly suspended"),
    }
}

pub fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([
            (
                "parents".into(),
                TableDefinition {
                    columns: BTreeMap::from([(
                        "name".into(),
                        Column {
                            kind: ColumnType::String,
                            optional: false,
                        },
                    )]),
                    relations: BTreeMap::from([(
                        "children".into(),
                        RelationDefinition::HasMany {
                            table: "children".into(),
                            foreign_key: "parent_id".into(),
                        },
                    )]),
                },
            ),
            (
                "children".into(),
                TableDefinition {
                    columns: BTreeMap::from([(
                        "rank".into(),
                        Column {
                            kind: ColumnType::Integer,
                            optional: false,
                        },
                    )]),
                    relations: BTreeMap::from([(
                        "parent".into(),
                        RelationDefinition::ToOne {
                            table: "parents".into(),
                            foreign_key: "parent_id".into(),
                            optional: false,
                        },
                    )]),
                },
            ),
        ]),
    }
}

pub fn definition() -> CommandDefinition {
    CommandDefinition {
        name: "create-parent-v1".into(),
        arguments: BTreeMap::from([
            (
                "label".into(),
                Argument {
                    kind: ArgumentType::String,
                    optional: false,
                },
            ),
            (
                "target".into(),
                Argument {
                    kind: ArgumentType::String,
                    optional: true,
                },
            ),
        ]),
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

fn identity() -> Option<Json> {
    Some(Json::String("trusted-user".into()))
}

pub fn owner_contract<S: SyncStorage>(mut fresh: impl FnMut() -> S) {
    let first = fresh();
    // SPEC-0037 R3: only unproven adapters and WASM share one coordinator.
    let shared = cfg!(target_arch = "wasm32") || first.ownership() == StoreOwnership::Unproven;
    let mut a = run(CommandDatabase::open(
        first,
        schema(),
        vec![],
        [1; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    let mut b = run(CommandDatabase::open(
        fresh(),
        schema(),
        vec![],
        [2; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    let initial = run(a.insert("parents", fields("prior"))).unwrap();
    let query = a.query("parents");
    let sub = run(a.observe(&query)).unwrap();
    a.drain_notifications(0);
    let ar = a.request().unwrap();
    let br = b.request().unwrap();
    let later = a.request().unwrap();
    assert_eq!(
        run(b.begin(br, definition(), args("second"), identity(), 1000)).unwrap(),
        !shared
    );
    assert!(run(a.begin(ar, definition(), args("first"), identity(), 1000)).unwrap());
    assert_eq!(a.scope_context(ar).unwrap().now(), 1000);
    assert_eq!(
        a.scope_context(ar).unwrap().identity(),
        &Json::String("trusted-user".into())
    );
    let parent = run(a.scope_insert(ar, "parents", fields("staged"))).unwrap();
    let child = run(a.scope_insert(
        ar,
        "children",
        BTreeMap::from([
            ("rank".into(), Value::Integer(7)),
            ("parent_id".into(), Value::Reference(parent.id.clone())),
        ]),
    ))
    .unwrap();
    let related = a
        .related("parents", &parent.id, "children")
        .order_by("rank", Direction::Desc)
        .limit(1);
    assert_eq!(run(a.scope_fetch(ar, &related)).unwrap(), vec![child]);
    assert_eq!(run(query.fetch(&a)).unwrap(), vec![initial]);
    assert!(a.drain_notifications(0).is_empty());
    assert_eq!(
        run(b.insert("parents", fields("bypass"))),
        Err(CommandError::Busy)
    );
    assert_eq!(
        run(a.insert("parents", fields("bypass"))),
        Err(CommandError::Busy)
    );
    assert_eq!(
        a.set_id_generator(|| "local:forbidden".into()),
        Err(CommandError::Busy)
    );
    assert!(!run(a.begin(later, definition(), args("later"), identity(), 1000)).unwrap());
    assert!(matches!(a.abort(later), Err(CommandError::InvalidTicket)));
    let first = run(a.finish(ar, Json::String("local result".into()))).unwrap();
    assert_eq!(first.result, Json::String("local result".into()));
    let events = a.drain_notifications(1000);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].subscription, sub);
    assert!(matches!(&events[0].value,Ok(ObservationValue::Rows(rows)) if rows.len()==2));
    if shared {
        assert!(run(b.begin(br, definition(), args("second"), identity(), 1000)).unwrap());
    }
    let second = run(b.finish(br, Json::Null)).unwrap();
    assert_ne!(first.key, second.key);
    assert!(run(a.begin(later, definition(), args("later"), identity(), 500)).unwrap());
    a.abort(later).unwrap();
    assert!(matches!(
        run(a.scope_fetch(later, &query)),
        Err(CommandError::InvalidTicket)
    ));
    assert!(matches!(
        run(a.finish(later, Json::Null)),
        Err(CommandError::InvalidTicket)
    ));
    for (definition, args, identity, now) in [
        (definition(), args("bad"), None, 0),
        (definition(), args("bad"), identity(), -1),
        (definition(), Json::Object(BTreeMap::new()), identity(), 0),
        (
            definition(),
            Json::Object(BTreeMap::from([("label".into(), Json::Bool(true))])),
            identity(),
            0,
        ),
        (
            CommandDefinition {
                name: "unversioned".into(),
                ..definition()
            },
            args("bad"),
            identity(),
            0,
        ),
    ] {
        let request = a.request().unwrap();
        assert!(run(a.begin(request, definition, args, identity, now)).is_err());
    }
    let request = a.request().unwrap();
    let local_reference = Json::Object(BTreeMap::from([
        ("label".into(), Json::String(parent.id.clone())),
        ("target".into(), Json::String(parent.id.clone())),
    ]));
    let dependent = CommandDefinition {
        reference_paths: vec![vec![PathPart::Key("target".into())]],
        ..definition()
    };
    assert!(run(a.begin(request, dependent.clone(), local_reference, identity(), 900)).unwrap());
    assert_eq!(
        a.scope_args(request).unwrap(),
        &Json::Object(BTreeMap::from([
            ("label".into(), Json::String(parent.id.clone())),
            ("target".into(), Json::String(parent.id.clone()))
        ]))
    );
    let dependent_result = run(a.finish(request, Json::Null)).unwrap();
    assert!(dependent_result.key > first.key);
    let request = a.request().unwrap();
    let unknown = Json::Object(BTreeMap::from([
        ("label".into(), Json::String("plain".into())),
        ("target".into(), Json::String("local:unknown".into())),
    ]));
    assert_eq!(
        run(a.begin(request, dependent, unknown, identity(), 1000)),
        Err(CommandError::UnknownLocalReference)
    );
    a.set_id_generator(|| "unsafe-server-id".into()).unwrap();
    let request = a.request().unwrap();
    assert!(run(a.begin(request, definition(), args("unsafe"), identity(), 1001)).unwrap());
    run(a.scope_insert(request, "parents", fields("must not persist"))).unwrap();
    assert!(run(a.finish(request, Json::Null)).is_err());
    assert_eq!(run(query.fetch(&a)).unwrap().len(), 2);
    assert!(a.scope_args(request).is_err());
    a.set_id_generator(|| "local:size-failure".into()).unwrap();
    // TC-0032 C05: args whose future frozen record cannot be stored reject
    // before handler admission and release the request.
    let request = a.request().unwrap();
    assert_eq!(
        run(a.begin(
            request,
            definition(),
            args(&"x".repeat(600_000)),
            identity(),
            1002
        )),
        Err(CommandError::LimitExceeded)
    );
    assert!(a.scope_args(request).is_err());
    // Staged images are known only at finish, which still rejects before commit.
    let request = a.request().unwrap();
    assert!(
        run(a.begin(
            request,
            definition(),
            args("size failure"),
            identity(),
            1002
        ))
        .unwrap()
    );
    run(a.scope_insert(
        request,
        "parents",
        fields(&"size must not commit".repeat(60_000)),
    ))
    .unwrap();
    assert_eq!(
        run(a.finish(request, Json::Null)),
        Err(CommandError::LimitExceeded)
    );
    assert_eq!(run(query.fetch(&a)).unwrap().len(), 2);
    assert!(a.scope_args(request).is_err());
    let request = a.request().unwrap();
    assert!(
        run(a.begin(
            request,
            definition(),
            args("after size failure"),
            identity(),
            1003
        ))
        .unwrap()
    );
    a.abort(request).unwrap();
    a.close();
    b.close();
    capacity(fresh());
    mapped_reference(fresh());
    retired_reference(fresh());
}

fn capacity<S: SyncStorage>(storage: S) {
    let options = SyncOptions {
        capacity: 1,
        ..SyncOptions::default()
    };
    let mut db = run(CommandDatabase::open(
        storage,
        schema(),
        vec![],
        [3; 16],
        options,
    ))
    .unwrap();
    let r = db.request().unwrap();
    run(db.begin(r, definition(), args("one"), identity(), 0)).unwrap();
    run(db.finish(r, Json::Null)).unwrap();
    let r = db.request().unwrap();
    assert_eq!(
        run(db.begin(r, definition(), args("two"), identity(), 0)),
        Err(CommandError::Capacity)
    );
    run(db.insert("parents", fields("local only"))).unwrap();
    db.close();
}

fn mapped_reference<S: SyncStorage>(storage: S) {
    let db = run(Database::open(storage, schema(), vec![])).unwrap();
    let mut storage = db.into_storage();
    let model = run(storage.catalog()).unwrap().state.unwrap();
    let namespace = "04".repeat(16);
    let key = format!("0000000000000000-{namespace}-0000000000000001");
    let mut producer = SyncRecord {
        entity: None,
        metadata_undo: vec![],
        key: key.clone(),
        command: "create-parent-v1".into(),
        args: args("history"),
        context: Json::Object(BTreeMap::from([
            ("identity".into(), Json::Null),
            (
                "capturedAt".into(),
                Json::String("1970-01-01T00:00:00.000Z".into()),
            ),
        ])),
        schema_version: 1,
        order: 1,
        reference_paths: vec![],
        dependencies: BTreeSet::new(),
        created: BTreeMap::from([("local:old".into(), "parents".into())]),
        mappings: BTreeMap::from([("local:old".into(), "server:known".into())]),
        undo: vec![],
        state: SyncState::Applied,
        frozen: None,
        attempts: 1,
        due_ms: 0,
        outcome: Some(RemoteOutcome::Applied(Json::Null)),
        rolled_back: false,
        reconciled: true,
    };
    producer.frozen = Some(producer.envelope().canonical().unwrap());
    let ledger = SyncLedger {
        entity: None,
        header: SyncHeader {
            namespace,
            sequence: 2,
            last_ms: 0,
            options: SyncOptions::default(),
        },
        records: BTreeMap::from([(key, producer)]),
    };
    storage
        .apply_sync_plan(SyncPlan {
            transition: SyncTransition::Normal,
            expected_model: model.clone(),
            next_model: model,
            expected_sync: None,
            next_sync: ledger.to_rows().unwrap(),
            guards: vec![],
            mutations: vec![],
        })
        .unwrap();
    let mut db = run(CommandDatabase::open(
        storage,
        schema(),
        vec![],
        [99; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    let request = db.request().unwrap();
    let definition = CommandDefinition {
        reference_paths: vec![vec![PathPart::Key("target".into())]],
        ..definition()
    };
    let args = Json::Object(BTreeMap::from([
        ("label".into(), Json::String("local:old".into())),
        ("target".into(), Json::String("local:old".into())),
    ]));
    assert!(run(db.begin(request, definition, args, identity(), 0)).unwrap());
    let Json::Object(args) = db.scope_args(request).unwrap() else {
        panic!("object args lost")
    };
    assert_eq!(args["target"], Json::String("server:known".into()));
    assert_eq!(args["label"], Json::String("local:old".into()));
    db.abort(request).unwrap();
}

fn retired_reference<S: SyncStorage>(storage: S) {
    let storage = run(Database::open(storage, schema(), vec![]))
        .unwrap()
        .into_storage();
    let mut storage = storage;
    let model = run(storage.catalog()).unwrap().state.unwrap();
    let namespace = "1f".repeat(16);
    let key = format!("0000000000000000-{namespace}-0000000000000001");
    let mut record = SyncRecord {
        entity: None,
        metadata_undo: vec![],
        key: key.clone(),
        command: "create-parent-v1".into(),
        args: args("producer"),
        context: Json::Object(BTreeMap::from([
            ("identity".into(), Json::Null),
            (
                "capturedAt".into(),
                Json::String("1970-01-01T00:00:00.000Z".into()),
            ),
        ])),
        schema_version: 1,
        order: 1,
        reference_paths: vec![],
        dependencies: BTreeSet::new(),
        created: BTreeMap::from([("local:dead".into(), "parents".into())]),
        mappings: BTreeMap::new(),
        undo: vec![],
        state: SyncState::Cancelled,
        frozen: None,
        attempts: 1,
        due_ms: 1000,
        outcome: None,
        rolled_back: true,
        reconciled: true,
    };
    record.frozen = Some(record.envelope().canonical().unwrap());
    let ledger = SyncLedger {
        entity: None,
        header: SyncHeader {
            namespace,
            sequence: 2,
            last_ms: 0,
            options: SyncOptions::default(),
        },
        records: BTreeMap::from([(key, record)]),
    };
    storage
        .apply_sync_plan(SyncPlan {
            transition: SyncTransition::Normal,
            expected_model: model.clone(),
            next_model: model,
            expected_sync: None,
            next_sync: ledger.to_rows().unwrap(),
            guards: vec![],
            mutations: vec![],
        })
        .unwrap();
    let mut db = run(CommandDatabase::open(
        storage,
        schema(),
        vec![],
        [31; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    let before = db.queue_status();
    let r = db.request().unwrap();
    let definition = CommandDefinition {
        reference_paths: vec![vec![PathPart::Key("target".into())]],
        ..definition()
    };
    let args = Json::Object(BTreeMap::from([
        ("label".into(), Json::String("child".into())),
        ("target".into(), Json::String("local:dead".into())),
    ]));
    assert_eq!(
        run(db.begin(r, definition, args, Some(Json::Null), 1)),
        Err(CommandError::UnknownLocalReference)
    );
    assert_eq!(db.queue_status(), before);
    assert!(db.scope_args(r).is_err());
    db.close();
}
