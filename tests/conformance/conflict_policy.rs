//! TC-0044: behavioral Stop/Discard cases through the public Rust owner.
use hoyadb_core::{command::*, model::*, storage::*};
use std::collections::BTreeMap;
#[allow(dead_code)]
#[path = "command_sync.rs"]
mod base;
pub use base::{enqueue, fields, run};
use hoyadb_core::command::records::{SyncLedger, SyncRecord};

pub fn schema() -> SchemaDefinition {
    let mut schema = base::schema();
    schema.tables.insert(
        "children".into(),
        TableDefinition {
            columns: BTreeMap::from([
                (
                    "name".into(),
                    Column {
                        kind: ColumnType::String,
                        optional: false,
                    },
                ),
                (
                    "parent".into(),
                    Column {
                        kind: ColumnType::Reference,
                        optional: false,
                    },
                ),
            ]),
            relations: BTreeMap::new(),
        },
    );
    schema
}

pub fn open<S: SyncStorage>(storage: S) -> CommandDatabase<S> {
    open_options(storage, SyncOptions::default())
}

pub fn open_options<S: SyncStorage>(storage: S, options: SyncOptions) -> CommandDatabase<S> {
    run(CommandDatabase::open(
        storage,
        schema(),
        vec![],
        [71; 16],
        options,
    ))
    .unwrap()
}

pub fn discard_policy<S: SyncStorage>(db: &mut CommandDatabase<S>) {
    db.set_conflict_policy(ConflictPolicy {
        default: ConflictReaction::Discard,
        commands: BTreeMap::new(),
    })
    .unwrap();
}

pub fn rejected<S: SyncStorage>(db: &mut CommandDatabase<S>, key: &str) {
    db.prepare_push(100, 0).unwrap();
    db.receive(key, Some(RemoteOutcome::Rejected("business rule".into())))
        .unwrap();
    run(db.reconcile(key, ReconcilePlan::default())).unwrap();
}

pub fn child<S: SyncStorage>(
    db: &mut CommandDatabase<S>,
    parent: &str,
) -> Result<(String, Row), CommandError> {
    child_proposed(db, parent, None)
}

fn child_proposed<S: SyncStorage>(
    db: &mut CommandDatabase<S>,
    parent: &str,
    proposed: Option<String>,
) -> Result<(String, Row), CommandError> {
    let request = db.request()?;
    let definition = CommandDefinition {
        name: "child-v1".into(),
        arguments: BTreeMap::from([(
            "parent".into(),
            Argument {
                kind: ArgumentType::String,
                optional: false,
            },
        )]),
        reference_paths: vec![vec![PathPart::Key("parent".into())]],
    };
    run(db.begin(
        request,
        definition,
        Json::Object(BTreeMap::from([(
            "parent".into(),
            Json::String(parent.into()),
        )])),
        Some(Json::Null),
        100,
    ))?;
    let mut values = fields("child");
    values.insert("parent".into(), Value::Reference(parent.into()));
    let row = run(db.scope_insert_proposed(request, "children", values, proposed))?;
    Ok((run(db.finish(request, Json::Null))?.key, row))
}

pub fn rows<S: SyncStorage>(db: &CommandDatabase<S>) -> Vec<Row> {
    let mut rows = run(db.query("parents").fetch(db)).unwrap();
    rows.extend(run(db.query("children").fetch(db)).unwrap());
    rows
}

pub fn capture<S: SyncStorage>(db: CommandDatabase<S>) -> (CommandDatabase<S>, SyncLedger) {
    let storage = db.into_storage();
    let ledger = SyncLedger::from_rows(storage.load_sync().unwrap().as_deref().unwrap()).unwrap();
    let options = ledger.header.options.clone();
    (open_options(storage, options), ledger)
}

fn retired(mut record: SyncRecord) -> SyncRecord {
    record.state = SyncState::Discarded;
    record.reconciled = true;
    record.rolled_back = false;
    record.reference_paths.clear();
    record.dependencies.clear();
    record
}

pub fn edit<S: SyncStorage>(db: &mut CommandDatabase<S>, id: &str, name: &str) -> String {
    let request = db.request().unwrap();
    run(db.begin(
        request,
        base::definition(),
        base::args(name),
        Some(Json::Null),
        100,
    ))
    .unwrap();
    run(db.scope_replace(request, "parents", id, fields(name))).unwrap();
    run(db.finish(request, Json::Null)).unwrap().key
}

