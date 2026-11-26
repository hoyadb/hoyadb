//! API-0011 / TC-0036: additive core seams shared by real storage adapters.
use hoyadb_core::{command::*, model::*, storage::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    task::{Context, Poll, Waker},
};

pub fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("adapter unexpectedly suspended"),
    }
}

pub fn links_schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([
            (
                "left".into(),
                TableDefinition {
                    columns: BTreeMap::new(),
                    relations: BTreeMap::from([(
                        "rights".into(),
                        RelationDefinition::ManyToMany {
                            table: "right".into(),
                            through: "edges".into(),
                            source_relation: "left".into(),
                            target_relation: "right".into(),
                        },
                    )]),
                },
            ),
            ("right".into(), TableDefinition::default()),
            (
                "edges".into(),
                TableDefinition {
                    columns: BTreeMap::new(),
                    relations: BTreeMap::from([
                        (
                            "left".into(),
                            RelationDefinition::ToOne {
                                table: "left".into(),
                                foreign_key: "left_id".into(),
                                optional: false,
                            },
                        ),
                        (
                            "right".into(),
                            RelationDefinition::ToOne {
                                table: "right".into(),
                                foreign_key: "right_id".into(),
                                optional: false,
                            },
                        ),
                    ]),
                },
            ),
        ]),
    }
}

