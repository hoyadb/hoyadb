//! SPEC-0039 / TC-0043 cases on the additive `reconcile_guarded` / `CorrectionGuard` API.
//! They reference the new symbols, so they are post-GREEN semantics checks, not RED; the
//! compile-free RED is GW01/GW02 (command_guard.rs) and the facade cases.
#[allow(dead_code)]
#[path = "command_guard.rs"]
mod guard;
pub use guard::*;
use hoyadb_core::{command::*, storage::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
};

pub fn expect(id: &str, value: Option<&str>) -> CorrectionGuard {
    CorrectionGuard {
        table: "notes".into(),
        id: id.into(),
        expected: value.map(body),
    }
}

pub fn put(id: &str, value: &str) -> ReconcileWrite {
    ReconcileWrite::Put {
        table: "notes".into(),
        id: id.into(),
        fields: body(value),
    }
}

pub fn remove(id: &str) -> ReconcileWrite {
    ReconcileWrite::Remove {
        table: "notes".into(),
        id: id.into(),
    }
}

pub fn writes(writes: Vec<ReconcileWrite>) -> ReconcilePlan {
    ReconcilePlan {
        writes,
        mappings: BTreeMap::new(),
    }
}

/// A command creating a notes row; returns its key and local id.
fn create<S: SyncStorage>(db: &mut CommandDatabase<S>, value: &str, now: i64) -> (String, String) {
    let request = db.request().unwrap();
    let definition = CommandDefinition {
        name: "create-note-v1".into(),
        arguments: BTreeMap::new(),
        reference_paths: vec![],
    };
    let args = Json::Object(BTreeMap::new());
    assert!(run(db.begin(request, definition, args, Some(Json::Null), now)).unwrap());
    let row = run(db.scope_insert(request, "notes", body(value))).unwrap();
    (run(db.finish(request, Json::Null)).unwrap().key, row.id)
}

fn note<S: SyncStorage>(db: &CommandDatabase<S>, id: &str) -> Option<Row> {
    rows(db).into_iter().find(|r| r.id == id)
}

/// A command editing server-note, sent and Corrected; other-note = "other" is unrelated.
pub fn corrected<S: SyncStorage>(storage: S) -> (CommandDatabase<S>, String) {
    let mut db = seeded(storage);
    let a = edit(&mut db, "server-note", "a", 100);
    db.prepare_push(101, 0).unwrap();
    db.receive(&a, Some(RemoteOutcome::Corrected(Json::Null)))
        .unwrap();
    (db, a)
}

#[track_caller]
fn conflict<S: SyncStorage>(
    db: CommandDatabase<S>,
    key: &str,
    plan: ReconcilePlan,
    guards: Vec<CorrectionGuard>,
    error: CommandError,
) -> CommandDatabase<S> {
    let (mut db, before) = capture(db);
    assert_eq!(run(db.reconcile_guarded(key, plan, guards)), Err(error));
    assert_unchanged(db, &before)
}

#[track_caller]
fn restart_equal<S: SyncStorage>(db: CommandDatabase<S>) -> CommandDatabase<S> {
    let (db, before) = capture(db);
    assert_unchanged(db, &before)
}