fn correction(id: &str, remote: &str) -> ReconcilePlan {
    ReconcilePlan {
        mappings: BTreeMap::from([(id.into(), remote.into())]),
        writes: vec![ReconcileWrite::Put {
            table: "parents".into(),
            id: remote.into(),
            fields: fields("server"),
        }],
    }
}

pub fn cp04<S: SyncStorage>(storage: S) {
    let mut db = open(storage);
    let (key, row) = enqueue(&mut db, "optimistic");
    db.prepare_push(100, 0).unwrap();
    db.receive(&key, Some(RemoteOutcome::Rejected("business rule".into())))
        .unwrap();
    run(db.reconcile(&key, ReconcilePlan::default())).unwrap();
    run(db.replace("parents", &row.id, fields("direct edit"))).unwrap();
    let before = run(db.query("parents").fetch(&db)).unwrap();
    assert_eq!(
        run(db.rollback_resolving(&key)),
        Err(CommandError::RollbackConflict)
    );
    db.set_conflict_policy(ConflictPolicy {
        default: ConflictReaction::Discard,
        commands: BTreeMap::new(),
    })
    .unwrap();
    assert_eq!(
        run(db.rollback_resolving(&key)),
        Ok(Resolution::KeptCurrent {
            discarded_keys: vec![key],
            unmapped: vec![row.id],
            skipped_writes: 0
        })
    );
    assert_eq!(run(db.query("parents").fetch(&db)).unwrap(), before);
    assert_eq!(db.queue_status().live, 0);
    let later = edit(&mut db, &before[0].id, "reissued intent");
    assert_eq!(
        db.queue_status()
            .commands
            .iter()
            .find(|r| r.key == later)
            .unwrap()
            .awaiting_key,
        None
    );
    assert_eq!(
        db.prepare_push(100, 0).unwrap().envelopes.len(),
        1,
        "discard releases same-row dispatch blocker"
    );
    db.close();
}

pub fn cp06<S: SyncStorage>(storage: S) {
    let mut db = open(storage);
    let (key, row) = enqueue(&mut db, "kept");
    assert_eq!(
        run(db.discard(&key)),
        Ok(Resolution::KeptCurrent {
            discarded_keys: vec![key],
            unmapped: vec![row.id.clone()],
            skipped_writes: 0
        })
    );
    assert_eq!(run(db.query("parents").fetch(&db)).unwrap(), vec![row]);
    assert!(db.prepare_push(100, 0).unwrap().envelopes.is_empty());
    assert_eq!(db.queue_status().live, 0);
    db.close();
}