pub fn proposed_links_and_recovery<S: SyncStorage>(mut fresh: impl FnMut() -> S) {
    let mut db = run(CommandDatabase::open(
        fresh(),
        links_schema(),
        vec![],
        [40; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    let left =
        run(db.insert_proposed("left", BTreeMap::new(), Some("test-fixed-id-001".into()))).unwrap();
    let right =
        run(db.insert_proposed("right", BTreeMap::new(), Some("right:fixed".into()))).unwrap();
    assert_eq!(left.id, "test-fixed-id-001");
    let initial = run(db.query("left").fetch(&db)).unwrap();
    let insert = || Write::Insert {
        table: "left".into(),
        fields: BTreeMap::new(),
    };
    assert!(
        run(db.apply_batch_proposed(vec![
            (insert(), Some("duplicate".into())),
            (insert(), Some("duplicate".into()))
        ]))
        .is_err()
    );
    assert_eq!(run(db.query("left").fetch(&db)).unwrap(), initial);
    let request = db.request().unwrap();
    assert!(
        run(db.begin(
            request,
            CommandDefinition {
                name: "link-v1".into(),
                arguments: BTreeMap::new(),
                reference_paths: vec![]
            },
            Json::Object(BTreeMap::new()),
            Some(Json::Null),
            0
        ))
        .unwrap()
    );
    let edge = run(db.scope_link_proposed(
        request,
        "left",
        &left.id,
        "rights",
        &right.id,
        (BTreeMap::new(), Some("local:scoped-edge".into())),
    ))
    .unwrap();
    assert_eq!(edge.id, "local:scoped-edge");
    assert!(run(db.query("edges").fetch(&db)).unwrap().is_empty());
    db.abort(request).unwrap();
    let edge = run(db.link_proposed(
        "left",
        &left.id,
        "rights",
        &right.id,
        BTreeMap::new(),
        Some("edge:fixed".into()),
    ))
    .unwrap();
    assert_eq!(edge.id, "edge:fixed");
    assert_eq!(
        run(db.link_proposed(
            "left",
            &left.id,
            "rights",
            &right.id,
            BTreeMap::new(),
            Some("discard-unused".into())
        ))
        .unwrap(),
        edge
    );
    assert_eq!(
        run(db.related("left", &left.id, "rights").fetch(&db)).unwrap(),
        vec![right]
    );
    assert_eq!(
        run(db.insert("left", BTreeMap::new())).unwrap().id,
        "local:1",
        "proposals and abort do not consume default counter"
    );
    db.close();

    let mut db = run(CommandDatabase::open(
        fresh(),
        links_schema(),
        vec![],
        [41; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    db.configure_entities(
        EntityConfig {
            scope: "shared-g07".into(),
            tables: BTreeSet::from(["left".into(), "right".into(), "edges".into()]),
        },
        || {
            Ok(EntityContext {
                identity: Json::Null,
                now_ms: 10,
            })
        },
    )
    .unwrap();
    let left = run(db.insert("left", BTreeMap::new())).unwrap();
    let right = run(db.insert("right", BTreeMap::new())).unwrap();
    assert_eq!(
        run(db.link_proposed(
            "left",
            &left.id,
            "rights",
            &right.id,
            BTreeMap::new(),
            Some("entity:forged".into())
        )),
        Err(CommandError::InvalidCommand)
    );
    let edge = run(db.link_proposed("left", &left.id, "rights", &right.id, BTreeMap::new(), None))
        .unwrap();
    assert!(
        edge.id
            .starts_with("entity:29292929292929292929292929292929:")
    );
    assert_eq!(db.queue_status().pending, 3);
    let sent = db.prepare_push(100, 2500).unwrap();
    assert_eq!(
        sent.envelopes.len(),
        2,
        "join waits on unresolved parent entity IDs"
    );
    for entry in db.queue_status().commands {
        if entry.state == SyncState::Sent {
            assert_eq!(entry.attempts, 1);
            assert_eq!(entry.next_due_ms, Some(1350));
        } else {
            assert_eq!(entry.attempts, 0);
            assert_eq!(entry.next_due_ms, None);
        }
    }
    let storage = db.into_storage();
    let db = run(CommandDatabase::open(
        storage,
        links_schema(),
        vec![],
        [99; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    assert_eq!(db.queue_status().sending, 2);
    assert_eq!(db.queue_status().pending, 1);
    assert_eq!(run(db.query("edges").fetch(&db)).unwrap(), vec![edge]);
    for entry in db.queue_status().commands {
        if entry.state == SyncState::Sent {
            assert_eq!(entry.attempts, 1);
            assert_eq!(entry.next_due_ms, Some(1350));
        } else {
            assert_eq!(entry.attempts, 0);
            assert_eq!(entry.next_due_ms, None);
        }
    }
}

pub fn merge_schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([(
            "items".into(),
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
                        "rank".into(),
                        Column {
                            kind: ColumnType::Integer,
                            optional: true,
                        },
                    ),
                ]),
                relations: BTreeMap::new(),
            },
        )]),
    }
}

pub fn change(body: &str, time: i64) -> EntityChange {
    EntityChange {
        table: "items".into(),
        record_id: "record".into(),
        op: EntityOperation::Update,
        changes: BTreeMap::from([("body".into(), Value::String(body.into()))]),
        removed_fields: BTreeSet::new(),
        timestamp_ms: time,
        field_timestamps: BTreeMap::from([("body".into(), time)]),
    }
}

fn hook(input: &MergeInput<'_>) -> Result<EntityResolution, CommandError> {
    let mut result = input.default.clone();
    if let Some(row) = &mut result.row
        && let Some(Value::String(value)) = row.fields.get_mut("body")
    {
        *value = format!("custom-{value}");
    }
    Ok(result)
}

pub fn passive_merge<S: SyncStorage>(mut fresh: impl FnMut() -> S) {
    let open = |store| {
        run(CommandDatabase::open(
            store,
            merge_schema(),
            vec![],
            [42; 16],
            SyncOptions::default(),
        ))
        .unwrap()
    };
    let first = fresh();
    // SPEC-0037 R3: only unproven adapters and WASM share one coordinator.
    let shared = cfg!(target_arch = "wasm32") || first.ownership() == StoreOwnership::Unproven;
    let mut db = open(first);
    let mut other = open(fresh());
    for db in [&mut db, &mut other] {
        db.configure_entities(
            EntityConfig {
                scope: "merge-test".into(),
                tables: BTreeSet::from(["items".into()]),
            },
            || {
                Ok(EntityContext {
                    identity: Json::Null,
                    now_ms: 10,
                })
            },
        )
        .unwrap();
        let request = db.begin_pull().unwrap().request;
        run(db.apply_pull(
            request,
            PullPage {
                checkpoint: "cp1".into(),
                complete: true,
                changes: vec![change("base", 10)],
            },
        ))
        .unwrap();
    }
    let query = db.query("items");
    let subscription = run(db.observe_with_columns(&query, vec!["body".into()])).unwrap();
    db.drain_events(0);
    let page = || PullPage {
        checkpoint: "cp2".into(),
        complete: true,
        changes: vec![change("first", 20), change("second", 30)],
    };
    let tables = || BTreeSet::from(["items".into()]);
    let first = db.begin_pull().unwrap().request;
    let MergeStep::Offer {
        session,
        step,
        ..
    } = run(db.merge_prepare(first, page(), tables())).unwrap()
    else {
        panic!("expected first offer")
    };
    let mut bad = db.merge_input(session, step).unwrap().default.row.unwrap();
    bad.fields.insert("rank".into(), Value::Integer(9));
    assert_eq!(
        run(db.merge_reply(session, step, Some(bad))),
        Err(CommandError::InvalidCommand)
    );
    assert_eq!(
        db.merge_input(session, step),
        Err(CommandError::InvalidTicket)
    );
    assert_eq!(db.sync_progress().pages, 1);
    assert!(db.drain_events(0).is_empty());
    let request = db.begin_pull().unwrap().request;
    let MergeStep::Offer {
        session,
        step,
        ..
    } = run(db.merge_prepare(request, page(), tables())).unwrap()
    else {
        panic!("expected first offer")
    };
    let input = db.merge_input(session, step).unwrap();
    assert_eq!(
        input.current.as_ref().unwrap().fields["body"],
        Value::String("base".into())
    );
    assert_eq!(
        input.default.row.as_ref().unwrap().fields["body"],
        Value::String("first".into())
    );
    let mut custom = input.default.row.unwrap();
    custom
        .fields
        .insert("body".into(), Value::String("custom-first".into()));
    let ticket = other.request().unwrap();
    assert!(
        run(other.begin(
            ticket,
            CommandDefinition {
                name: "wait-v1".into(),
                arguments: BTreeMap::new(),
                reference_paths: vec![]
            },
            Json::Object(BTreeMap::new()),
            Some(Json::Null),
            0
        ))
        .unwrap()
    );
    if shared {
        assert_eq!(
            run(db.merge_reply(session, step, Some(custom.clone()))),
            Err(CommandError::Busy)
        );
        other.abort(ticket).unwrap();
    }
    // Per-database admission: the other database's active handler does not block.
    let MergeStep::Offer {
        session: next,
        step: second,
        ..
    } = run(db.merge_reply(session, step, Some(custom))).unwrap()
    else {
        panic!("expected second offer")
    };
    if !shared {
        other.abort(ticket).unwrap();
    }
    assert_eq!(next, session);
    let input = db.merge_input(session, second).unwrap();
    assert_eq!(
        input.current.as_ref().unwrap().fields["body"],
        Value::String("custom-first".into())
    );
    let mut custom = input.default.row.unwrap();
    custom
        .fields
        .insert("body".into(), Value::String("custom-second".into()));
    assert!(matches!(
        run(db.merge_reply(session, second, Some(custom))),
        Ok(MergeStep::Ready { .. })
    ));
    assert_eq!(
        run(db.query("items").fetch(&db)).unwrap()[0].fields["body"],
        Value::String("base".into())
    );
    run(db.merge_commit(session)).unwrap();
    assert_eq!(
        run(db.merge_commit(session)),
        Err(CommandError::InvalidTicket)
    );
    assert_eq!(db.sync_progress().pages, 2);
    assert_eq!(db.sync_progress().changes, 3);
    let events = db.drain_events(0);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event,CommandEvent::Data(n) if n.subscription==subscription))
            .count(),
        1
    );
    other.set_merge_hook("items", hook).unwrap();
    let request = other.begin_pull().unwrap().request;
    run(other.apply_pull(request, page())).unwrap();
    assert_eq!(
        run(db.query("items").fetch(&db)).unwrap(),
        run(other.query("items").fetch(&other)).unwrap(),
        "passive planning equals existing synchronous hook"
    );
    let request = db.begin_pull().unwrap();
    assert_eq!(request.since, Some("cp2".into()));
    let MergeStep::Offer {
        session,
        step,
        ..
    } = run(db.merge_prepare(
        request.request,
        PullPage {
            checkpoint: "cp3".into(),
            complete: true,
            changes: vec![change("third", 40)],
        },
        tables(),
    ))
    .unwrap()
    else {
        panic!("expected stale offer")
    };
    let row = db.merge_input(session, step).unwrap().default.row;
    assert!(db.prepare_push(50, 0).unwrap().envelopes.is_empty());
    assert_eq!(
        run(db.merge_reply(session, step, row)),
        Err(CommandError::StalePull),
        "header-only durable changes also invalidate full ledger guard"
    );
    let request = db.begin_pull().unwrap();
    assert_eq!(request.since, Some("cp2".into()));
    db.abort_pull(request.request).unwrap();
    assert_eq!(db.sync_progress().pages, 2);
    let candidate_page = || PullPage {
        checkpoint: "cp3".into(),
        complete: true,
        changes: vec![change("third", 40)],
    };
    let request = db.begin_pull().unwrap().request;
    assert_eq!(
        run(db.merge_prepare(
            request,
            candidate_page(),
            BTreeSet::from(["missing".into()])
        )),
        Err(CommandError::InvalidCommand)
    );
    db.abort_pull(request).unwrap();
    let request = db.begin_pull().unwrap().request;
    let MergeStep::Offer {
        session,
        step,
        ..
    } = run(db.merge_prepare(request, candidate_page(), tables())).unwrap()
    else {
        panic!("expected offer")
    };
    assert_eq!(
        other.merge_input(session, step),
        Err(CommandError::InvalidTicket)
    );
    assert_eq!(
        run(db.apply_pull(request, candidate_page())),
        Err(CommandError::Busy)
    );
    assert!(db.merge_input(session, step).is_ok());
    db.abort_pull(request).unwrap();
    assert_eq!(
        run(db.merge_commit(session)),
        Err(CommandError::InvalidTicket)
    );
    let request = db.begin_pull().unwrap().request;
    let MergeStep::Offer {
        session,
        step,
        ..
    } = run(db.merge_prepare(
        request,
        PullPage {
            checkpoint: "tie".into(),
            complete: true,
            changes: vec![change("backend-tied", 30)],
        },
        tables(),
    ))
    .unwrap()
    else {
        panic!("expected tied offer")
    };
    let mut row = db.merge_input(session, step).unwrap().default.row.unwrap();
    row.fields
        .insert("body".into(), Value::String("forged-tie".into()));
    assert_eq!(
        run(db.merge_reply(session, step, Some(row))),
        Err(CommandError::InvalidCommand)
    );
    let request = db.begin_pull().unwrap().request;
    let MergeStep::Offer {
        session,
        step,
        ..
    } = run(db.merge_prepare(request, candidate_page(), tables())).unwrap()
    else {
        panic!("expected offer")
    };
    let row = db.merge_input(session, step).unwrap().default.row;
    assert!(matches!(
        run(db.merge_reply(session, step, row)),
        Ok(MergeStep::Ready { .. })
    ));
    db.prepare_push(60, 0).unwrap();
    assert_eq!(run(db.merge_commit(session)), Err(CommandError::StalePull));
    assert_eq!(db.sync_progress().pages, 2);
    let ticket = db.request().unwrap();
    assert!(
        run(db.begin(
            ticket,
            CommandDefinition {
                name: "receipt-guard-v1".into(),
                arguments: BTreeMap::new(),
                reference_paths: vec![]
            },
            Json::Object(BTreeMap::new()),
            Some(Json::Null),
            100
        ))
        .unwrap()
    );
    let key = run(db.finish(ticket, Json::Null)).unwrap().key;
    assert_eq!(db.prepare_push(100, 0).unwrap().envelopes.len(), 1);
    let request = db.begin_pull().unwrap().request;
    let MergeStep::Offer {
        session,
        step,
        ..
    } = run(db.merge_prepare(request, candidate_page(), tables())).unwrap()
    else {
        panic!("expected receipt guard offer")
    };
    let row = db.merge_input(session, step).unwrap().default.row;
    db.receive(&key, None).unwrap();
    assert_eq!(
        run(db.merge_reply(session, step, row)),
        Err(CommandError::StalePull),
        "queue-only receipt invalidates passive plan even without a data mutation"
    );
    assert_eq!(db.sync_progress().pages, 2);
    let request = db.begin_pull().unwrap().request;
    let MergeStep::Offer {
        session,
        ..
    } = run(db.merge_prepare(request, candidate_page(), tables())).unwrap()
    else {
        panic!("expected abort offer")
    };
    db.merge_abort(session).unwrap();
    assert_eq!(db.merge_abort(session), Err(CommandError::InvalidTicket));
    assert_eq!(db.abort_pull(request), Err(CommandError::InvalidTicket));
    let request = db.begin_pull().unwrap().request;
    let MergeStep::Offer {
        session,
        step,
        ..
    } = run(db.merge_prepare(request, candidate_page(), tables())).unwrap()
    else {
        panic!("expected reset offer")
    };
    run(db.reset(ResetMode::DiscardRecovery)).unwrap();
    assert_eq!(
        db.merge_input(session, step),
        Err(CommandError::InvalidTicket)
    );
    assert_eq!(db.sync_progress().pages, 0);
    db.close();
    other.close();
    let mut db = open(fresh());
    db.configure_entities(
        EntityConfig {
            scope: "merge-protection".into(),
            tables: tables(),
        },
        || {
            Ok(EntityContext {
                identity: Json::Null,
                now_ms: 1000,
            })
        },
    )
    .unwrap();
    let request = db.begin_pull().unwrap().request;
    run(db.apply_pull(
        request,
        PullPage {
            checkpoint: "seed".into(),
            complete: true,
            changes: vec![change("base", 10)],
        },
    ))
    .unwrap();
    run(db.patch(
        "items",
        "record",
        BTreeMap::from([("body".into(), Value::String("optimistic".into()))]),
    ))
    .unwrap();
    let key = db.queue_status().commands[0].key.clone();
    let request = db.begin_pull().unwrap().request;
    assert_eq!(
        run(db.merge_prepare(
            request,
            PullPage {
                checkpoint: "hidden-noop".into(),
                complete: true,
                changes: vec![change("older", 20)]
            },
            tables()
        )),
        Err(CommandError::PullConflict),
        "hidden optimistic no-op cannot consume checkpoint before cancellation"
    );
    db.abort_pull(request).unwrap();
    run(db.cancel(&key)).unwrap();
    let mut delete = change("unused", 50);
    delete.op = EntityOperation::Delete;
    delete.changes.clear();
    delete.field_timestamps.clear();
    let request = db.begin_pull().unwrap().request;
    let MergeStep::Offer {
        session,
        step,
        ..
    } = run(db.merge_prepare(
        request,
        PullPage {
            checkpoint: "deleted".into(),
            complete: true,
            changes: vec![delete],
        },
        tables(),
    ))
    .unwrap()
    else {
        panic!("expected delete offer")
    };
    assert_eq!(db.merge_input(session, step).unwrap().default.row, None);
    assert!(matches!(
        run(db.merge_reply(session, step, None)),
        Ok(MergeStep::Ready { .. })
    ));
    run(db.merge_commit(session)).unwrap();
    let request = db.begin_pull().unwrap().request;
    let MergeStep::Ready {
        session,
        ..
    } = run(db.merge_prepare(
        request,
        PullPage {
            checkpoint: "tombstone-noop".into(),
            complete: true,
            changes: vec![change("resurrect", 100)],
        },
        tables(),
    ))
    .unwrap()
    else {
        panic!("confirmed tombstone skips hook as ordinary resolver does")
    };
    run(db.merge_commit(session)).unwrap();
    assert!(run(db.query("items").fetch(&db)).unwrap().is_empty());
    assert_eq!(db.sync_progress().tombstones, 1);
}

