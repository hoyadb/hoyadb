//! API-0009 / TC-0034: one production queue and exact entity patch semantics.
use hoyadb_core::{command::entity::*, command::*, model::*, storage::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
    task::{Context, Poll, Waker},
};

pub fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("concrete adapter unexpectedly suspended"),
    }
}

pub fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([
            (
                "notes".into(),
                TableDefinition {
                    columns: BTreeMap::from([
                        (
                            "body".into(),
                            Column {
                                kind: ColumnType::String,
                                optional: false,
                            },
                        ),
                        (
                            "extra".into(),
                            Column {
                                kind: ColumnType::String,
                                optional: true,
                            },
                        ),
                    ]),
                    ..Default::default()
                },
            ),
            ("local".into(), TableDefinition::default()),
        ]),
    }
}

fn fields(body: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([("body".into(), Value::String(body.into()))])
}

pub fn automatic_entities<S: SyncStorage>(fresh: impl Fn() -> S) {
    let mut db = run(CommandDatabase::open(
        fresh(),
        schema(),
        vec![],
        [0x35; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    let clock = Arc::new(AtomicI64::new(100));
    let captured = clock.clone();
    db.configure_entities(
        EntityConfig {
            scope: "test-backend/workspace-a".into(),
            tables: BTreeSet::from(["notes".into()]),
        },
        move || {
            Ok(EntityContext {
                identity: Json::String("trusted-context".into()),
                now_ms: captured.load(Ordering::Relaxed),
            })
        },
    )
    .unwrap();
    let mut initial = fields("one");
    initial.insert("extra".into(), Value::String("remove-me".into()));
    let row = run(db.insert("notes", initial)).unwrap();
    assert_eq!(
        db.queue_status().pending,
        1,
        "E01: an ordinary configured create must atomically enqueue its diff"
    );
    assert!(
        row.id
            .starts_with("entity:35353535353535353535353535353535:")
    );
    assert_eq!(
        db.queue_status().commands[0].kind,
        QueuedWriteKind::Entity,
        "E04 status must distinguish entity recovery"
    );
    assert_eq!(
        db.queue_status().commands[0].table.as_deref(),
        Some("notes")
    );
    assert_eq!(
        db.queue_status().commands[0].record_id.as_deref(),
        Some(row.id.as_str())
    );
    let first = db.queue_status().commands[0].key.clone();
    clock.store(200, Ordering::Relaxed);
    run(db.patch("notes", &row.id, fields("one"))).unwrap();
    assert_eq!(
        db.queue_status().pending,
        2,
        "same-value explicitly written fields still queue"
    );
    assert_eq!(
        db.queue_status().commands[1].blocked_on,
        vec![row.id.clone()]
    );
    let batch = db.prepare_push(200, 0).unwrap();
    assert_eq!(
        batch.envelopes.len(),
        1,
        "dependent patch waits for create outcome"
    );
    let envelope = Json::parse(&batch.envelopes[0]).unwrap();
    let Json::Object(wire) = envelope else {
        panic!("object envelope")
    };
    assert!(!wire.contains_key("idempotencyKey"));
    assert!(!wire.contains_key("command"));
    assert_eq!(wire["writeId"], Json::String(first.clone()));
    assert_eq!(wire["recordId"], Json::String(row.id.clone()));
    db.receive_entity(
        "notes",
        &row.id,
        &first,
        Some(RemoteOutcome::Applied(Json::Null)),
    )
    .unwrap();
    run(db.reconcile(
        &first,
        ReconcilePlan {
            mappings: BTreeMap::from([(row.id.clone(), row.id.clone())]),
            writes: vec![],
        },
    ))
    .unwrap();
    let batch = db.prepare_push(201, 0).unwrap();
    assert_eq!(batch.envelopes.len(), 1);
    let Json::Object(patch) = Json::parse(&batch.envelopes[0]).unwrap() else {
        panic!("object patch")
    };
    assert_eq!(
        patch["changes"],
        Json::Object(BTreeMap::from([(
            "body".into(),
            Json::String("one".into())
        )]))
    );
    let second = db
        .queue_status()
        .commands
        .iter()
        .find(|r| r.key != first)
        .unwrap()
        .key
        .clone();
    db.receive_entity(
        "notes",
        &row.id,
        &second,
        Some(RemoteOutcome::Applied(Json::Null)),
    )
    .unwrap();
    run(db.reconcile(&second, ReconcilePlan::default())).unwrap();
    clock.store(300, Ordering::Relaxed);
    run(db.replace("notes", &row.id, fields("two"))).unwrap();
    let batch = db.prepare_push(300, 0).unwrap();
    let Json::Object(replace) = Json::parse(&batch.envelopes[0]).unwrap() else {
        panic!("object replace")
    };
    assert_eq!(
        replace["removedFields"],
        Json::Array(vec![Json::String("extra".into())])
    );
    let Json::Object(changes) = &replace["changes"] else {
        panic!("changes")
    };
    assert_eq!(changes["extra"], Json::Null);
    let Json::String(third) = &replace["writeId"] else {
        panic!("write id")
    };
    db.receive_entity(
        "notes",
        &row.id,
        third,
        Some(RemoteOutcome::Applied(Json::Null)),
    )
    .unwrap();
    run(db.reconcile(third, ReconcilePlan::default())).unwrap();
    clock.store(350, Ordering::Relaxed);
    run(db.replace("notes", &row.id, fields("two")))
        .expect("explicit same-value replacement succeeds and queues its supplied fields");
    let noop = db.prepare_push(350, 0).unwrap();
    assert_eq!(
        noop.envelopes.len(),
        1,
        "explicit same-value replacement must enqueue"
    );
    let Json::Object(noop) = Json::parse(&noop.envelopes[0]).unwrap() else {
        panic!("object")
    };
    let Json::String(noop_key) = &noop["writeId"] else {
        panic!("key")
    };
    db.receive_entity(
        "notes",
        &row.id,
        noop_key,
        Some(RemoteOutcome::Applied(Json::Null)),
    )
    .unwrap();
    run(db.reconcile(noop_key, ReconcilePlan::default())).unwrap();
    clock.store(400, Ordering::Relaxed);
    run(db.patch("notes", &row.id, fields("next"))).unwrap();
    clock.store(500, Ordering::Relaxed);
    run(db.patch(
        "notes",
        &row.id,
        BTreeMap::from([("extra".into(), Value::String("independent".into()))]),
    ))
    .unwrap();
    let batch = db.prepare_push(500, 0).unwrap();
    assert_eq!(batch.envelopes.len(), 2);
    for envelope in batch.envelopes.iter().rev() {
        let Json::Object(envelope) = Json::parse(envelope).unwrap() else {
            panic!("object")
        };
        let Json::String(key) = &envelope["writeId"] else {
            panic!("write id")
        };
        db.receive_entity(
            "notes",
            &row.id,
            key,
            Some(RemoteOutcome::Applied(Json::Null)),
        )
        .unwrap();
        run(db.reconcile(key,ReconcilePlan::default())).expect("E03 independent same-record Applied outcomes reconcile in either order without overwriting newer local fields");
    }
    let current = run(db.query("notes").fetch(&db)).unwrap().remove(0);
    assert_eq!(current.fields["body"], Value::String("next".into()));
    assert_eq!(current.fields["extra"], Value::String("independent".into()));
    let count = db.queue_status().commands.len();
    let local = run(db.insert("local", BTreeMap::new())).unwrap();
    run(db.replace("local", &local.id, BTreeMap::new())).unwrap();
    run(db.patch("local", &local.id, BTreeMap::new())).unwrap();
    assert!(run(db.remove("local", &local.id)).unwrap());
    assert_eq!(
        db.queue_status().commands.len(),
        count,
        "unconfigured mutation stays local"
    );
}

pub fn admission_and_inverse<S: SyncStorage>(fresh: impl Fn() -> S) {
    let options = SyncOptions {
        capacity: 1,
        history: 0,
        ..Default::default()
    };
    let mut db = run(CommandDatabase::open(
        fresh(),
        schema(),
        vec![],
        [0x36; 16],
        options,
    ))
    .unwrap();
    let clock = Arc::new(AtomicI64::new(-1));
    let captured = clock.clone();
    db.configure_entities(
        EntityConfig {
            scope: "guard-fixture".into(),
            tables: BTreeSet::from(["notes".into()]),
        },
        move || {
            Ok(EntityContext {
                identity: Json::Null,
                now_ms: captured.load(Ordering::Relaxed),
            })
        },
    )
    .unwrap();
    assert!(run(db.insert("notes", fields("invalid-time"))).is_err());
    assert!(run(db.query("notes").fetch(&db)).unwrap().is_empty());
    assert_eq!(db.queue_status().live, 0);
    clock.store(100, Ordering::Relaxed);
    let row = run(db.insert("notes", fields("before"))).unwrap();
    assert!(row.id.ends_with("0000000000000001"));
    let key = db.queue_status().commands[0].key.clone();
    assert_eq!(
        run(db.patch("notes", &row.id, fields("capacity"))),
        Err(CommandError::Capacity)
    );
    assert_eq!(
        run(db.apply_batch(vec![
            Write::Insert {
                table: "local".into(),
                fields: BTreeMap::new()
            },
            Write::Replace {
                table: "notes".into(),
                id: row.id.clone(),
                fields: fields("atomic capacity")
            }
        ])),
        Err(CommandError::Capacity)
    );
    assert!(run(db.query("local").fetch(&db)).unwrap().is_empty());
    db.prepare_push(100, 0).unwrap();
    db.receive_entity(
        "notes",
        &row.id,
        &key,
        Some(RemoteOutcome::Applied(Json::Null)),
    )
    .unwrap();
    run(db.reconcile(
        &key,
        ReconcilePlan {
            mappings: BTreeMap::from([(row.id.clone(), row.id.clone())]),
            writes: vec![],
        },
    ))
    .unwrap();
    assert_eq!(
        db.queue_status().commands.len(),
        0,
        "history zero evicts resolved entity producer"
    );
    clock.store(200, Ordering::Relaxed);
    run(db.patch("notes", &row.id, fields("temporary"))).unwrap();
    let patch = db.queue_status().commands[0].key.clone();
    run(db.cancel(&patch)).unwrap();
    assert_eq!(
        run(db.query("notes").fetch(&db)).unwrap()[0].fields["body"],
        Value::String("before".into())
    );
    assert_eq!(db.queue_status().live, 0);
    clock.store(300, Ordering::Relaxed);
    assert!(run(db.remove("notes", &row.id)).unwrap());
    let deletion = db.queue_status().commands[0].key.clone();
    db.prepare_push(300, 0).unwrap();
    db.receive_entity(
        "notes",
        &row.id,
        &deletion,
        Some(RemoteOutcome::Rejected("not authorized".into())),
    )
    .unwrap();
    run(db.reconcile(&deletion, ReconcilePlan::default())).unwrap();
    run(db.rollback(&deletion)).unwrap();
    assert_eq!(
        db.sync_progress().tombstones,
        0,
        "rejected deletion cannot retain a deletion tombstone"
    );
    assert_eq!(
        run(db.query("notes").fetch(&db)).unwrap()[0].fields["body"],
        Value::String("before".into())
    );
    clock.store(400, Ordering::Relaxed);
    run(db.patch("notes", &row.id, fields("usable"))).unwrap();
    assert_eq!(
        db.queue_status().pending,
        1,
        "same stable ID works after history eviction and rollback"
    );
}

fn apply_page<S: SyncStorage>(
    db: &mut CommandDatabase<S>,
    checkpoint: &str,
    changes: Vec<EntityChange>,
) {
    let request = db.begin_pull().unwrap();
    run(db.apply_pull(
        request.request,
        PullPage {
            checkpoint: checkpoint.into(),
            complete: true,
            changes,
        },
    ))
    .unwrap();
}

fn remote(id: &str, body: &str, extra: &str, time: i64) -> EntityChange {
    EntityChange {
        table: "notes".into(),
        record_id: id.into(),
        op: EntityOperation::Update,
        changes: BTreeMap::from([
            ("body".into(), Value::String(body.into())),
            ("extra".into(), Value::String(extra.into())),
        ]),
        removed_fields: BTreeSet::new(),
        timestamp_ms: time,
        field_timestamps: BTreeMap::from([("body".into(), time), ("extra".into(), time)]),
    }
}

fn queued_change(batch: &PushBatch) -> (String, EntityChange) {
    assert_eq!(batch.envelopes.len(), 1);
    let Json::Object(e) = Json::parse(&batch.envelopes[0]).unwrap() else {
        panic!("wire object")
    };
    let string = |key: &str| match &e[key] {
        Json::String(v) => v.clone(),
        _ => panic!("wire string"),
    };
    let Json::Object(values) = &e["changes"] else {
        panic!("wire fields")
    };
    let fields = values
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                match v {
                    Json::String(v) => Value::String(v.clone()),
                    Json::Null => Value::Null,
                    _ => panic!("fixture string field"),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    // The deterministic fixture clocks below are explicit; compare exact UTC wire time.
    let time = match string("clientTimestamp").as_str() {
        "1970-01-01T00:00:00.010Z" => 10,
        "1970-01-01T00:00:00.020Z" => 20,
        "1970-01-01T00:00:00.030Z" => 30,
        "1970-01-01T00:00:00.040Z" => 40,
        "1970-01-01T00:00:00.050Z" => 50,
        _ => panic!("unexpected fixture time"),
    };
    (
        string("writeId"),
        EntityChange {
            table: string("table"),
            record_id: string("recordId"),
            op: match string("op").as_str() {
                "delete" => EntityOperation::Delete,
                "update" => EntityOperation::Update,
                _ => panic!("fixture op"),
            },
            field_timestamps: fields.keys().map(|k| (k.clone(), time)).collect(),
            changes: fields,
            removed_fields: BTreeSet::new(),
            timestamp_ms: time,
        },
    )
}

// This is a deterministic integrating-backend test double, not a core server.
// Its explicit first-arrival rule supplies authority the client resolver lacks.
fn backend_arrival(state: &mut EntityResolution, incoming: &EntityChange) {
    match resolve_entity(
        &schema(),
        state.row.as_ref(),
        &state.metadata,
        incoming,
        false,
        None,
    ) {
        Ok(next) => *state = next,
        Err(CommandError::AuthorityRequired) => {}
        Err(error) => panic!("backend fixture: {error:?}"),
    }
}

fn accept<S: SyncStorage>(db: &mut CommandDatabase<S>, key: &str) {
    db.receive_entity(
        "notes",
        "server:note",
        key,
        Some(RemoteOutcome::Applied(Json::Null)),
    )
    .unwrap();
    run(db.reconcile(key, ReconcilePlan::default())).unwrap();
}

pub fn two_client_resolution<S: SyncStorage>(mut fresh: impl FnMut() -> S) {
    let mut a = run(CommandDatabase::open(
        fresh(),
        schema(),
        vec![],
        [0x41; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    let mut b = run(CommandDatabase::open(
        fresh(),
        schema(),
        vec![],
        [0x42; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    let ca = Arc::new(AtomicI64::new(10));
    let cb = Arc::new(AtomicI64::new(20));
    for (db, clock) in [(&mut a, ca.clone()), (&mut b, cb.clone())] {
        db.configure_entities(
            EntityConfig {
                scope: "same-identity-backend".into(),
                tables: BTreeSet::from(["notes".into()]),
            },
            move || {
                Ok(EntityContext {
                    identity: Json::String("same-identity".into()),
                    now_ms: clock.load(Ordering::Relaxed),
                })
            },
        )
        .unwrap();
        apply_page(db, "seed", vec![remote("server:note", "base", "base", 1)]);
    }
    let mut backend = resolve_entity(
        &schema(),
        None,
        &RowMetadata::default(),
        &remote("server:note", "base", "base", 1),
        true,
        None,
    )
    .unwrap();
    run(a.patch("notes", "server:note", fields("left"))).unwrap();
    run(b.patch(
        "notes",
        "server:note",
        BTreeMap::from([("extra".into(), Value::String("right".into()))]),
    ))
    .unwrap();
    let (ak, ac) = queued_change(&a.prepare_push(10, 0).unwrap());
    let (bk, bc) = queued_change(&b.prepare_push(20, 0).unwrap());
    backend_arrival(&mut backend, &ac);
    backend_arrival(&mut backend, &bc);
    accept(&mut b, &bk);
    accept(&mut a, &ak);
    let merged = EntityChange {
        field_timestamps: BTreeMap::from([("body".into(), 10), ("extra".into(), 20)]),
        ..remote("server:note", "left", "right", 20)
    };
    for db in [&mut a, &mut b] {
        apply_page(db, "distinct-fields", vec![merged.clone()]);
        assert_eq!(
            run(db.query("notes").fetch(db)).unwrap()[0].fields,
            backend.row.as_ref().unwrap().fields
        );
    }
    ca.store(30, Ordering::Relaxed);
    cb.store(30, Ordering::Relaxed);
    run(a.patch("notes", "server:note", fields("first-arrival"))).unwrap();
    run(b.patch("notes", "server:note", fields("second-arrival"))).unwrap();
    let (ak, ac) = queued_change(&a.prepare_push(30, 0).unwrap());
    let (bk, bc) = queued_change(&b.prepare_push(30, 0).unwrap());
    backend_arrival(&mut backend, &ac);
    assert_eq!(
        resolve_entity(
            &schema(),
            backend.row.as_ref(),
            &backend.metadata,
            &bc,
            false,
            None
        ),
        Err(CommandError::AuthorityRequired)
    );
    backend_arrival(&mut backend, &bc);
    accept(&mut b, &bk);
    accept(&mut a, &ak);
    let winner = EntityChange {
        field_timestamps: BTreeMap::from([("body".into(), 30), ("extra".into(), 20)]),
        ..remote("server:note", "first-arrival", "right", 30)
    };
    for db in [&mut b, &mut a] {
        apply_page(db, "server-tie", vec![winner.clone()]);
        assert_eq!(
            run(db.query("notes").fetch(db)).unwrap()[0].fields["body"],
            Value::String("first-arrival".into())
        );
    }
    ca.store(40, Ordering::Relaxed);
    cb.store(50, Ordering::Relaxed);
    run(a.remove("notes", "server:note")).unwrap();
    run(b.patch("notes", "server:note", fields("too-late"))).unwrap();
    let (ak, ac) = queued_change(&a.prepare_push(40, 0).unwrap());
    let (bk, bc) = queued_change(&b.prepare_push(50, 0).unwrap());
    backend_arrival(&mut backend, &ac);
    backend_arrival(&mut backend, &bc);
    assert!(backend.row.is_none());
    accept(&mut a, &ak);
    accept(&mut b, &bk);
    for db in [&mut a, &mut b] {
        apply_page(db, "deleted", vec![ac.clone()]);
        assert!(run(db.query("notes").fetch(db)).unwrap().is_empty());
        assert_eq!(db.sync_progress().tombstones, 1);
    }
    // The hook runs inside the actual production pull path on only its table.
    fn hook(input: &MergeInput<'_>) -> Result<EntityResolution, CommandError> {
        let mut out = input.default.clone();
        if input.incoming.op == EntityOperation::Update {
            out.row
                .as_mut()
                .unwrap()
                .fields
                .insert("body".into(), Value::String("custom".into()));
        }
        Ok(out)
    }

    fn bad(input: &MergeInput<'_>) -> Result<EntityResolution, CommandError> {
        let mut out = input.default.clone();
        out.row
            .as_mut()
            .unwrap()
            .fields
            .insert("undeclared".into(), Value::String("bad".into()));
        Ok(out)
    }
    let mut c = run(CommandDatabase::open(
        fresh(),
        schema(),
        vec![],
        [0x43; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    c.configure_entities(
        EntityConfig {
            scope: "hook-fixture".into(),
            tables: BTreeSet::from(["notes".into()]),
        },
        || {
            Ok(EntityContext {
                identity: Json::Null,
                now_ms: 1,
            })
        },
    )
    .unwrap();
    apply_page(&mut c, "seed", vec![remote("hook:note", "base", "base", 1)]);
    c.set_merge_hook("notes", hook).unwrap();
    apply_page(
        &mut c,
        "hook",
        vec![remote("hook:note", "incoming", "new", 100)],
    );
    assert_eq!(
        run(c.query("notes").fetch(&c)).unwrap()[0].fields["body"],
        Value::String("custom".into())
    );
    let before = c.sync_progress();
    c.set_merge_hook("notes", bad).unwrap();
    let ticket = c.begin_pull().unwrap();
    assert!(
        run(c.apply_pull(
            ticket.request,
            PullPage {
                checkpoint: "bad-hook".into(),
                complete: true,
                changes: vec![remote("hook:note", "invalid", "newer", 200)]
            }
        ))
        .is_err()
    );
    assert_eq!(c.sync_progress(), before);
    assert_eq!(
        run(c.query("notes").fetch(&c)).unwrap()[0].fields["body"],
        Value::String("custom".into())
    );
    c.abort_pull(ticket.request).unwrap();
}

pub fn managed_links_and_mapped_rows<S: SyncStorage>(mut fresh: impl FnMut() -> S) {
    let graph = SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([
            (
                "authors".into(),
                TableDefinition {
                    relations: BTreeMap::from([(
                        "tags".into(),
                        RelationDefinition::ManyToMany {
                            table: "tags".into(),
                            through: "author_tags".into(),
                            source_relation: "author".into(),
                            target_relation: "tag".into(),
                        },
                    )]),
                    ..Default::default()
                },
            ),
            ("tags".into(), TableDefinition::default()),
            (
                "author_tags".into(),
                TableDefinition {
                    relations: BTreeMap::from([
                        (
                            "author".into(),
                            RelationDefinition::ToOne {
                                table: "authors".into(),
                                foreign_key: "author_id".into(),
                                optional: false,
                            },
                        ),
                        (
                            "tag".into(),
                            RelationDefinition::ToOne {
                                table: "tags".into(),
                                foreign_key: "tag_id".into(),
                                optional: false,
                            },
                        ),
                    ]),
                    ..Default::default()
                },
            ),
        ]),
    };
    let mut db = run(CommandDatabase::open(
        fresh(),
        graph.clone(),
        vec![],
        [0x55; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    let mut id = 0;
    db.set_id_generator(move || {
        id += 1;
        format!("stable:{id}")
    })
    .unwrap();
    db.configure_entities(
        EntityConfig {
            scope: "link-backend".into(),
            tables: BTreeSet::from(["author_tags".into()]),
        },
        || {
            Ok(EntityContext {
                identity: Json::Null,
                now_ms: 100,
            })
        },
    )
    .unwrap();
    let author = run(db.insert("authors", BTreeMap::new())).unwrap();
    let tag = run(db.insert("tags", BTreeMap::new())).unwrap();
    let linked = run(db.link("authors", &author.id, "tags", &tag.id, BTreeMap::new()))
        .expect("E01 managed configured link allocates a valid namespaced entity ID");
    assert!(
        linked
            .id
            .starts_with("entity:55555555555555555555555555555555:")
    );
    let before = db.queue_status();
    assert_eq!(
        run(db.link("authors", &author.id, "tags", &tag.id, BTreeMap::new())).unwrap(),
        linked
    );
    assert_eq!(
        db.queue_status(),
        before,
        "idempotent link must not consume another queue record"
    );
    let key = db.queue_status().commands[0].key.clone();
    assert_eq!(
        run(db.unlink("authors", &author.id, "tags", &tag.id)).unwrap(),
        1
    );
    assert_eq!(
        db.queue_status().commands[1].blocked_on,
        vec![linked.id.clone()]
    );
    run(db.cancel(&key)).unwrap();
    assert_eq!(db.queue_status().live, 0);
    assert!(run(db.query("author_tags").fetch(&db)).unwrap().is_empty());
    db.close();
    let mut mapped = run(CommandDatabase::open(
        fresh(),
        schema(),
        vec![],
        [0x56; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    mapped
        .configure_entities(
            EntityConfig {
                scope: "mapped-backend".into(),
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
    let row = run(mapped.insert("notes", fields("initial"))).unwrap();
    let key = mapped.queue_status().commands[0].key.clone();
    mapped.prepare_push(100, 0).unwrap();
    mapped
        .receive_entity(
            "notes",
            &row.id,
            &key,
            Some(RemoteOutcome::Applied(Json::Null)),
        )
        .unwrap();
    run(mapped.reconcile(
        &key,
        ReconcilePlan {
            mappings: BTreeMap::from([(row.id.clone(), "server:canonical".into())]),
            writes: vec![],
        },
    ))
    .unwrap();
    let replaced = run(mapped.replace("notes", &row.id, fields("mapped"))).unwrap();
    assert_eq!(
        replaced.id, "server:canonical",
        "replacement must return the actual mapped committed row identity"
    );
    assert_eq!(
        run(mapped.query("notes").fetch(&mapped)).unwrap(),
        vec![replaced]
    );
    mapped.close();
    let mut graph = graph;
    graph.tables.get_mut("author_tags").unwrap().columns.insert(
        "label".into(),
        Column {
            kind: ColumnType::String,
            optional: true,
        },
    );
    let mut dependent = run(CommandDatabase::open(
        fresh(),
        graph,
        vec![],
        [0x57; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    dependent
        .configure_entities(
            EntityConfig {
                scope: "parents-backend".into(),
                tables: BTreeSet::from(["author_tags".into()]),
            },
            || {
                Ok(EntityContext {
                    identity: Json::Null,
                    now_ms: 100,
                })
            },
        )
        .unwrap();
    let mut producers = vec![];
    for table in ["authors", "tags"] {
        let ticket = dependent.request().unwrap();
        assert!(
            run(dependent.begin(
                ticket,
                CommandDefinition {
                    name: "producer-v1".into(),
                    arguments: BTreeMap::new(),
                    reference_paths: vec![]
                },
                Json::Object(BTreeMap::new()),
                Some(Json::Null),
                100
            ))
            .unwrap()
        );
        let row = run(dependent.scope_insert(ticket, table, BTreeMap::new())).unwrap();
        let result = run(dependent.finish(ticket, Json::Null)).unwrap();
        producers.push((row.id, result.key));
    }
    run(dependent.link(
        "authors",
        &producers[0].0,
        "tags",
        &producers[1].0,
        BTreeMap::from([("label".into(), Value::String(producers[0].0.clone()))]),
    ))
    .unwrap();
    assert_eq!(
        dependent.queue_status().commands[2].blocked_on.len(),
        2,
        "a managed entity can await two command producers"
    );
    assert_eq!(dependent.prepare_push(100, 0).unwrap().envelopes.len(), 2);
    for ((local, key), canonical) in producers.iter().zip(["server:author", "server:tag"]) {
        dependent
            .receive(key, Some(RemoteOutcome::Applied(Json::Null)))
            .unwrap();
        run(dependent.reconcile(
            key,
            ReconcilePlan {
                mappings: BTreeMap::from([(local.clone(), canonical.into())]),
                writes: vec![],
            },
        ))
        .unwrap();
    }
    let wire = dependent.prepare_push(101, 0).unwrap();
    assert_eq!(wire.envelopes.len(), 1);
    let Json::Object(wire) = Json::parse(&wire.envelopes[0]).unwrap() else {
        panic!("object")
    };
    let Json::Object(changes) = &wire["changes"] else {
        panic!("changes")
    };
    assert_eq!(changes["author_id"], Json::String("server:author".into()));
    assert_eq!(changes["tag_id"], Json::String("server:tag".into()));
    assert_eq!(
        changes["label"],
        Json::String(producers[0].0.clone()),
        "ordinary String payloads are never reference rewrites"
    );
}

pub fn reconfiguration_guards<S: SyncStorage>(fresh: impl Fn() -> S) {
    let mut db = run(CommandDatabase::open(
        fresh(),
        schema(),
        vec![],
        [0x58; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    let config = |scope: &str| EntityConfig {
        scope: scope.into(),
        tables: BTreeSet::from(["notes".into()]),
    };
    db.configure_entities(config("scope-a"), || {
        Ok(EntityContext {
            identity: Json::String("a".into()),
            now_ms: 100,
        })
    })
    .unwrap();
    assert!(
        run(
            db.reconfigure_entities(config("scope-b"), || Ok(EntityContext {
                identity: Json::String("b".into()),
                now_ms: 100
            }))
        )
        .is_err(),
        "reconfiguration requires prior explicit reset"
    );
    run(db.reset(ResetMode::PreserveRecovery)).unwrap();
    let old = db.begin_pull().unwrap();
    assert!(
        db.configure_entities(config("scope-b"), || Ok(EntityContext {
            identity: Json::Null,
            now_ms: 100
        }))
        .is_err(),
        "ordinary mismatched opens never reconfigure"
    );
    run(db.reconfigure_entities(config("scope-b"), || {
        Ok(EntityContext {
            identity: Json::String("b".into()),
            now_ms: 100,
        })
    }))
    .expect("E19 explicit reconfiguration of the empty post-reset shell succeeds");
    assert!(
        run(db.apply_pull(
            old.request,
            PullPage {
                checkpoint: "foreign-old-scope".into(),
                complete: true,
                changes: vec![]
            }
        ))
        .is_err()
    );
    let current = db.begin_pull().unwrap();
    assert_eq!(current.scope, "scope-b");
    db.abort_pull(current.request).unwrap();
    run(db.insert("local", BTreeMap::new())).unwrap();
    let retained = db.begin_pull().unwrap();
    assert!(
        run(
            db.reconfigure_entities(config("scope-c"), || Ok(EntityContext {
                identity: Json::String("c".into()),
                now_ms: 100
            }))
        )
        .is_err(),
        "local-only data cannot be reclassified across scope"
    );
    assert_eq!(run(db.query("local").fetch(&db)).unwrap().len(), 1);
    db.abort_pull(retained.request)
        .expect("failed reconfiguration preserves the outstanding request");
    run(db.insert("notes", fields("provider remains b"))).unwrap();
    let batch = db.prepare_push(100, 0).unwrap();
    let Json::Object(wire) = Json::parse(&batch.envelopes[0]).unwrap() else {
        panic!("object")
    };
    let Json::Object(context) = &wire["context"] else {
        panic!("context")
    };
    assert_eq!(context["identity"], Json::String("b".into()));
}