/// CP01/02/03: default and owner reopen Stop; exact names, atomic invalid/Busy configuration.
pub fn configuration<S: SyncStorage>(storage: S) {
    let mut db = open(storage);
    let (key, row) = enqueue(&mut db, "optimistic");
    rejected(&mut db, &key);
    run(db.replace("parents", &row.id, fields("direct"))).unwrap();
    discard_policy(&mut db);
    let (mut db, before) = capture(db);
    assert_eq!(
        run(db.rollback_resolving(&key)),
        Err(CommandError::RollbackConflict),
        "owner reopen restores Stop"
    );
    db.set_conflict_policy(ConflictPolicy {
        default: ConflictReaction::Discard,
        commands: BTreeMap::from([
            ("create-parent-v1".into(), ConflictReaction::Stop),
            ("unknown-v1".into(), ConflictReaction::Discard),
        ]),
    })
    .unwrap();
    assert_eq!(
        run(db.rollback(&key)),
        Err(CommandError::RollbackConflict),
        "exact Stop override wins"
    );
    assert_eq!(
        db.set_conflict_policy(ConflictPolicy {
            default: ConflictReaction::Discard,
            commands: BTreeMap::from([("bad".into(), ConflictReaction::Discard)])
        }),
        Err(CommandError::InvalidCommand)
    );
    assert_eq!(
        run(db.rollback(&key)),
        Err(CommandError::RollbackConflict),
        "invalid update keeps prior policy"
    );
    let oversized = ConflictPolicy {
        default: ConflictReaction::Discard,
        commands: (0..10_000)
            .map(|n| {
                (
                    format!("{}-{n}-v1", "x".repeat(116)),
                    ConflictReaction::Stop,
                )
            })
            .collect(),
    };
    assert_eq!(
        db.set_conflict_policy(oversized),
        Err(CommandError::LimitExceeded)
    );
    assert_eq!(
        run(db.rollback(&key)),
        Err(CommandError::RollbackConflict),
        "oversized update keeps prior policy"
    );
    let request = db.request().unwrap();
    run(db.begin(
        request,
        base::definition(),
        base::args("admitted"),
        Some(Json::Null),
        100,
    ))
    .unwrap();
    assert_eq!(
        db.set_conflict_policy(ConflictPolicy::default()),
        Err(CommandError::Busy)
    );
    db.abort(request).unwrap();
    let (mut db, after) = capture(db);
    assert_eq!(
        before, after,
        "failed operations do not alter persisted data"
    );
    db.set_conflict_policy(ConflictPolicy {
        default: ConflictReaction::Stop,
        commands: BTreeMap::from([("create-parent-v1".into(), ConflictReaction::Discard)]),
    })
    .unwrap();
    run(db.rollback(&key)).unwrap();
    assert_eq!(
        db.queue_status().commands[0].state,
        SyncState::Discarded,
        "unit rollback uses same planner"
    );
    let (ordinary, _) = enqueue(&mut db, "ordinary rejection");
    rejected(&mut db, &ordinary);
    assert_eq!(
        run(db.rollback_resolving(&ordinary)),
        Ok(Resolution::Completed),
        "unchanged rows inverse normally"
    );
    db.close();
}

/// CP05/10: guard errors precede policy; valid mapping and skipped outside writes commit together.
pub fn correction_guards<S: SyncStorage>(storage: S) {
    let mut db = open(storage);
    let (key, row) = enqueue(&mut db, "local");
    let outside =
        run(db.insert_proposed("parents", fields("outside"), Some("outside".into()))).unwrap();
    db.prepare_push(100, 0).unwrap();
    db.receive(&key, Some(RemoteOutcome::Corrected(Json::Null)))
        .unwrap();
    run(db.replace("parents", &row.id, fields("direct"))).unwrap();
    discard_policy(&mut db);
    assert_eq!(
        run(db.reconcile_resolving(&key, correction(&row.id, "local:bad"), vec![])),
        Err(CommandError::InvalidCommand)
    );
    let mut plan = correction(&row.id, "remote");
    plan.writes.push(ReconcileWrite::Remove {
        table: "parents".into(),
        id: outside.id.clone(),
    });
    let stale = CorrectionGuard {
        table: "parents".into(),
        id: outside.id.clone(),
        expected: Some(fields("stale")),
    };
    let before = rows(&db);
    assert_eq!(
        run(db.reconcile_resolving(&key, plan.clone(), vec![stale])),
        Err(CommandError::RollbackConflict)
    );
    assert_eq!(rows(&db), before);
    assert_eq!(
        run(db.reconcile_resolving(
            &key,
            plan,
            vec![CorrectionGuard {
                table: "parents".into(),
                id: outside.id.clone(),
                expected: Some(outside.fields.clone())
            }]
        )),
        Ok(Resolution::KeptCurrent {
            discarded_keys: vec![],
            unmapped: vec![],
            skipped_writes: 2
        })
    );
    assert_eq!(
        rows(&db).iter().find(|r| r.id == "remote").unwrap().fields,
        fields("direct")
    );
    assert!(rows(&db).contains(&outside));
    let (db, ledger) = capture(db);
    assert_eq!(ledger.records[&key].state, SyncState::Corrected);
    assert!(ledger.records[&key].reconciled);
    assert!(db.pending_reconciliations().is_empty());
    db.close();
}