pub fn normalization<S: SyncStorage>(store: S) {
    let mut db = run(CommandDatabase::open(
        store,
        links_schema(),
        vec![],
        [44; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    db.configure_entities(
        EntityConfig {
            scope: "normalization".into(),
            tables: BTreeSet::from(["left".into(), "edges".into()]),
        },
        || {
            Ok(EntityContext {
                identity: Json::Null,
                now_ms: 1,
            })
        },
    )
    .unwrap();
    let request = db.request().unwrap();
    assert!(
        run(db.begin(
            request,
            CommandDefinition {
                name: "normalize-v1".into(),
                arguments: BTreeMap::new(),
                reference_paths: vec![]
            },
            Json::Object(BTreeMap::new()),
            Some(Json::Null),
            1
        ))
        .unwrap()
    );
    let row = run(db.scope_insert_proposed(
        request,
        "left",
        BTreeMap::new(),
        Some("local:producer".into()),
    ))
    .unwrap();
    let key = run(db.finish(request, Json::Null)).unwrap().key;
    db.prepare_push(1, 0).unwrap();
    db.receive(&key, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    let target = format!("server:{}", "mapped".repeat(100));
    run(db.reconcile(
        &key,
        ReconcilePlan {
            mappings: BTreeMap::from([(row.id.clone(), target.clone())]),
            writes: vec![],
        },
    ))
    .unwrap();
    let status = db.queue_status();
    assert_eq!(db.normalize_entity_target("left", &row.id).unwrap(), target);
    assert_eq!(
        db.normalize_entity_target("right", &row.id).unwrap(),
        row.id,
        "unconfigured table does not normalize"
    );
    let fields = BTreeMap::from([
        ("left_id".into(), Value::Reference(row.id.clone())),
        ("label".into(), Value::String(row.id.clone())),
    ]);
    let normalized = db.normalize_entity_fields("edges", fields.clone()).unwrap();
    assert_eq!(normalized["left_id"], Value::Reference(target));
    assert_eq!(
        normalized["label"],
        Value::String(row.id.clone()),
        "only typed Reference cells normalize"
    );
    assert_eq!(
        db.normalize_entity_fields("right", fields.clone()).unwrap(),
        fields
    );
    assert_eq!(
        db.normalize_entity_target("left", "local:unknown"),
        Err(CommandError::UnknownLocalReference)
    );
    assert_eq!(
        db.queue_status(),
        status,
        "readonly normalization leaves durable lifecycle untouched"
    );
}