/// GW03: an exact expected image permits a cross-row Put (and, separately, a Remove) together
/// with A's confirmation; omitted optional fields and explicit Null are distinct maps.
pub fn gw03<S: SyncStorage>(fresh: impl Fn() -> S) {
    let (db, a) = corrected(fresh());
    let mut with_null = body("other");
    with_null.insert("extra".into(), Value::Null);
    let null_guard = CorrectionGuard {
        expected: Some(with_null),
        ..expect("other-note", None)
    };
    let mut db = conflict(
        db,
        &a,
        writes(vec![put("other-note", "backend")]),
        vec![null_guard],
        CommandError::RollbackConflict,
    );
    assert_eq!(
        run(db.reconcile_guarded(
            &a,
            writes(vec![put("other-note", "backend")]),
            vec![expect("other-note", Some("other"))],
        )),
        Ok(())
    );
    assert_eq!(note(&db, "other-note").unwrap().fields, body("backend"));
    assert!(db.pending_reconciliations().is_empty());
    let db = restart_equal(db);
    db.close();

    let (mut db, a) = corrected(fresh());
    assert_eq!(
        run(db.reconcile_guarded(
            &a,
            writes(vec![remove("other-note")]),
            vec![expect("other-note", Some("other"))],
        )),
        Ok(())
    );
    assert_eq!(note(&db, "other-note"), None);
    assert!(db.pending_reconciliations().is_empty());
    db.close();

    // The reverse distinction: stored explicit Null vs an expectation that omits the field.
    let mut db = seeded(fresh());
    let mut stored = body("n");
    stored.insert("extra".into(), Value::Null);
    run(db.insert_proposed("notes", stored.clone(), Some("nullable".into()))).unwrap();
    let a = edit(&mut db, "server-note", "a", 100);
    db.prepare_push(101, 0).unwrap();
    db.receive(&a, Some(RemoteOutcome::Corrected(Json::Null)))
        .unwrap();
    let db = conflict(
        db,
        &a,
        writes(vec![put("nullable", "backend")]),
        vec![expect("nullable", Some("n"))],
        CommandError::RollbackConflict,
    );
    let mut db = db;
    let exact = CorrectionGuard {
        expected: Some(stored),
        ..expect("nullable", None)
    };
    assert_eq!(
        run(db.reconcile_guarded(&a, writes(vec![put("nullable", "backend")]), vec![exact])),
        Ok(())
    );
    db.close();
}

/// GW04: a write after the expectation was captured invalidates it; it is never refreshed.
pub fn gw04<S: SyncStorage>(storage: S) {
    let (mut db, a) = corrected(storage);
    let captured = expect("other-note", Some("other"));
    run(db.replace("notes", "other-note", body("intervening"))).unwrap();
    let plan = writes(vec![put("other-note", "backend")]);
    let db = conflict(
        db,
        &a,
        plan.clone(),
        vec![captured.clone()],
        CommandError::RollbackConflict,
    );
    let db = conflict(db, &a, plan, vec![captured], CommandError::RollbackConflict);
    assert_eq!(note(&db, "other-note").unwrap().fields, body("intervening"));
    db.close();
}

/// GW05: absent rows, expected absence, ordered writes and no resurrection.
pub fn gw05<S: SyncStorage>(fresh: impl Fn() -> S) {
    // Insert of an absent unrelated row needs expected absence.
    let (db, a) = corrected(fresh());
    let insert = writes(vec![put("server-created", "backend")]);
    let mut db = conflict(
        db,
        &a,
        insert.clone(),
        vec![],
        CommandError::RollbackConflict,
    );
    assert_eq!(
        run(db.reconcile_guarded(&a, insert, vec![expect("server-created", None)])),
        Ok(())
    );
    assert_eq!(note(&db, "server-created").unwrap().fields, body("backend"));
    db.close();
    // Expected absence conflicts while the row exists.
    let (db, a) = corrected(fresh());
    let db = conflict(
        db,
        &a,
        writes(vec![put("other-note", "backend")]),
        vec![expect("other-note", None)],
        CommandError::RollbackConflict,
    );
    db.close();
    // Ordered writes to one identity share one original before-image.
    let (mut db, a) = corrected(fresh());
    let ordered = writes(vec![put("other-note", "first"), remove("other-note")]);
    let twice = vec![
        expect("other-note", Some("other")),
        expect("other-note", Some("first")),
    ];
    let mut db2 = conflict(db, &a, ordered.clone(), twice, CommandError::InvalidCommand);
    assert_eq!(
        run(db2.reconcile_guarded(&a, ordered, vec![expect("other-note", Some("other"))])),
        Ok(())
    );
    assert_eq!(note(&db2, "other-note"), None);
    db = db2;
    db.close();
    // A later unrecorded remove cannot be resurrected by an unguarded or stale insert.
    let (mut db, a) = corrected(fresh());
    assert!(run(db.remove("notes", "other-note")).unwrap());
    let resurrect = writes(vec![put("other-note", "backend")]);
    let db = conflict(
        db,
        &a,
        resurrect.clone(),
        vec![],
        CommandError::RollbackConflict,
    );
    let db = conflict(
        db,
        &a,
        resurrect,
        vec![expect("other-note", Some("other"))],
        CommandError::RollbackConflict,
    );
    db.close();
    // A no-op absent Remove uses its absence guard; unguarded it is refused.
    let (db, a) = corrected(fresh());
    let noop = writes(vec![remove("never-existed")]);
    let mut db = conflict(db, &a, noop.clone(), vec![], CommandError::RollbackConflict);
    assert_eq!(
        run(db.reconcile_guarded(&a, noop, vec![expect("never-existed", None)])),
        Ok(())
    );
    assert_eq!(note(&db, "never-existed"), None);
    assert!(db.pending_reconciliations().is_empty());
    db.close();
    // An equal-value Put retains its expectation and publishes no data change.
    let (mut db, a) = corrected(fresh());
    let query = db.query("notes");
    let data = run(db.observe(&query)).unwrap();
    db.drain_events(0);
    assert_eq!(
        run(db.reconcile_guarded(
            &a,
            writes(vec![put("other-note", "other")]),
            vec![expect("other-note", Some("other"))],
        )),
        Ok(())
    );
    let events = db.drain_events(0);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, CommandEvent::Data(n) if n.subscription == data)),
        "no data notification: no application row changed"
    );
    db.close();
}