/// CP06/15/a/b: transition preserves history; dependency bookkeeping never pins retired producers.
pub fn history<S: SyncStorage>(storage: S, history: usize, sent: bool, reopen: impl Fn(S) -> S) {
    let options = SyncOptions {
        history,
        ..SyncOptions::default()
    };
    let mut db = open_options(storage, options.clone());
    let (producer, parent) = enqueue(&mut db, "parent");
    if sent {
        rejected(&mut db, &producer);
    }
    let (dependent, _) = child(&mut db, &parent.id).unwrap();
    if sent {
        run(db.replace("parents", &parent.id, fields("direct"))).unwrap();
    }
    let (mut db, before) = capture(db);
    let kept_rows = rows(&db);
    let status = db.observe_queue_status().unwrap();
    db.observe_outcome(&producer).unwrap();
    db.drain_events(100);
    discard_policy(&mut db);
    let report = if sent {
        run(db.rollback_resolving(&producer))
    } else {
        run(db.discard(&producer))
    }
    .unwrap();
    let mut keys = vec![producer.clone(), dependent.clone()];
    keys.sort();
    let unmapped = before
        .records
        .values()
        .flat_map(|r| r.created.keys().cloned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    assert_eq!(
        report,
        Resolution::KeptCurrent {
            discarded_keys: keys,
            unmapped,
            skipped_writes: 0
        }
    );
    assert_eq!(rows(&db), kept_rows);
    let events = db.drain_events(100);
    assert!(
        events.iter().any(
            |e| matches!(e,CommandEvent::Status { subscription, .. } if *subscription == status)
        )
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, CommandEvent::Outcome { .. })),
        "no fabricated outcome"
    );
    let storage = reopen(db.into_storage());
    let after = SyncLedger::from_rows(storage.load_sync().unwrap().as_deref().unwrap()).unwrap();
    assert_eq!(
        after.records[&dependent],
        retired(before.records[&dependent].clone())
    );
    // CP15c: legal-state checks still reject dangling dispatch paths and either missed clear.
    let mut malformed = after.clone();
    malformed
        .records
        .get_mut(&dependent)
        .unwrap()
        .reference_paths = before.records[&dependent].reference_paths.clone();
    assert!(malformed.validate().is_err());
    let mut malformed = after.clone();
    malformed.records.get_mut(&dependent).unwrap().dependencies =
        before.records[&dependent].dependencies.clone();
    assert!(malformed.validate().is_err());
    let mut dangling = before.clone();
    dangling.records.remove(&producer);
    assert!(dangling.validate().is_err());
    assert_eq!(after.records.contains_key(&producer), history > 1);
    let mut db = open_options(storage, options);
    assert_eq!(rows(&db), kept_rows);
    assert_eq!(db.queue_status().live, 0);
    assert!(db.pending_reconciliations().is_empty());
    assert_eq!(
        child(&mut db, &parent.id),
        Err(CommandError::UnknownLocalReference)
    );
    if history > 1 {
        assert_eq!(
            after.records[&producer],
            retired(before.records[&producer].clone())
        );
        for op in 0..4 {
            let result = match op {
                0 => run(db.rollback(&producer)),
                1 => run(db.cancel(&producer)),
                2 => run(db.reconcile(&producer, ReconcilePlan::default())),
                _ => db.request_redelivery(&producer, 100),
            };
            assert_eq!(result, Err(CommandError::Protocol));
        }
        assert_eq!(
            db.receive(&producer, None),
            if sent {
                Ok(())
            } else {
                Err(CommandError::Protocol)
            }
        );
        assert_eq!(
            db.receive(
                &producer,
                Some(RemoteOutcome::Rejected("business rule".into()))
            ),
            if sent {
                Ok(())
            } else {
                Err(CommandError::Protocol)
            }
        );
        assert_eq!(
            db.receive(&producer, Some(RemoteOutcome::Rejected("different".into()))),
            Err(CommandError::Protocol)
        );
        let (mut reopened, unchanged) = capture(db);
        assert_eq!(
            after, unchanged,
            "receipts/errors are byte-identical no-ops"
        );
        reopened.forget(&producer).unwrap();
        db = reopened;
    }
    assert_eq!(
        child(&mut db, &parent.id),
        Err(CommandError::UnknownLocalReference)
    );
    let (mut db, remaining) = capture(db);
    assert_eq!(remaining.records[&dependent], after.records[&dependent]);
    db.forget(&dependent).unwrap();
    let (_, fresh) = enqueue(&mut db, "new parent");
    assert_ne!(fresh.id, parent.id);
    db.close();
}

