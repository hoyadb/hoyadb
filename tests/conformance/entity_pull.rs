//! API-0009 / TC-0034 E11–E19: actual production adapters, atomic pull/reset.
use hoyadb_core::{
    command::{entity::*, records::SyncLedger, *},
    model::*,
    storage::*,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    task::{Context, Poll, Waker},
};
pub fn run<F: Future>(f: F) -> F::Output {
    match std::pin::pin!(f).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("adapter suspended"),
    }
}
pub fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([
            (
                "items".into(),
                TableDefinition {
                    columns: BTreeMap::from([(
                        "label".into(),
                        Column {
                            kind: ColumnType::String,
                            optional: false,
                        },
                    )]),
                    ..Default::default()
                },
            ),
            ("local".into(), TableDefinition::default()),
        ]),
    }
}
fn find<S: SyncStorage>(db: &CommandDatabase<S>, table: &str, id: &str) -> Option<Row> {
    run(db.fetch(
        &db.query(table)
            .where_in("id", vec![Value::String(id.into())]),
    ))
    .unwrap()
    .into_iter()
    .next()
}
pub fn configured<S: SyncStorage>(store: S) -> CommandDatabase<S> {
    let mut db = run(CommandDatabase::open(
        store,
        schema(),
        vec![],
        [0x71; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    db.configure_entities(
        EntityConfig {
            scope: "backend:a".into(),
            tables: BTreeSet::from(["items".into()]),
        },
        || {
            Ok(EntityContext {
                identity: Json::Null,
                now_ms: 100,
            })
        },
    )
    .unwrap();
    db
}
pub fn change(table: &str, id: &str, label: Option<&str>) -> EntityChange {
    let changes: BTreeMap<String, Value> = label
        .map(|v| BTreeMap::from([("label".into(), Value::String(v.into()))]))
        .unwrap_or_default();
    EntityChange {
        table: table.into(),
        record_id: id.into(),
        op: EntityOperation::Create,
        field_timestamps: changes.keys().map(|k| (k.clone(), 100)).collect(),
        changes,
        removed_fields: BTreeSet::new(),
        timestamp_ms: 100,
    }
}
pub fn page(checkpoint: &str, complete: bool, changes: Vec<EntityChange>) -> PullPage {
    PullPage {
        checkpoint: checkpoint.into(),
        complete,
        changes,
    }
}
pub static OWNERS: std::sync::Mutex<()> = std::sync::Mutex::new(());
pub fn pull_atomicity_bootstrap_and_reset<S: SyncStorage>(store: S) {
    let _serial = OWNERS.lock().unwrap();
    let mut db = configured(store);
    let token = db.observe_progress().unwrap();
    db.drain_events(0);
    let request = db.begin_pull().unwrap().request;
    run(db.apply_pull(
        request,
        page("one", false, vec![change("items", "r", Some("first"))]),
    ))
    .unwrap();
    assert_eq!(
        find(&db, "items", "r").unwrap().fields["label"],
        Value::String("first".into())
    );
    assert_eq!(
        db.queue_status().pending,
        0,
        "inbound pages do not enqueue outgoing diffs"
    );
    assert_eq!(db.sync_progress().pages, 1);
    assert!(!db.sync_progress().ready);
    assert!(
        db.drain_events(0)
            .iter()
            .any(|e| matches!(e,CommandEvent::Progress {subscription,..} if *subscription==token))
    );
    let second = db.begin_pull().unwrap();
    assert_eq!(second.since, Some("one".into()));
    let mut invalid = change("items", "bad", Some("bad"));
    invalid.changes.insert("undeclared".into(), Value::Null);
    invalid.field_timestamps.insert("undeclared".into(), 100);
    assert!(
        run(db.apply_pull(
            second.request,
            page(
                "two",
                true,
                vec![change("items", "new", Some("new")), invalid]
            )
        ))
        .is_err()
    );
    assert!(find(&db, "items", "new").is_none());
    assert_eq!(db.sync_progress().pages, 1);
    assert!(db.drain_events(0).is_empty());
    run(db.apply_pull(
        second.request,
        page("two", true, vec![change("local", "backend-local", None)]),
    ))
    .expect("backend membership may include a declared table without automatic outgoing diffs");
    assert!(find(&db, "local", "backend-local").is_some());
    assert_eq!(db.sync_progress().pages, 2);
    assert!(db.sync_progress().ready);
    let third = db.begin_pull().unwrap();
    run(db.apply_pull(third.request, page("three", false, vec![]))).unwrap();
    assert!(db.sync_progress().ready, "readiness remains sticky");
    run(db.reset(ResetMode::PreserveRecovery)).unwrap();
    assert!(db.drain_events(0).is_empty());
    assert!(!db.unsubscribe_sync(token));
    assert!(find(&db, "items", "r").is_none());
    assert!(find(&db, "local", "backend-local").is_none());
    assert_eq!(db.sync_progress().pages, 0);
    assert_eq!(db.begin_pull().unwrap().since, None);
}
pub fn reset_inventory<S: SyncStorage>(store: S) {
    let mut store = run(Database::open(store, schema(), vec![]))
        .unwrap()
        .into_storage();
    let model = run(store.catalog()).unwrap().state.unwrap();
    let ledger = SyncLedger {
        entity: Some(EntityLedger::new(EntityConfig {
            scope: "backend:a".into(),
            tables: BTreeSet::from(["items".into()]),
        })),
        header: hoyadb_core::command::records::SyncHeader {
            namespace: "a".repeat(32),
            sequence: 1,
            last_ms: 0,
            options: SyncOptions::default(),
        },
        records: BTreeMap::new(),
    };
    let mut forged_initial = ledger.clone();
    forged_initial.entity.as_mut().unwrap().generation = 2;
    assert!(
        store
            .apply_sync_plan(SyncPlan {
                transition: SyncTransition::Normal,
                expected_model: model.clone(),
                next_model: model.clone(),
                expected_sync: None,
                next_sync: forged_initial.to_rows().unwrap(),
                guards: vec![],
                mutations: vec![]
            })
            .is_err(),
        "initial entity binding begins at generation one"
    );
    let original = ledger.to_rows().unwrap();
    store
        .apply_sync_plan(SyncPlan {
            transition: SyncTransition::Normal,
            expected_model: model.clone(),
            next_model: model.clone(),
            expected_sync: None,
            next_sync: original.clone(),
            guards: vec![],
            mutations: vec![],
        })
        .unwrap();

    let mut forged = ledger.clone();
    forged.entity.as_mut().unwrap().generation += 1;
    let invalid = SyncPlan {
        transition: SyncTransition::Normal,
        expected_model: model.clone(),
        next_model: model.clone(),
        expected_sync: Some(original.clone()),
        next_sync: forged.to_rows().unwrap(),
        guards: vec![],
        mutations: vec![],
    };
    assert!(
        store.apply_sync_plan(invalid).is_err(),
        "normal transition cannot advance reset generation"
    );
    let row = Row {
        id: "late".into(),
        fields: BTreeMap::from([
            ("label".into(), Value::String("preserve".into())),
            (ROW_VERSION.into(), Value::Integer(1)),
        ]),
    };
    run(store.insert("items", row.clone())).unwrap();
    let mut next = ledger.clone();
    let e = next.entity.as_mut().unwrap();
    e.generation += 1;
    e.revision += 1;
    let mut plan = SyncPlan {
        transition: SyncTransition::Reset {
            discard_recovery: true,
        },
        expected_model: model.clone(),
        next_model: model,
        expected_sync: Some(original.clone()),
        next_sync: next.to_rows().unwrap(),
        guards: vec![],
        mutations: vec![],
    };
    assert!(
        store.apply_sync_plan(plan.clone()).is_err(),
        "E17 adapter must reject reset that omits an actual row"
    );
    assert_eq!(run(store.get("items", "late")).unwrap(), Some(row.clone()));
    check_sync_rows(store.load_sync().unwrap().as_deref(), Some(&original)).unwrap();
    plan.guards.push(RowGuard {
        table: "items".into(),
        id: "late".into(),
        expected: Some(row),
    });
    plan.mutations.push(Mutation::Remove {
        table: "items".into(),
        id: "late".into(),
    });
    store.apply_sync_plan(plan).unwrap();
    assert!(run(store.scan("items")).unwrap().is_empty());
}

pub fn lifecycle_comparison_guard<S: SyncStorage>(store: S) {
    let mut store = run(Database::open(store, schema(), vec![]))
        .unwrap()
        .into_storage();
    let model = run(store.catalog()).unwrap().state.unwrap();
    let mut ledger = SyncLedger {
        entity: Some(EntityLedger::new(EntityConfig {
            scope: "backend:a".into(),
            tables: BTreeSet::from(["items".into()]),
        })),
        header: hoyadb_core::command::records::SyncHeader {
            namespace: "b".repeat(32),
            sequence: 1,
            last_ms: 0,
            options: SyncOptions::default(),
        },
        records: BTreeMap::new(),
    };
    store
        .apply_sync_plan(SyncPlan {
            transition: SyncTransition::Normal,
            expected_model: model.clone(),
            next_model: model.clone(),
            expected_sync: None,
            next_sync: ledger.to_rows().unwrap(),
            guards: vec![],
            mutations: vec![],
        })
        .unwrap();
    let expected = store.load_sync().unwrap();
    let e = ledger.entity.as_mut().unwrap();
    e.revision = 1;
    e.metadata.insert(
        ("items".into(), "gone".into()),
        RowMetadata {
            fields: BTreeMap::new(),
            tombstone: Some(FieldStamp {
                timestamp_ms: 100,
                authoritative: true,
            }),
        },
    );
    store
        .apply_sync_plan(SyncPlan {
            transition: SyncTransition::Normal,
            expected_model: model.clone(),
            next_model: model.clone(),
            expected_sync: expected,
            next_sync: ledger.to_rows().unwrap(),
            guards: vec![],
            mutations: vec![],
        })
        .unwrap();
    let mut definition = schema();
    definition.version = 2;
    let next_model = ModelState {
        version: 2,
        definition: definition.canonical().unwrap(),
        next_id: model.next_id,
    };
    let schema_plan = SchemaPlan {
        expected: Some(model.clone()),
        next: next_model,
        operations: vec![],
        schema: definition.storage_schema().unwrap(),
    };
    assert!(
        run(store.apply_schema_plan(schema_plan.clone())).is_err(),
        "E19 confirmed comparison tombstones still block migration"
    );
    let expected = store.load_sync().unwrap();
    let e = ledger.entity.as_mut().unwrap();
    e.revision += 1;
    e.generation += 1;
    e.metadata.clear();
    store
        .apply_sync_plan(SyncPlan {
            transition: SyncTransition::Reset {
                discard_recovery: false,
            },
            expected_model: model.clone(),
            next_model: model,
            expected_sync: expected,
            next_sync: ledger.to_rows().unwrap(),
            guards: vec![],
            mutations: vec![],
        })
        .unwrap();
    let mut removed = definition.clone();
    removed.tables.remove("items");
    let invalid_target = SchemaPlan {
        expected: Some(run(store.catalog()).unwrap().state.unwrap()),
        next: ModelState {
            version: 2,
            definition: removed.canonical().unwrap(),
            next_id: 1,
        },
        schema: removed.storage_schema().unwrap(),
        operations: vec![SchemaMutation::DropTable("items".into())],
    };
    assert!(
        run(store.apply_schema_plan(invalid_target)).is_err(),
        "pristine config must still validate against the target schema before migration"
    );
    run(store.apply_schema_plan(schema_plan))
        .expect("pristine reset binding shell is not a migration block");
}

pub fn pull_recovery_protection<S: SyncStorage>(store: S) {
    let _serial = OWNERS.lock().unwrap();
    let mut db = configured(store);
    let local = run(db.insert(
        "items",
        BTreeMap::from([("label".into(), Value::String("draft".into()))]),
    ))
    .unwrap();
    let request = db.begin_pull().unwrap().request;
    let mut equal = change("items", &local.id, Some("draft"));
    equal.op = EntityOperation::Update;
    assert_eq!(
        run(db.apply_pull(request, page("one", true, vec![equal.clone()]))),
        Err(CommandError::PullConflict),
        "equal values cannot overwrite protected comparison authority"
    );
    assert_eq!(
        run(db.apply_pull(
            request,
            page(
                "one",
                true,
                vec![change("items", "remote-new", Some("remote"))]
            )
        )),
        Err(CommandError::PullConflict),
        "unresolved producer protects table insertion against unknown final mapping collision"
    );
    assert_eq!(db.sync_progress().pages, 0);
    db.abort_pull(request).unwrap();
    let key = db.queue_status().commands[0].key.clone();
    db.prepare_push(100, 0).unwrap();
    db.receive_entity(
        "items",
        &local.id,
        &key,
        Some(RemoteOutcome::Applied(Json::Null)),
    )
    .unwrap();
    run(db.reconcile(
        &key,
        ReconcilePlan {
            mappings: BTreeMap::from([(local.id.clone(), local.id.clone())]),
            writes: vec![],
        },
    ))
    .unwrap();
    let request = db.begin_pull().unwrap().request;
    run(db.apply_pull(request, page("one", true, vec![equal]))).unwrap();
    assert_eq!(db.sync_progress().pages, 1);

    let backend = |id: &str, label: &str, time: i64| EntityChange {
        table: "items".into(),
        record_id: id.into(),
        op: EntityOperation::Update,
        changes: BTreeMap::from([("label".into(), Value::String(label.into()))]),
        removed_fields: BTreeSet::new(),
        timestamp_ms: time,
        field_timestamps: BTreeMap::from([("label".into(), time)]),
    };
    let request = db.begin_pull().unwrap().request;
    run(db.apply_pull(
        request,
        page(
            "seed10",
            true,
            vec![
                backend("remote:deleted", "old10", 10),
                backend("remote:clock", "old10", 10),
            ],
        ),
    ))
    .unwrap();
    db.observe_progress().unwrap();
    db.drain_events(0);
    run(db.remove("items", "remote:deleted")).unwrap();
    let deletion = db.queue_status().commands.last().unwrap().key.clone();
    db.drain_events(0);
    let prior = db.sync_progress();
    let request = db.begin_pull().unwrap().request;
    let incoming = page(
        "server20",
        true,
        vec![backend("remote:deleted", "server20", 20)],
    );
    assert_eq!(
        run(db.apply_pull(request, incoming.clone())),
        Err(CommandError::PullConflict),
        "optimistic tombstone must not consume a backend update checkpoint"
    );
    assert_eq!(db.sync_progress(), prior);
    assert!(db.drain_events(0).is_empty());
    assert!(find(&db, "items", "remote:deleted").is_none());
    db.abort_pull(request).unwrap();
    run(db.cancel(&deletion)).unwrap();
    assert_eq!(
        find(&db, "items", "remote:deleted").unwrap().fields["label"],
        Value::String("old10".into())
    );
    let request = db.begin_pull().unwrap();
    assert_eq!(request.since, Some("seed10".into()));
    run(db.apply_pull(request.request, incoming)).unwrap();
    assert_eq!(
        find(&db, "items", "remote:deleted").unwrap().fields["label"],
        Value::String("server20".into())
    );

    run(db.patch(
        "items",
        "remote:clock",
        BTreeMap::from([("label".into(), Value::String("local100".into()))]),
    ))
    .unwrap();
    let edit = db.queue_status().commands.last().unwrap().key.clone();
    db.drain_events(0);
    let prior = db.sync_progress();
    let request = db.begin_pull().unwrap().request;
    let incoming = page(
        "clock20",
        true,
        vec![backend("remote:clock", "server20", 20)],
    );
    assert_eq!(
        run(db.apply_pull(request, incoming.clone())),
        Err(CommandError::PullConflict),
        "optimistic field clock must not hide a backend value until rollback"
    );
    assert_eq!(db.sync_progress(), prior);
    assert!(db.drain_events(0).is_empty());
    db.abort_pull(request).unwrap();
    run(db.cancel(&edit)).unwrap();
    let request = db.begin_pull().unwrap();
    assert_eq!(request.since, Some("server20".into()));
    run(db.apply_pull(request.request, incoming)).unwrap();
    assert_eq!(
        find(&db, "items", "remote:clock").unwrap().fields["label"],
        Value::String("server20".into())
    );

    let confirmed = EntityChange {
        table: "items".into(),
        record_id: "remote:confirmed".into(),
        op: EntityOperation::Delete,
        changes: BTreeMap::new(),
        removed_fields: BTreeSet::new(),
        timestamp_ms: 30,
        field_timestamps: BTreeMap::new(),
    };
    let request = db.begin_pull().unwrap().request;
    run(db.apply_pull(request, page("confirmed30", true, vec![confirmed]))).unwrap();
    // A net-zero command leaves live row undo protecting the confirmed deletion.
    db.set_id_generator(|| "remote:confirmed".into()).unwrap();
    let command = db.request().unwrap();
    assert!(
        run(db.begin(
            command,
            CommandDefinition {
                name: "guard-v1".into(),
                arguments: BTreeMap::new(),
                reference_paths: vec![]
            },
            Json::Object(BTreeMap::new()),
            Some(Json::Null),
            100
        ))
        .unwrap()
    );
    let temporary = run(db.scope_insert(
        command,
        "items",
        BTreeMap::from([("label".into(), Value::String("temporary".into()))]),
    ))
    .unwrap();
    run(db.scope_remove(command, "items", &temporary.id)).unwrap();
    run(db.finish(command, Json::Null)).unwrap();
    let request = db.begin_pull().unwrap().request;
    run(db.apply_pull(
        request,
        page(
            "obsolete40",
            true,
            vec![backend("remote:confirmed", "obsolete", 40)],
        ),
    ))
    .expect("authoritative tombstone no-op remains nonblocking despite live row undo");
    assert!(find(&db, "items", "remote:confirmed").is_none());
}

/// API-0009 section 4: insertion protection prevents lost-outcome mapping collisions. A
/// pulled Delete for an identity this device never saw would leave a tombstone on the
/// identity a pending create can still be mapped to, so it conflicts like an insertion.
pub fn pull_tombstone_mapping_target<S: SyncStorage>(store: S) {
    let _serial = OWNERS.lock().unwrap();
    let mut db = configured(store);
    let deletion = |id: &str| EntityChange {
        table: "items".into(),
        record_id: id.into(),
        op: EntityOperation::Delete,
        changes: BTreeMap::new(),
        removed_fields: BTreeSet::new(),
        timestamp_ms: 300,
        field_timestamps: BTreeMap::new(),
    };
    let local = run(db.insert(
        "items",
        BTreeMap::from([("label".into(), Value::String("draft".into()))]),
    ))
    .unwrap();
    let key = db.queue_status().commands[0].key.clone();
    db.prepare_push(100, 0).unwrap();
    // The backend stored the row as server:9; another client deleted it before this
    // device processed the create's outcome.
    let request = db.begin_pull().unwrap().request;
    assert_eq!(
        run(db.apply_pull(request, page("deleted", true, vec![deletion("server:9")]))),
        Err(CommandError::PullConflict),
        "a first-seen tombstone must not occupy the pending create's mapping target"
    );
    assert_eq!(db.sync_progress().pages, 0);
    assert_eq!(db.sync_progress().tombstones, 0);
    db.abort_pull(request).unwrap();
    db.receive_entity(
        "items",
        &local.id,
        &key,
        Some(RemoteOutcome::Applied(Json::Null)),
    )
    .unwrap();
    run(db.reconcile(
        &key,
        ReconcilePlan {
            mappings: BTreeMap::from([(local.id.clone(), "server:9".into())]),
            writes: vec![],
        },
    ))
    .expect("the accepted create keeps its mapping exit");
    assert!(find(&db, "items", &local.id).is_none());
    assert!(find(&db, "items", "server:9").is_some());
    let request = db.begin_pull().unwrap().request;
    run(db.apply_pull(request, page("deleted", true, vec![deletion("server:9")]))).unwrap();
    assert!(find(&db, "items", "server:9").is_none());
    assert_eq!(db.sync_progress().tombstones, 1);
    // Without an unmapped producer a first-seen tombstone remains an ordinary change.
    let request = db.begin_pull().unwrap().request;
    run(db.apply_pull(
        request,
        page("unseen", true, vec![deletion("server:unseen")]),
    ))
    .unwrap();
    assert_eq!(db.sync_progress().tombstones, 2);
}
/// API-0009 section 4: a sent cancellation that completed its rollback keeps a live
/// tombstone until forget, but it has no local row and no future mapping, so it does not
/// make pulled insertion into its table a mapping collision.
pub fn pull_after_rolled_back_sent_cancellation<S: SyncStorage>(store: S) {
    let _serial = OWNERS.lock().unwrap();
    let mut db = configured(store);
    let local = run(db.insert(
        "items",
        BTreeMap::from([("label".into(), Value::String("draft".into()))]),
    ))
    .unwrap();
    let key = db.queue_status().commands[0].key.clone();
    assert_eq!(db.prepare_push(100, 0).unwrap().envelopes.len(), 1);
    let remote = || {
        page(
            "one",
            true,
            vec![change("items", "remote-new", Some("remote"))],
        )
    };
    // Control: the sent, still-mappable create protects insertion.
    let request = db.begin_pull().unwrap().request;
    assert_eq!(
        run(db.apply_pull(request, remote())),
        Err(CommandError::PullConflict)
    );
    db.abort_pull(request).unwrap();
    run(db.cancel(&key)).unwrap();
    assert!(find(&db, "items", &local.id).is_none());
    let status = db.queue_status();
    assert_eq!(status.live, 1, "the sent tombstone stays live until forget");
    assert!(status.commands[0].rolled_back);
    let request = db.begin_pull().unwrap().request;
    // The tombstone's own created identity keeps its protection.
    assert_eq!(
        run(db.apply_pull(
            request,
            page("one", true, vec![change("items", &local.id, Some("echo"))])
        )),
        Err(CommandError::PullConflict)
    );
    run(db.apply_pull(request, remote()))
        .expect("a rolled-back sent tombstone does not block unrelated pulled rows");
    assert_eq!(
        find(&db, "items", "remote-new").unwrap().fields["label"],
        Value::String("remote".into())
    );
    assert_eq!(db.sync_progress().pages, 1);
    assert_eq!(db.queue_status().live, 1);
}
pub fn progress_transition_guard<S: SyncStorage>(store: S) {
    let mut store = run(Database::open(store, schema(), vec![]))
        .unwrap()
        .into_storage();
    let model = run(store.catalog()).unwrap().state.unwrap();
    let mut ledger = SyncLedger {
        entity: Some(EntityLedger::new(EntityConfig {
            scope: "backend:a".into(),
            tables: BTreeSet::from(["items".into()]),
        })),
        header: hoyadb_core::command::records::SyncHeader {
            namespace: "c".repeat(32),
            sequence: 1,
            last_ms: 0,
            options: SyncOptions::default(),
        },
        records: BTreeMap::new(),
    };
    let commit = |store: &mut S, ledger: &SyncLedger| {
        let expected = store.load_sync().unwrap();
        store.apply_sync_plan(SyncPlan {
            transition: SyncTransition::Normal,
            expected_model: model.clone(),
            next_model: model.clone(),
            expected_sync: expected,
            next_sync: ledger.to_rows().unwrap(),
            guards: vec![],
            mutations: vec![],
        })
    };
    commit(&mut store, &ledger).unwrap();
    let entity = ledger.entity.as_mut().unwrap();
    entity.pages = 1;
    entity.changes = 2;
    entity.ready = true;
    entity.checkpoint = Some("cursor".into());
    commit(&mut store, &ledger).unwrap();
    let persisted = store.load_sync().unwrap();
    let mut invalid = ledger.clone();
    let e = invalid.entity.as_mut().unwrap();
    e.ready = false;
    e.pages = 0;
    e.changes = 0;
    e.checkpoint = None;
    assert!(
        commit(&mut store, &invalid).is_err(),
        "only explicit reset can erase bootstrap readiness/counts"
    );
    check_sync_rows(store.load_sync().unwrap().as_deref(), persisted.as_deref()).unwrap();
}

pub fn reset_modes_and_coordinator_guard<S: SyncStorage>(fresh: impl Fn() -> S) {
    let _serial = OWNERS.lock().unwrap();
    let first = fresh();
    // SPEC-0037 R3: only unproven adapters and WASM share one coordinator.
    let shared = cfg!(target_arch = "wasm32") || first.ownership() == StoreOwnership::Unproven;
    let mut db = configured(first);
    let row = run(db.insert(
        "items",
        BTreeMap::from([("label".into(), Value::String("offline".into()))]),
    ))
    .unwrap();
    let progress = db.observe_progress().unwrap();
    let status = db.observe_queue_status().unwrap();
    let query = db.query("items");
    let data = run(db.observe(&query)).unwrap();
    let other = configured(fresh());
    let queued = other.request().unwrap();
    if shared {
        assert_eq!(
            run(db.reset(ResetMode::DiscardRecovery)),
            Err(CommandError::Busy)
        );
        other.cancel_request(queued).unwrap();
    }
    // Per database this holds even while the other owner's request is still queued.
    assert_eq!(
        run(db.reset(ResetMode::PreserveRecovery)),
        Err(CommandError::Protocol)
    );
    if !shared {
        other.cancel_request(queued).unwrap();
    }
    assert_eq!(db.queue_status().pending, 1);
    assert_eq!(find(&db, "items", &row.id), Some(row.clone()));
    let events = db.drain_events(0);
    assert!(
        events.iter().any(
            |e| matches!(e,CommandEvent::Progress {subscription,..} if *subscription==progress)
        )
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e,CommandEvent::Status {subscription,..} if *subscription==status))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e,CommandEvent::Data(event) if event.subscription==data))
    );
    let request = db.begin_pull().unwrap().request;
    run(db.reset(ResetMode::DiscardRecovery)).unwrap();
    assert!(db.queue_status().commands.is_empty());
    assert!(find(&db, "items", &row.id).is_none());
    assert!(!db.unsubscribe_sync(progress));
    assert!(!db.unsubscribe_sync(status));
    assert!(!db.unsubscribe(data));
    assert!(db.drain_events(0).is_empty());
    assert_eq!(db.abort_pull(request), Err(CommandError::InvalidTicket));
    let next = run(db.insert(
        "items",
        BTreeMap::from([("label".into(), Value::String("after".into()))]),
    ))
    .unwrap();
    assert_ne!(next.id, row.id);
    assert!(next.id.ends_with(":0000000000000002"));
}