/// GW06: malformed guards are InvalidCommand, before any data conflict, changing nothing.
pub fn gw06<S: SyncStorage>(storage: S) {
    let (mut db, a) = corrected(storage);
    let plan = writes(vec![put("other-note", "backend")]);
    let ok = expect("other-note", Some("other"));
    let mut reserved = body("other");
    reserved.insert("__hoya_schema_version".into(), Value::Integer(1));
    let mut with_id = body("other");
    with_id.insert("id".into(), Value::String("other-note".into()));
    let mut wrong_type = BTreeMap::new();
    wrong_type.insert("body".into(), Value::Integer(1));
    let mut unknown_field = body("other");
    unknown_field.insert("nope".into(), Value::String("x".into()));
    let cases: Vec<(&str, Vec<CorrectionGuard>)> = vec![
        ("duplicate", vec![ok.clone(), ok.clone()]),
        (
            "unused",
            vec![ok.clone(), expect("server-note-unwritten", None)],
        ),
        (
            "unknown table",
            vec![CorrectionGuard {
                table: "nope".into(),
                ..ok.clone()
            }],
        ),
        (
            "reserved table",
            vec![CorrectionGuard {
                table: "__hoya_sync".into(),
                expected: None,
                ..ok.clone()
            }],
        ),
        (
            "empty id",
            vec![CorrectionGuard {
                id: String::new(),
                ..ok.clone()
            }],
        ),
        (
            "reserved field",
            vec![CorrectionGuard {
                expected: Some(reserved),
                ..ok.clone()
            }],
        ),
        (
            "id field",
            vec![CorrectionGuard {
                expected: Some(with_id),
                ..ok.clone()
            }],
        ),
        (
            "wrong type",
            vec![CorrectionGuard {
                expected: Some(wrong_type),
                ..ok.clone()
            }],
        ),
        (
            "unknown field",
            vec![CorrectionGuard {
                expected: Some(unknown_field),
                ..ok.clone()
            }],
        ),
    ];
    for (name, guards) in cases {
        let (next, before) = capture(db);
        db = next;
        assert_eq!(
            run(db.reconcile_guarded(&a, plan.clone(), guards)),
            Err(CommandError::InvalidCommand),
            "{name}"
        );
        db = assert_unchanged(db, &before);
    }
    // Invalid guard structure takes precedence over a separate missing or stale expectation.
    let both = writes(vec![
        put("other-note", "backend"),
        put("server-created", "x"),
    ]);
    let stale = expect("other-note", Some("stale"));
    let db = conflict(
        db,
        &a,
        both,
        vec![stale.clone(), stale],
        CommandError::InvalidCommand,
    );
    db.close();
}