/// CP15d/e: detached history keeps data while the still-live producer later maps or rolls back.
pub fn dependent_only<S: SyncStorage>(storage: S, mapping: bool, reopen: impl Fn(S) -> S) {
    let mut db = open(storage);
    let (producer, parent) = enqueue(&mut db, "parent");
    let (dependent, child_row) = child(&mut db, &parent.id).unwrap();
    let kept = rows(&db);
    run(db.discard(&dependent)).unwrap();
    assert_eq!(rows(&db), kept);
    let (mut db, before) = capture(db);
    if mapping {
        db.prepare_push(100, 0).unwrap();
        db.receive(&producer, Some(RemoteOutcome::Applied(Json::Null)))
            .unwrap();
        run(db.reconcile(
            &producer,
            ReconcilePlan {
                writes: vec![],
                mappings: BTreeMap::from([(parent.id.clone(), "remote".into())]),
            },
        ))
        .unwrap();
    } else {
        rejected(&mut db, &producer);
        assert_eq!(
            run(db.rollback_resolving(&producer)),
            Ok(Resolution::Completed)
        );
    }
    let storage = reopen(db.into_storage());
    let after = SyncLedger::from_rows(storage.load_sync().unwrap().as_deref().unwrap()).unwrap();
    let db = open(storage);
    let actual_child = run(db.query("children").fetch(&db)).unwrap().remove(0);
    if mapping {
        let mut expected = before.records[&dependent].clone();
        for change in &mut expected.undo {
            for row in change.before.iter_mut().chain(change.after.iter_mut()) {
                if row.fields.get("parent") == Some(&Value::Reference(parent.id.clone())) {
                    row.fields
                        .insert("parent".into(), Value::Reference("remote".into()));
                }
            }
        }
        assert_eq!(after.records[&dependent], expected);
        assert_eq!(
            actual_child.fields["parent"],
            Value::Reference("remote".into())
        );
        assert_eq!(run(db.query("parents").fetch(&db)).unwrap()[0].id, "remote");
    } else {
        assert_eq!(after.records[&dependent], before.records[&dependent]);
        assert_eq!(actual_child, child_row);
        assert!(run(db.query("parents").fetch(&db)).unwrap().is_empty());
    }
    db.close();
}

/// CP08/08b/16/06b: later recorded writes refuse; cancel and Cancelled-root rollback ignore policy.
pub fn later_work_cancel<S: SyncStorage>(storage: S) {
    let mut db = open(storage);
    let (key, row) = enqueue(&mut db, "parent");
    let later = edit(&mut db, &row.id, "later");
    discard_policy(&mut db);
    assert_eq!(run(db.discard(&key)), Err(CommandError::RollbackConflict));
    db.prepare_push(100, 0).unwrap();
    db.receive(&key, Some(RemoteOutcome::Corrected(Json::Null)))
        .unwrap();
    assert_eq!(
        run(db.reconcile_resolving(&key, correction(&row.id, "remote"), vec![])),
        Err(CommandError::RollbackConflict)
    );
    run(db.cancel(&later)).unwrap();
    assert_eq!(
        run(db.reconcile_resolving(&key, correction(&row.id, "remote"), vec![])),
        Ok(Resolution::Completed)
    );
    let (rejection, rejected_row) = enqueue(&mut db, "rejection");
    rejected(&mut db, &rejection);
    let later = edit(&mut db, &rejected_row.id, "later recorded work");
    let kept = rows(&db);
    assert_eq!(
        run(db.rollback_resolving(&rejection)),
        Err(CommandError::RollbackConflict)
    );
    assert_eq!(rows(&db), kept);
    run(db.cancel(&later)).unwrap();
    assert_eq!(
        run(db.rollback_resolving(&rejection)),
        Ok(Resolution::Completed)
    );
    let (cancelled, row) = enqueue(&mut db, "cancelled");
    run(db.replace("parents", &row.id, fields("direct"))).unwrap();
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
    assert_eq!(
        run(db.rollback(&cancelled)),
        Err(CommandError::RollbackConflict)
    );
    assert!(matches!(
        run(db.discard(&cancelled)),
        Ok(Resolution::KeptCurrent { .. })
    ));
    db.close();
}