pub fn typed_correction_clocks<S: SyncStorage>(store: S) {
    let _serial = OWNERS.lock().unwrap();
    let mut db = configured(store);
    let row = run(db.insert(
        "items",
        BTreeMap::from([("label".into(), Value::String("optimistic".into()))]),
    ))
    .unwrap();
    let key = db.queue_status().commands[0].key.clone();
    db.prepare_push(100, 0).unwrap();
    db.receive_entity(
        "items",
        &row.id,
        &key,
        Some(RemoteOutcome::Corrected(Json::Null)),
    )
    .unwrap();
    let query = db.query("items");
    let data = run(db.observe_with_columns(&query, vec!["label".into()])).unwrap();
    let outcome = db.observe_outcome(&key).unwrap();
    db.drain_events(0);
    let before = db.begin_pull().unwrap().request;
    assert_eq!(
        run(db.reconcile(
            &key,
            ReconcilePlan {
                mappings: BTreeMap::from([(row.id.clone(), row.id.clone())]),
                writes: vec![ReconcileWrite::Put {
                    table: "items".into(),
                    id: row.id.clone(),
                    fields: BTreeMap::from([("label".into(), Value::String("unclocked".into()))])
                }]
            }
        )),
        Err(CommandError::InvalidCommand)
    );
    assert_eq!(find(&db, "items", &row.id), Some(row.clone()));
    assert!(db.drain_events(0).is_empty());
    let selected = EntityChange {
        table: "items".into(),
        record_id: row.id.clone(),
        op: EntityOperation::Update,
        changes: BTreeMap::from([("label".into(), Value::String("corrected".into()))]),
        removed_fields: BTreeSet::new(),
        timestamp_ms: 50,
        field_timestamps: BTreeMap::from([("label".into(), 50)]),
    };
    run(db.reconcile(
        &key,
        ReconcilePlan {
            mappings: BTreeMap::from([(row.id.clone(), row.id.clone())]),
            writes: vec![ReconcileWrite::Entity {
                change: selected.clone(),
            }],
        },
    ))
    .unwrap();
    assert_eq!(
        find(&db, "items", &row.id).unwrap().fields["label"],
        Value::String("corrected".into())
    );
    let events = db.drain_events(0);
    assert!(
        events
            .iter()
            .any(|e| matches!(e,CommandEvent::Data(event) if event.subscription==data))
    );
    assert!(events.iter().any(|e|matches!(e,CommandEvent::Outcome {subscription,value:CommandOutcome::Corrected(_)} if *subscription==outcome)));
    assert_eq!(
        run(db.apply_pull(before, page("unused", true, vec![]))),
        Err(CommandError::StalePull),
        "correction advances data/comparison revision in its durable commit"
    );
    let mut incoming = selected;
    incoming
        .changes
        .insert("label".into(), Value::String("stale".into()));
    incoming.field_timestamps.insert("label".into(), 40);
    incoming.timestamp_ms = 40;
    let request = db.begin_pull().unwrap().request;
    run(db.apply_pull(request, page("one", true, vec![incoming.clone()]))).unwrap();
    assert_eq!(
        find(&db, "items", &row.id).unwrap().fields["label"],
        Value::String("corrected".into())
    );
    incoming.changes.insert(
        "label".into(),
        Value::String("newer than selected50".into()),
    );
    incoming.field_timestamps.insert("label".into(), 75);
    incoming.timestamp_ms = 75;
    let request = db.begin_pull().unwrap().request;
    run(db.apply_pull(request, page("two", true, vec![incoming]))).unwrap();
    assert_eq!(
        find(&db, "items", &row.id).unwrap().fields["label"],
        Value::String("newer than selected50".into()),
        "selected server50 must replace client100 in comparison state"
    );
    assert_eq!(
        db.queue_status().commands.len(),
        1,
        "correction and pull generate no extra outbound write"
    );
}