/// GW07: a mapped created row stays inside A's write set; guards for it are unused; a guard
/// for an unrelated row compares its original pre-rename reference.
pub fn gw07<S: SyncStorage>(fresh: impl Fn() -> S) {
    for target in ["local", "mapped"] {
        let mut db = open(fresh());
        let (a, local) = create(&mut db, "created", 100);
        db.prepare_push(101, 0).unwrap();
        db.receive(&a, Some(RemoteOutcome::Corrected(Json::Null)))
            .unwrap();
        let id = if target == "local" {
            local.clone()
        } else {
            "srv".into()
        };
        let plan = ReconcilePlan {
            mappings: BTreeMap::from([(local.clone(), "srv".into())]),
            writes: vec![put("srv", "corrected")],
        };
        let db = conflict(
            db,
            &a,
            plan.clone(),
            vec![expect(&id, Some("created"))],
            CommandError::InvalidCommand,
        );
        let mut db = db;
        assert_eq!(
            run(db.reconcile_guarded(&a, plan, vec![])),
            Ok(()),
            "{target}"
        );
        assert_eq!(note(&db, "srv").unwrap().fields, body("corrected"));
        db.close();
    }
    // An unrelated link referencing A's created row: the guard holds the pre-rename value.
    let mut db = open(fresh());
    let (a, local) = create(&mut db, "created", 100);
    let link = run(db.insert(
        "links",
        BTreeMap::from([("note".into(), Value::Reference(local.clone()))]),
    ))
    .unwrap();
    db.prepare_push(101, 0).unwrap();
    db.receive(&a, Some(RemoteOutcome::Corrected(Json::Null)))
        .unwrap();
    let relink = |value: &str| ReconcilePlan {
        mappings: BTreeMap::from([(local.clone(), "srv".into())]),
        writes: vec![ReconcileWrite::Put {
            table: "links".into(),
            id: link.id.clone(),
            fields: BTreeMap::from([("note".into(), Value::Reference(value.into()))]),
        }],
    };
    let link_guard = |value: &str| CorrectionGuard {
        table: "links".into(),
        id: link.id.clone(),
        expected: Some(BTreeMap::from([(
            "note".into(),
            Value::Reference(value.into()),
        )])),
    };
    let db = conflict(
        db,
        &a,
        relink("srv"),
        vec![link_guard("srv")],
        CommandError::RollbackConflict,
    );
    let mut db = db;
    assert_eq!(
        run(db.reconcile_guarded(&a, relink("srv"), vec![link_guard(&local)])),
        Ok(())
    );
    let links = run(db.query("links").fetch(&db)).unwrap();
    assert_eq!(links[0].fields["note"], Value::Reference("srv".into()));
    db.close();
}