/// CP07: every in-flight or accepted manual root is refused, including cancelled sent recovery.
pub fn in_flight<S: SyncStorage>(storage: S) {
    fn refused<S: SyncStorage>(db: CommandDatabase<S>, key: &str) -> CommandDatabase<S> {
        let (mut db, before) = capture(db);
        let kept = rows(&db);
        db.observe_queue_status().unwrap();
        db.drain_events(100);
        assert_eq!(run(db.discard(key)), Err(CommandError::Protocol));
        assert!(db.drain_events(100).is_empty());
        assert_eq!(rows(&db), kept);
        let (db, after) = capture(db);
        assert_eq!(before, after);
        db
    }
    let mut db = open(storage);
    let (key, row) = enqueue(&mut db, "sent");
    db.prepare_push(100, 0).unwrap();
    db = refused(db, &key);
    db.receive(&key, None).unwrap();
    db = refused(db, &key);
    db.receive(&key, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    db = refused(db, &key);
    run(db.reconcile(
        &key,
        ReconcilePlan {
            writes: vec![],
            mappings: BTreeMap::from([(row.id, "accepted".into())]),
        },
    ))
    .unwrap();
    let (key, row) = enqueue(&mut db, "cancelled sent");
    db.prepare_push(100, 0).unwrap();
    run(db.replace("parents", &row.id, fields("direct"))).unwrap();
    assert_eq!(run(db.cancel(&key)), Err(CommandError::RollbackConflict));
    db = refused(db, &key);
    let (key, _) = enqueue(&mut db, "corrected");
    db.prepare_push(100, 0).unwrap();
    db.receive(&key, Some(RemoteOutcome::Corrected(Json::Null)))
        .unwrap();
    db = refused(db, &key);
    db.close();
}

/// CP09: entity-created work and a command's undo on a configured entity table both refuse.
pub fn entity_refusal<S: SyncStorage>(storage: S) {
    let mut db = open(storage);
    db.configure_entities(
        EntityConfig {
            scope: "policy-test".into(),
            tables: std::collections::BTreeSet::from(["parents".into()]),
        },
        || {
            Ok(EntityContext {
                identity: Json::Null,
                now_ms: 100,
            })
        },
    )
    .unwrap();
    let row = run(db.insert("parents", fields("entity"))).unwrap();
    let key = db.queue_status().commands[0].key.clone();
    let kept = rows(&db);
    assert_eq!(run(db.discard(&key)), Err(CommandError::RollbackConflict));
    assert_eq!(rows(&db), kept);
    // A command on the entity table is also refused even with no entity descriptor.
    let command = edit(&mut db, &row.id, "command");
    let kept = rows(&db);
    assert_eq!(
        run(db.discard(&command)),
        Err(CommandError::RollbackConflict)
    );
    assert_eq!(rows(&db), kept);
    discard_policy(&mut db);
    db.prepare_push(100, 0).unwrap();
    db.receive_entity(
        "parents",
        &row.id,
        &key,
        Some(RemoteOutcome::Rejected("rule".into())),
    )
    .unwrap();
    run(db.reconcile(&key, ReconcilePlan::default())).unwrap();
    assert_eq!(
        run(db.rollback_resolving(&key)),
        Err(CommandError::RollbackConflict)
    );
    assert_eq!(rows(&db), kept);
    db.close();
}

/// CP11: individually valid records can produce an oversized complete report.
/// Correction-discard reports have empty lists and one usize count, so that path's report
/// cannot approach the byte/node bound; the shared preflight still applies to it.
pub fn report_limit<S: SyncStorage>(storage: S) {
    let mut db = open(storage);
    let (key, row) = enqueue(&mut db, "root");
    rejected(&mut db, &key);
    let (dependent, _) = child(&mut db, &row.id).unwrap();
    run(db.replace("parents", &row.id, fields("direct"))).unwrap();
    // Seed the large, fully validated fixture in one atomic plan. Repeated enqueue setup
    // would repeatedly encode all prior 90KB identities and obscure the report boundary.
    let mut storage = db.into_storage();
    let expected_sync = storage.load_sync().unwrap();
    let mut ledger = SyncLedger::from_rows(expected_sync.as_deref().unwrap()).unwrap();
    let prototype = ledger.records[&dependent].clone();
    let model = run(storage.catalog()).unwrap().state.unwrap();
    let mut guards = Vec::new();
    let mut mutations = Vec::new();
    for n in 0..12 {
        let id = format!("local:{}-{n}", "x".repeat(90_000));
        let mut record = prototype.clone();
        record.order = ledger.header.sequence;
        ledger.header.sequence += 1;
        record.key = format!(
            "{:016x}-{}-{:016x}",
            ledger.header.last_ms, ledger.header.namespace, record.order
        );
        record.created = BTreeMap::from([(id.clone(), "children".into())]);
        record.undo[0].id = id.clone();
        record.undo[0].after.as_mut().unwrap().id = id.clone();
        let mut inserted = record.undo[0].after.clone().unwrap();
        inserted
            .fields
            .insert(ROW_VERSION.into(), Value::Integer(1));
        guards.push(RowGuard {
            table: "children".into(),
            id,
            expected: None,
        });
        mutations.push(Mutation::Insert {
            table: "children".into(),
            row: inserted,
        });
        ledger.records.insert(record.key.clone(), record);
    }
    storage
        .apply_sync_plan(SyncPlan {
            transition: SyncTransition::Normal,
            expected_model: model.clone(),
            next_model: model,
            expected_sync,
            next_sync: ledger.to_rows().unwrap(),
            guards,
            mutations,
        })
        .unwrap();
    let (mut db, before) = capture(open(storage));
    let kept = rows(&db);
    db.observe_queue_status().unwrap();
    db.observe_outcome(&key).unwrap();
    db.drain_events(100);
    discard_policy(&mut db);
    assert_eq!(run(db.discard(&key)), Err(CommandError::LimitExceeded));
    assert_eq!(
        run(db.rollback_resolving(&key)),
        Err(CommandError::LimitExceeded)
    );
    assert_eq!(run(db.rollback(&key)), Err(CommandError::LimitExceeded));
    assert_eq!(rows(&db), kept);
    assert!(db.drain_events(100).is_empty());
    let (db, after) = capture(db);
    assert_eq!(before, after);
    db.close();
}

/// CP15c: already-mapped arguments remain remote; producer history is freely removable.
pub fn mapped_history<S: SyncStorage>(storage: S, reopen: impl Fn(S) -> S) {
    let mut db = open(storage);
    let (producer, parent) = enqueue(&mut db, "mapped");
    db.prepare_push(100, 0).unwrap();
    db.receive(&producer, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    run(db.reconcile(
        &producer,
        ReconcilePlan {
            writes: vec![],
            mappings: BTreeMap::from([(parent.id.clone(), "remote".into())]),
        },
    ))
    .unwrap();
    let (dependent, _) = child(&mut db, "remote").unwrap();
    let (mut db, before) = capture(db);
    run(db.discard(&dependent)).unwrap();
    db.forget(&producer).unwrap();
    let storage = reopen(db.into_storage());
    let after = SyncLedger::from_rows(storage.load_sync().unwrap().as_deref().unwrap()).unwrap();
    assert_eq!(
        after.records[&dependent],
        retired(before.records[&dependent].clone())
    );
    open(storage).close();
}

/// CP06/06b: a transitive closure includes never-sent Cancelled recovery without restoring rows.
pub fn transitive<S: SyncStorage>(storage: S) {
    let mut db = open(storage);
    let (root, parent) = enqueue(&mut db, "root");
    let (_, dependent) = child(&mut db, &parent.id).unwrap();
    let (cancelled, leaf) = child(&mut db, &dependent.id).unwrap();
    let mut edited = leaf.fields.clone();
    edited.insert("name".into(), Value::String("direct edit".into()));
    run(db.replace("children", &leaf.id, edited)).unwrap();
    assert_eq!(
        run(db.cancel(&cancelled)),
        Err(CommandError::RollbackConflict)
    );
    let (mut db, before) = capture(db);
    assert_eq!(before.records[&cancelled].state, SyncState::Cancelled);
    let kept = rows(&db);
    assert_eq!(
        run(db.discard(&root)),
        Ok(Resolution::KeptCurrent {
            discarded_keys: before.records.keys().cloned().collect(),
            unmapped: before
                .records
                .values()
                .flat_map(|r| r.created.keys().cloned())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect(),
            skipped_writes: 0,
        })
    );
    assert_eq!(rows(&db), kept);
    let (db, after) = capture(db);
    for (key, original) in before.records {
        assert_eq!(after.records[&key], retired(original));
    }
    db.close();
}
