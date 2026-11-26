//! SPEC-0038 C1a / TC-0042 C1a (DEF-0039): value-aware confirmation promotion. Every case drives
//! existing public operations only, so unchanged code compiles and fails by assertion.
#[allow(dead_code)]
#[path = "command_recovery.rs"]
mod base;
pub use base::{body, keys, open, run};
use hoyadb_core::command::entity::{FieldStamp, RowMetadata};
use hoyadb_core::command::records::SyncLedger;
use hoyadb_core::{command::*, storage::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
};

pub type Clock = Arc<AtomicI64>;

fn configure<S: SyncStorage>(db: &mut CommandDatabase<S>, clock: &Clock) {
    let captured = clock.clone();
    db.configure_entities(
        EntityConfig {
            scope: "promotion-fixture".into(),
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
}

pub fn owner<S: SyncStorage>(storage: S) -> (CommandDatabase<S>, Clock) {
    let mut db = open(storage);
    let clock = Arc::new(AtomicI64::new(10));
    configure(&mut db, &clock);
    (db, clock)
}

pub fn ledger<S: SyncStorage>(
    db: CommandDatabase<S>,
    clock: &Clock,
) -> (CommandDatabase<S>, SyncLedger, Vec<Row>) {
    let storage = db.into_storage();
    let rows = storage.load_sync().unwrap().unwrap();
    let parsed = SyncLedger::from_rows(&rows).unwrap();
    let mut db = open(storage);
    configure(&mut db, clock);
    (db, parsed, rows)
}

/// A row created at clock 10 and confirmed, so its baseline stamps are authoritative and differ
/// from later writes' stamps.
pub fn confirmed_row<S: SyncStorage>(
    db: &mut CommandDatabase<S>,
    clock: &Clock,
    fields: BTreeMap<String, Value>,
) -> String {
    clock.store(10, Ordering::Relaxed);
    let row = run(db.insert("notes", fields)).unwrap();
    let create = keys(&db.prepare_push(10, 0).unwrap());
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

pub fn last<S: SyncStorage>(db: &CommandDatabase<S>) -> String {
    db.queue_status().commands.last().unwrap().key.clone()
}

pub fn current(ledger: &SyncLedger, id: &str) -> RowMetadata {
    ledger.entity.as_ref().unwrap().metadata[&("notes".to_string(), id.to_string())].clone()
}

pub fn images(ledger: &SyncLedger, key: &str) -> (RowMetadata, RowMetadata) {
    let change = &ledger.records[key].metadata_undo[0];
    (
        change.before.clone().unwrap(),
        change.after.clone().unwrap(),
    )
}

pub fn auth(meta: &RowMetadata, field: &str) -> Option<bool> {
    meta.fields.get(field).map(|s| s.authoritative)
}

pub fn stamp(meta: &RowMetadata, field: &str) -> Option<FieldStamp> {
    meta.fields.get(field).cloned()
}

fn confirm<S: SyncStorage>(db: &mut CommandDatabase<S>, id: &str, key: &str) {
    db.receive_entity("notes", id, key, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    run(db.reconcile(key, ReconcilePlan::default())).unwrap();
}

fn note<S: SyncStorage>(db: &CommandDatabase<S>, id: &str) -> Option<Row> {
    run(db.query("notes").fetch(db))
        .unwrap()
        .into_iter()
        .find(|r| r.id == id)
}

/// Two entity writes to `body` on one row at the same client time 20, both sent.
fn equal_clock_pair<S: SyncStorage>(
    storage: S,
) -> (CommandDatabase<S>, Clock, String, String, String) {
    let (mut db, clock) = owner(storage);
    let w = confirmed_row(&mut db, &clock, body("orig"));
    clock.store(20, Ordering::Relaxed);
    run(db.patch("notes", &w, body("A"))).unwrap();
    let a = last(&db);
    run(db.patch("notes", &w, body("E"))).unwrap();
    let e = last(&db);
    assert_eq!(
        keys(&db.prepare_push(21, 0).unwrap()),
        vec![a.clone(), e.clone()],
        "setup: both sent"
    );
    (db, clock, w, a, e)
}

/// C1a-A: confirming A first promotes only E.before (A's value); current and E.after (E's
/// different equal-clock value) stay unauthoritative. Returns the storage for a restart check.
pub fn c1a_a<S: SyncStorage>(storage: S) -> (S, Vec<Row>) {
    let (db, clock, w, a, e) = equal_clock_pair(storage);
    let (mut db, before, _) = ledger(db, &clock);
    confirm(&mut db, &w, &a);
    let (db, after, rows) = ledger(db, &clock);
    let (e_before, e_after) = images(&after, &e);
    assert_eq!(
        auth(&e_before, "body"),
        Some(true),
        "E.before holds A's accepted value"
    );
    assert_eq!(
        auth(&current(&after, &w), "body"),
        Some(false),
        "current holds E's pending value"
    );
    assert_eq!(
        auth(&e_after, "body"),
        Some(false),
        "E.after holds E's pending value"
    );
    for meta in [&e_before, &e_after, &current(&after, &w)] {
        assert_eq!(meta.fields["body"].timestamp_ms, 20, "timestamps unchanged");
    }
    assert_eq!(
        after.records[&e].frozen, before.records[&e].frozen,
        "E frozen bytes identical"
    );
    assert_eq!(
        after.records[&e].undo, before.records[&e].undo,
        "E row images identical"
    );
    assert_eq!(
        note(&db, &w).unwrap().fields,
        body("E"),
        "visible value unchanged"
    );
    (db.into_storage(), rows)
}

/// C1a-B: confirming E first promotes the current cell (E's accepted value); A's A-valued images
/// stay unauthoritative; a later confirmation of A changes nothing on body.
pub fn c1a_b<S: SyncStorage>(storage: S) -> (S, Vec<Row>) {
    let (mut db, clock, w, a, e) = equal_clock_pair(storage);
    confirm(&mut db, &w, &e);
    let (mut db, mid, _) = ledger(db, &clock);
    assert_eq!(
        auth(&current(&mid, &w), "body"),
        Some(true),
        "accepted current E is authoritative"
    );
    let (a_before, a_after) = images(&mid, &a);
    assert_eq!(
        auth(&a_after, "body"),
        Some(false),
        "A's A-valued after-image stays unauthoritative"
    );
    assert_eq!(
        a_before.fields["body"],
        FieldStamp {
            timestamp_ms: 10,
            authoritative: true
        }
    );
    let current_mid = current(&mid, &w);
    confirm(&mut db, &w, &a);
    let (db, after, rows) = ledger(db, &clock);
    assert_eq!(
        current(&after, &w),
        current_mid,
        "A's later confirmation changes nothing"
    );
    assert_eq!(note(&db, &w).unwrap().fields, body("E"));
    (db.into_storage(), rows)
}

/// C1a-NULL: removal and explicit Null at an equal clock are distinct presences.
/// `swap == false`: A removes `extra`, E sets it to explicit Null. `swap == true`: the reverse.
pub fn c1a_null<S: SyncStorage>(storage: S, swap: bool) -> (S, Vec<Row>) {
    let (mut db, clock) = owner(storage);
    let mut full = body("orig");
    full.insert("extra".into(), Value::String("x".into()));
    let w = confirmed_row(&mut db, &clock, full);
    clock.store(20, Ordering::Relaxed);
    let null = BTreeMap::from([("extra".to_string(), Value::Null)]);
    let remove = |db: &mut CommandDatabase<S>| run(db.replace("notes", &w, body("orig"))).unwrap();
    if swap {
        run(db.patch("notes", &w, null.clone())).unwrap();
    } else {
        remove(&mut db);
    }
    let a = last(&db);
    if swap {
        remove(&mut db);
    } else {
        run(db.patch("notes", &w, null)).unwrap();
    }
    let e = last(&db);
    db.prepare_push(21, 0).unwrap();
    let (mut db, before, _) = ledger(db, &clock);
    let remover = if swap {
        &e
    } else {
        &a
    };
    assert!(
        before.records[remover]
            .entity
            .as_ref()
            .unwrap()
            .removed_fields
            .contains("extra"),
        "setup: replace declares the removal"
    );
    confirm(&mut db, &w, &a);
    let (db, after, rows) = ledger(db, &clock);
    let cur = current(&after, &w);
    let (e_before, e_after) = images(&after, &e);
    assert_eq!(
        stamp(&cur, "extra").map(|s| s.timestamp_ms),
        Some(20),
        "stamp retained"
    );
    assert_eq!(
        auth(&cur, "extra"),
        Some(false),
        "differing presence is not promoted (current)"
    );
    assert_eq!(
        auth(&e_after, "extra"),
        Some(false),
        "differing presence is not promoted (E.after)"
    );
    assert_eq!(
        auth(&e_before, "extra"),
        Some(true),
        "A's own presence is promoted (E.before)"
    );
    let visible = note(&db, &w).unwrap();
    assert_eq!(
        visible.fields.get("extra"),
        if swap {
            None
        } else {
            Some(&Value::Null)
        }
    );
    assert_eq!(after.records[&e].frozen, before.records[&e].frozen);
    (db.into_storage(), rows)
}

/// C1a-CORRECTED (characterization): a corrected field keeps its selected stamp; the other
/// declared field is promoted.
pub fn c1a_corrected<S: SyncStorage>(storage: S) -> (S, Vec<Row>) {
    let (mut db, clock) = owner(storage);
    let w = confirmed_row(&mut db, &clock, body("orig"));
    clock.store(20, Ordering::Relaxed);
    let mut both = body("A");
    both.insert("extra".into(), Value::String("g".into()));
    run(db.patch("notes", &w, both)).unwrap();
    let a = last(&db);
    db.prepare_push(21, 0).unwrap();
    db.receive_entity("notes", &w, &a, Some(RemoteOutcome::Corrected(Json::Null)))
        .unwrap();
    let plan = ReconcilePlan {
        mappings: BTreeMap::new(),
        writes: vec![ReconcileWrite::Entity {
            change: EntityChange {
                table: "notes".into(),
                record_id: w.clone(),
                op: EntityOperation::Update,
                changes: body("S"),
                removed_fields: BTreeSet::new(),
                timestamp_ms: 300,
                field_timestamps: BTreeMap::from([("body".into(), 300)]),
            },
        }],
    };
    run(db.reconcile(&a, plan)).unwrap();
    let (db, after, rows) = ledger(db, &clock);
    let cur = current(&after, &w);
    assert_eq!(
        stamp(&cur, "body"),
        Some(FieldStamp {
            timestamp_ms: 300,
            authoritative: true
        })
    );
    assert_eq!(
        stamp(&cur, "extra"),
        Some(FieldStamp {
            timestamp_ms: 20,
            authoritative: true
        })
    );
    (db.into_storage(), rows)
}

/// C1a-DELETE (characterization): a declared delete promotes its matching tombstone.
pub fn c1a_delete<S: SyncStorage>(storage: S) -> (S, Vec<Row>) {
    let (mut db, clock) = owner(storage);
    let w = confirmed_row(&mut db, &clock, body("orig"));
    clock.store(20, Ordering::Relaxed);
    assert!(run(db.remove("notes", &w)).unwrap());
    let a = last(&db);
    db.prepare_push(21, 0).unwrap();
    confirm(&mut db, &w, &a);
    let (db, after, rows) = ledger(db, &clock);
    assert_eq!(
        current(&after, &w).tombstone,
        Some(FieldStamp {
            timestamp_ms: 20,
            authoritative: true
        })
    );
    assert!(note(&db, &w).is_none());
    (db.into_storage(), rows)
}

/// C1a-CTRL (characterization): distinct clocks keep today's flags exactly; a present timestamp 0
/// over a confirmed {10,true} baseline is promoted.
pub fn c1a_ctrl<S: SyncStorage>(fresh: impl Fn() -> S) -> Vec<(S, Vec<Row>)> {
    let mut stores = vec![];
    let (mut db, clock) = owner(fresh());
    let w = confirmed_row(&mut db, &clock, body("orig"));
    clock.store(20, Ordering::Relaxed);
    run(db.patch("notes", &w, body("A"))).unwrap();
    let a = last(&db);
    clock.store(21, Ordering::Relaxed);
    run(db.patch("notes", &w, body("E"))).unwrap();
    let e = last(&db);
    db.prepare_push(22, 0).unwrap();
    confirm(&mut db, &w, &a);
    let (db, after, rows) = ledger(db, &clock);
    let (e_before, e_after) = images(&after, &e);
    assert_eq!(
        stamp(&e_before, "body"),
        Some(FieldStamp {
            timestamp_ms: 20,
            authoritative: true
        })
    );
    assert_eq!(
        stamp(&e_after, "body"),
        Some(FieldStamp {
            timestamp_ms: 21,
            authoritative: false
        })
    );
    assert_eq!(
        stamp(&current(&after, &w), "body"),
        Some(FieldStamp {
            timestamp_ms: 21,
            authoritative: false
        })
    );
    stores.push((db.into_storage(), rows));

    // Same value, different timestamp: L restores A's value at a later clock. The current cell
    // holds A's value but L's timestamp, so it is L's pending intent and is not promoted.
    let (mut db, clock) = owner(fresh());
    let w = confirmed_row(&mut db, &clock, body("orig"));
    let mut written = vec![];
    for (time, value) in [(20, "A"), (21, "E"), (22, "A")] {
        clock.store(time, Ordering::Relaxed);
        run(db.patch("notes", &w, body(value))).unwrap();
        written.push(last(&db));
    }
    let (a, l) = (written[0].clone(), written[2].clone());
    db.prepare_push(23, 0).unwrap();
    confirm(&mut db, &w, &a);
    let (db, after, rows) = ledger(db, &clock);
    assert_eq!(
        stamp(&current(&after, &w), "body"),
        Some(FieldStamp {
            timestamp_ms: 22,
            authoritative: false
        }),
        "same value with a different timestamp is not promoted"
    );
    assert_eq!(auth(&images(&after, &l).1, "body"), Some(false));
    stores.push((db.into_storage(), rows));

    let (mut db, clock) = owner(fresh());
    let w = confirmed_row(&mut db, &clock, body("orig"));
    clock.store(0, Ordering::Relaxed);
    run(db.patch("notes", &w, body("A"))).unwrap();
    let a = last(&db);
    db.prepare_push(22, 0).unwrap();
    confirm(&mut db, &w, &a);
    let (db, after, rows) = ledger(db, &clock);
    assert_eq!(
        stamp(&current(&after, &w), "body"),
        Some(FieldStamp {
            timestamp_ms: 0,
            authoritative: true
        }),
        "a present timestamp 0 is valid and promoted"
    );
    stores.push((db.into_storage(), rows));
    stores
}

/// No retroactive repair (characterization): authority flags already written in an existing store
/// are not changed by reopening or by later unrelated confirmations.
pub fn c1a_no_repair<S: SyncStorage>(storage: S) -> (S, Vec<Row>) {
    let (db, clock, w, a, _e) = equal_clock_pair(storage);
    let (mut db, _, _) = ledger(db, &clock);
    confirm(&mut db, &w, &a);
    // Simulate a store written by the old stamp-only rule: the current body flag set to true.
    let mut storage = db.into_storage();
    let expected = storage.load_sync().unwrap();
    let mut old_rule = SyncLedger::from_rows(expected.as_deref().unwrap()).unwrap();
    let flag = &mut old_rule
        .entity
        .as_mut()
        .unwrap()
        .metadata
        .get_mut(&("notes".to_string(), w.clone()))
        .unwrap()
        .fields
        .get_mut("body")
        .unwrap()
        .authoritative;
    // Write the old-rule flag only if the rule under test left it false; a real change needs the
    // entity revision bump storage requires (a no-op rewrite would make this case vacuous).
    if !*flag {
        *flag = true;
        old_rule.entity.as_mut().unwrap().revision += 1;
    }
    let state = run(storage.catalog()).unwrap().state.unwrap();
    storage
        .apply_sync_plan(SyncPlan {
            transition: SyncTransition::Normal,
            expected_model: state.clone(),
            next_model: state,
            expected_sync: expected,
            next_sync: old_rule.to_rows().unwrap(),
            guards: vec![],
            mutations: vec![],
        })
        .unwrap();
    let mut db = open(storage);
    configure(&mut db, &clock);
    let other = confirmed_row(&mut db, &clock, body("other"));
    let (db, after, rows) = ledger(db, &clock);
    assert_ne!(other, w);
    assert_eq!(
        auth(&current(&after, &w), "body"),
        Some(true),
        "existing flags are not demoted"
    );
    (db.into_storage(), rows)
}

/// C1a mapping: an entity Create mapped from its local id to a different remote id, with a pending
/// same-clock entity patch E that depends on it (E is withheld, unsent). Confirmation rekeys the
/// current metadata and E's paired row/metadata images to the remote id, and promotes only the
/// Create's accepted value (E.before), not E's different value (current, E.after).
pub fn c1a_mapping<S: SyncStorage>(storage: S) -> (S, Vec<Row>) {
    let (mut db, clock) = owner(storage);
    let row = run(db.insert("notes", body("orig"))).unwrap();
    let c = last(&db);
    run(db.patch("notes", &row.id, body("E"))).unwrap();
    let e = last(&db);
    assert_eq!(
        keys(&db.prepare_push(10, 0).unwrap()),
        vec![c.clone()],
        "setup: E waits on C"
    );
    db.receive_entity(
        "notes",
        &row.id,
        &c,
        Some(RemoteOutcome::Applied(Json::Null)),
    )
    .unwrap();
    let (mut db, before, _) = ledger(db, &clock);
    assert_eq!(before.records[&e].frozen, None, "setup: E unsent");
    run(db.reconcile(
        &c,
        ReconcilePlan {
            mappings: BTreeMap::from([(row.id.clone(), "srv-1".into())]),
            writes: vec![],
        },
    ))
    .unwrap();
    let (db, after, rows) = ledger(db, &clock);
    let metadata = &after.entity.as_ref().unwrap().metadata;
    assert!(
        !metadata.contains_key(&("notes".to_string(), row.id.clone())),
        "old key moved"
    );
    let current = current(&after, "srv-1");
    assert_eq!(
        stamp(&current, "body"),
        Some(FieldStamp {
            timestamp_ms: 10,
            authoritative: false
        })
    );
    let record = &after.records[&e];
    assert_eq!(record.metadata_undo[0].record_id, "srv-1");
    assert_eq!(record.undo[0].id, "srv-1");
    let (e_before, e_after) = images(&after, &e);
    assert_eq!(
        stamp(&e_before, "body"),
        Some(FieldStamp {
            timestamp_ms: 10,
            authoritative: true
        })
    );
    assert_eq!(
        stamp(&e_after, "body"),
        Some(FieldStamp {
            timestamp_ms: 10,
            authoritative: false
        })
    );
    assert_eq!(
        record.frozen, None,
        "E is still unsent; no frozen bytes exist yet"
    );
    let Json::Object(args) = &record.args else {
        panic!("object args")
    };
    assert_eq!(
        args.get("recordId"),
        Some(&Json::String("srv-1".into())),
        "queued args normalized"
    );
    assert_eq!(note(&db, "srv-1").unwrap().fields, body("E"));
    (db.into_storage(), rows)
}