/// GW10: a matching expectation cannot overwrite another live command's optimistic row.
pub fn gw10<S: SyncStorage>(storage: S) {
    let mut db = seeded(storage);
    let a = edit(&mut db, "server-note", "a", 100);
    let e = edit(&mut db, "other-note", "e", 101);
    db.prepare_push(102, 0).unwrap();
    db.receive(&a, Some(RemoteOutcome::Corrected(Json::Null)))
        .unwrap();
    for (plan, guard) in [
        (
            writes(vec![put("other-note", "backend")]),
            expect("other-note", Some("e")),
        ),
        (
            writes(vec![remove("other-note")]),
            expect("other-note", Some("e")),
        ),
    ] {
        db = conflict(db, &a, plan, vec![guard], CommandError::RollbackConflict);
    }
    run(db.reconcile(&a, ReconcilePlan::default())).unwrap();
    db.receive(&e, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    run(db.reconcile(&e, ReconcilePlan::default())).unwrap();
    assert_eq!(note(&db, "other-note").unwrap().fields, body("e"));
    db.close();
}

fn entity_owner<S: SyncStorage>(storage: S) -> (CommandDatabase<S>, Arc<AtomicI64>) {
    let mut db = open(storage);
    let clock = Arc::new(AtomicI64::new(100));
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
    (db, clock)
}

/// An entity row whose create has been confirmed, so it is in no live record's undo.
fn entity_row<S: SyncStorage>(db: &mut CommandDatabase<S>, value: &str, now: i64) -> String {
    let row = run(db.insert("notes", body(value))).unwrap();
    let create = keys(&db.prepare_push(now, 0).unwrap());
    db.receive_entity(
        "notes",
        &row.id,
        &create[0],
        Some(RemoteOutcome::Applied(Json::Null)),
    )
    .unwrap();
    run(db.reconcile(
        &create[0],
        ReconcilePlan {
            mappings: BTreeMap::from([(row.id.clone(), row.id.clone())]),
            writes: vec![],
        },
    ))
    .unwrap();
    row.id
}

/// GW11: an ordinary Put/Remove on an entity-managed table outside the closure is refused,
/// even with an exact guard, preserving the row and comparison metadata.
pub fn gw11<S: SyncStorage>(storage: S) {
    let (mut db, _) = entity_owner(storage);
    let r = entity_row(&mut db, "r", 100);
    let s = entity_row(&mut db, "s", 110);
    let a = edit(&mut db, &r, "a", 120);
    db.prepare_push(121, 0).unwrap();
    db.receive(&a, Some(RemoteOutcome::Corrected(Json::Null)))
        .unwrap();
    for write in [put(&s, "backend"), remove(&s)] {
        db = conflict(
            db,
            &a,
            writes(vec![write]),
            vec![expect(&s, Some("s"))],
            CommandError::RollbackConflict,
        );
    }
    db.close();
}

/// GW12: a typed Entity correction outside the closure is refused, also with a row guard.
pub fn gw12<S: SyncStorage>(storage: S) {
    let (mut db, clock) = entity_owner(storage);
    let r = entity_row(&mut db, "r", 100);
    let s = entity_row(&mut db, "s", 110);
    let a = edit(&mut db, &r, "a", 120);
    clock.store(130, Ordering::Relaxed);
    run(db.patch("notes", &s, body("e"))).unwrap();
    db.prepare_push(131, 0).unwrap();
    db.receive(&a, Some(RemoteOutcome::Corrected(Json::Null)))
        .unwrap();
    let typed = writes(vec![ReconcileWrite::Entity {
        change: EntityChange {
            table: "notes".into(),
            record_id: s.clone(),
            op: EntityOperation::Update,
            changes: body("backend"),
            removed_fields: BTreeSet::new(),
            timestamp_ms: 300,
            field_timestamps: BTreeMap::from([("body".into(), 300)]),
        },
    }]);
    let db = conflict(
        db,
        &a,
        typed.clone(),
        vec![],
        CommandError::RollbackConflict,
    );
    let db = conflict(
        db,
        &a,
        typed,
        vec![expect(&s, Some("e"))],
        CommandError::RollbackConflict,
    );
    assert_eq!(note(&db, &s).unwrap().fields, body("e"));
    db.close();
}

/// GW12 (Create/Delete): typed Entity Create and Delete corrections outside the closure are
/// refused, with and without a row guard, preserving rows, comparison metadata, recovery
/// records and notifications exactly.
pub fn gw12_create_delete<S: SyncStorage>(storage: S) {
    let (mut db, _) = entity_owner(storage);
    let r = entity_row(&mut db, "r", 100);
    let s = entity_row(&mut db, "s", 110);
    let a = edit(&mut db, &r, "a", 120);
    db.prepare_push(121, 0).unwrap();
    db.receive(&a, Some(RemoteOutcome::Corrected(Json::Null)))
        .unwrap();
    let change = |op, id: &str, changes: BTreeMap<String, Value>| ReconcileWrite::Entity {
        change: EntityChange {
            table: "notes".into(),
            record_id: id.into(),
            op,
            field_timestamps: changes.keys().map(|name| (name.clone(), 300)).collect(),
            changes,
            removed_fields: BTreeSet::new(),
            timestamp_ms: 300,
        },
    };
    for (name, write, guard) in [
        (
            "create",
            change(EntityOperation::Create, "server-made", body("x")),
            expect("server-made", None),
        ),
        (
            "delete",
            change(EntityOperation::Delete, &s, BTreeMap::new()),
            expect(&s, Some("s")),
        ),
    ] {
        for guards in [vec![], vec![guard]] {
            let (next, before) = capture(db);
            db = next;
            let query = db.query("notes");
            let data = run(db.observe(&query)).unwrap();
            let outcome = db.observe_outcome(&a).unwrap();
            db.drain_events(0);
            assert_eq!(
                run(db.reconcile_guarded(&a, writes(vec![write.clone()]), guards)),
                Err(CommandError::RollbackConflict),
                "{name}"
            );
            assert!(
                !db.drain_events(0).iter().any(|e| matches!(e, CommandEvent::Data(n) if n.subscription == data)
                    || matches!(e, CommandEvent::Outcome { subscription, .. } if *subscription == outcome)),
                "{name}: no notification"
            );
            db = assert_unchanged(db, &before);
        }
    }
    assert_eq!(note(&db, &s).unwrap().fields, body("s"));
    assert_eq!(note(&db, "server-made"), None);
    db.close();
}

/// GW12b: with an entity ledger, a typed Entity correction may address any declared table, so the
/// typed outside-closure refusal also protects an ordinary (non-entity-configured) table: a typed
/// Update of an existing row with an exact guard and a typed Create of an absent row with an
/// absence guard both return RollbackConflict, changing nothing.
pub fn gw12b<S: SyncStorage>(storage: S) {
    let mut db = open(storage);
    let link = BTreeMap::from([("note".into(), Value::Reference("server-note".into()))]);
    run(db.insert_proposed("notes", body("original"), Some("server-note".into()))).unwrap();
    run(db.insert_proposed("links", link.clone(), Some("link-s".into()))).unwrap();
    let clock = Arc::new(AtomicI64::new(100));
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
    let r = entity_row(&mut db, "r", 100);
    let a = edit(&mut db, &r, "a", 120);
    db.prepare_push(121, 0).unwrap();
    db.receive(&a, Some(RemoteOutcome::Corrected(Json::Null)))
        .unwrap();
    let typed = |op, id: &str| ReconcileWrite::Entity {
        change: EntityChange {
            table: "links".into(),
            record_id: id.into(),
            op,
            changes: BTreeMap::from([("note".into(), Value::Reference(r.clone()))]),
            removed_fields: BTreeSet::new(),
            timestamp_ms: 300,
            field_timestamps: BTreeMap::from([("note".into(), 300)]),
        },
    };
    let guard = |id: &str, expected: Option<BTreeMap<String, Value>>| CorrectionGuard {
        table: "links".into(),
        id: id.into(),
        expected,
    };
    for (name, write, guards) in [
        (
            "update",
            typed(EntityOperation::Update, "link-s"),
            vec![guard("link-s", Some(link.clone()))],
        ),
        (
            "create",
            typed(EntityOperation::Create, "link-new"),
            vec![guard("link-new", None)],
        ),
    ] {
        let (next, before) = capture(db);
        db = next;
        let query = db.query("links");
        let data = run(db.observe(&query)).unwrap();
        let outcome = db.observe_outcome(&a).unwrap();
        db.drain_events(0);
        assert_eq!(
            run(db.reconcile_guarded(&a, writes(vec![write]), guards)),
            Err(CommandError::RollbackConflict),
            "{name}"
        );
        assert!(
            !db.drain_events(0).iter().any(|e| matches!(e, CommandEvent::Data(n) if n.subscription == data)
                || matches!(e, CommandEvent::Outcome { subscription, .. } if *subscription == outcome)),
            "{name}: no notification"
        );
        db = assert_unchanged(db, &before);
    }
    let links = run(db.query("links").fetch(&db)).unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].fields, link);
    db.close();
}