pub fn reconfigure_inventory<S: SyncStorage>(store: S) {
    let mut store = run(Database::open(store, schema(), vec![]))
        .unwrap()
        .into_storage();
    let model = run(store.catalog()).unwrap().state.unwrap();
    let mut ledger = SyncLedger {
        entity: Some(EntityLedger::new(EntityConfig {
            scope: "before".into(),
            tables: BTreeSet::from(["items".into()]),
        })),
        header: hoyadb_core::command::records::SyncHeader {
            namespace: "d".repeat(32),
            sequence: 1,
            last_ms: 0,
            options: SyncOptions::default(),
        },
        records: BTreeMap::new(),
    };
    let plan = |store: &S, next: &SyncLedger, transition| SyncPlan {
        transition,
        expected_model: model.clone(),
        next_model: model.clone(),
        expected_sync: store.load_sync().unwrap(),
        next_sync: next.to_rows().unwrap(),
        guards: vec![],
        mutations: vec![],
    };
    store
        .apply_sync_plan(plan(&store, &ledger, SyncTransition::Normal))
        .unwrap();
    let mut changed = ledger.clone();
    changed.entity.as_mut().unwrap().config = EntityConfig {
        scope: "after".into(),
        tables: BTreeSet::new(),
    };
    assert!(
        store
            .apply_sync_plan(plan(&store, &changed, SyncTransition::Normal))
            .is_err(),
        "generation one is not a post-reset reconfiguration shell"
    );
    ledger.entity.as_mut().unwrap().generation = 2;
    ledger.entity.as_mut().unwrap().revision = 1;
    store
        .apply_sync_plan(plan(
            &store,
            &ledger,
            SyncTransition::Reset {
                discard_recovery: false,
            },
        ))
        .unwrap();
    changed = ledger.clone();
    changed.entity.as_mut().unwrap().config = EntityConfig {
        scope: "after".into(),
        tables: BTreeSet::new(),
    };
    let mut counters = plan(&store, &changed, SyncTransition::Normal);
    counters.next_model.next_id += 1;
    assert!(
        store.apply_sync_plan(counters).is_err(),
        "reconfiguration preserves local identity counter exactly"
    );
    let row = Row {
        id: "local:late".into(),
        fields: BTreeMap::from([
            ("label".into(), Value::String("local-only remains".into())),
            (ROW_VERSION.into(), Value::Integer(1)),
        ]),
    };
    run(store.insert("items", row.clone())).unwrap();
    let before = store.load_sync().unwrap();
    assert!(
        store
            .apply_sync_plan(plan(&store, &changed, SyncTransition::Normal))
            .is_err(),
        "adapter must reject reconfiguration if an actual row exists despite pristine comparison shell"
    );
    check_sync_rows(store.load_sync().unwrap().as_deref(), before.as_deref()).unwrap();
    assert_eq!(run(store.get("items", &row.id)).unwrap(), Some(row.clone()));
    run(store.remove("items", &row.id)).unwrap();
    store
        .apply_sync_plan(plan(&store, &changed, SyncTransition::Normal))
        .unwrap();
    assert_eq!(
        SyncLedger::from_rows(&store.load_sync().unwrap().unwrap())
            .unwrap()
            .entity
            .unwrap()
            .config,
        changed.entity.unwrap().config
    );
}
