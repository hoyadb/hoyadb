//! SPEC-0038 / TC-0042: safe confirmation, row-causal dispatch and explicit resolution paths.
//! Uses only pre-existing public symbols so the unchanged implementation compiles and fails
//! by assertion. awaiting_key and request_redelivery are asserted through the bindings facade.
use hoyadb_core::command::records::SyncLedger;
use hoyadb_core::model::*;
use hoyadb_core::{command::*, storage::*};
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
        Poll::Ready(v) => v,
        Poll::Pending => panic!("immediate adapter suspended"),
    }
}

fn column(kind: ColumnType, optional: bool) -> Column {
    Column {
        kind,
        optional,
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
                        ("body".into(), column(ColumnType::String, false)),
                        ("extra".into(), column(ColumnType::String, true)),
                    ]),
                    relations: BTreeMap::new(),
                },
            ),
            (
                "links".into(),
                TableDefinition {
                    columns: BTreeMap::from([(
                        "note".into(),
                        column(ColumnType::Reference, false),
                    )]),
                    relations: BTreeMap::new(),
                },
            ),
        ]),
    }
}

pub fn body(value: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([("body".into(), Value::String(value.into()))])
}

pub fn open<S: SyncStorage>(storage: S) -> CommandDatabase<S> {
    run(CommandDatabase::open(
        storage,
        schema(),
        vec![],
        [42; 16],
        SyncOptions::default(),
    ))
    .unwrap()
}

fn definition(reference: bool) -> CommandDefinition {
    CommandDefinition {
        name: "note-v1".into(),
        arguments: BTreeMap::from([(
            "target".into(),
            Argument {
                kind: ArgumentType::String,
                optional: true,
            },
        )]),
        reference_paths: if reference {
            vec![vec![PathPart::Key("target".into())]]
        } else {
            vec![]
        },
    }
}

fn begin<S: SyncStorage>(
    db: &mut CommandDatabase<S>,
    target: Option<&str>,
    now: i64,
) -> CommandRequest {
    let request = db.request().unwrap();
    let args = Json::Object(
        target
            .map(|t| BTreeMap::from([("target".into(), Json::String(t.into()))]))
            .unwrap_or_default(),
    );
    assert!(
        run(db.begin(
            request,
            definition(target.is_some()),
            args,
            Some(Json::Null),
            now
        ))
        .unwrap()
    );
    request
}

fn target<S: SyncStorage>(db: &CommandDatabase<S>, request: CommandRequest) -> String {
    let Json::Object(args) = db.scope_args(request).unwrap() else {
        panic!("object args")
    };
    let Json::String(id) = &args["target"] else {
        panic!("string target")
    };
    id.clone()
}

/// A command replacing an existing row (no declared reference: the row id is authoritative).
pub fn edit<S: SyncStorage>(
    db: &mut CommandDatabase<S>,
    id: &str,
    value: &str,
    now: i64,
) -> String {
    let request = begin(db, None, now);
    run(db.scope_replace(request, "notes", id, body(value))).unwrap();
    run(db.finish(request, Json::Null)).unwrap().key
}

pub fn create<S: SyncStorage>(
    db: &mut CommandDatabase<S>,
    value: &str,
    now: i64,
) -> (String, String) {
    let request = begin(db, None, now);
    let row = run(db.scope_insert(request, "notes", body(value))).unwrap();
    (run(db.finish(request, Json::Null)).unwrap().key, row.id)
}

/// A dependent command that edits a created row through a declared reference path and links to it.
fn dependent_edit<S: SyncStorage>(
    db: &mut CommandDatabase<S>,
    local: &str,
    value: &str,
    now: i64,
) -> (String, String) {
    let request = begin(db, Some(local), now);
    let id = target(db, request);
    run(db.scope_replace(request, "notes", &id, body(value))).unwrap();
    let link = run(db.scope_insert(
        request,
        "links",
        BTreeMap::from([("note".into(), Value::Reference(id.clone()))]),
    ))
    .unwrap();
    (run(db.finish(request, Json::Null)).unwrap().key, link.id)
}

fn dependent_remove<S: SyncStorage>(db: &mut CommandDatabase<S>, local: &str, now: i64) -> String {
    let request = begin(db, Some(local), now);
    let id = target(db, request);
    assert!(run(db.scope_remove(request, "notes", &id)).unwrap());
    run(db.finish(request, Json::Null)).unwrap().key
}

pub fn keys(batch: &PushBatch) -> Vec<String> {
    batch
        .envelopes
        .iter()
        .map(|envelope| {
            let Json::Object(wire) = Json::parse(envelope).unwrap() else {
                panic!("object envelope")
            };
            match wire.get("idempotencyKey").or_else(|| wire.get("writeId")) {
                Some(Json::String(key)) => key.clone(),
                _ => panic!("keyed envelope"),
            }
        })
        .collect()
}

fn note<S: SyncStorage>(db: &CommandDatabase<S>, id: &str) -> Option<Row> {
    run(db.query("notes").fetch(db))
        .unwrap()
        .into_iter()
        .find(|r| r.id == id)
}

fn all_rows<S: SyncStorage>(db: &CommandDatabase<S>) -> Vec<Row> {
    let mut rows = run(db.query("notes").fetch(db)).unwrap();
    rows.extend(run(db.query("links").fetch(db)).unwrap());
    rows
}

fn entry<S: SyncStorage>(db: &CommandDatabase<S>, key: &str) -> QueueEntry {
    db.queue_status()
        .commands
        .into_iter()
        .find(|e| e.key == key)
        .expect("known key")
}

fn seeded<S: SyncStorage>(storage: S) -> CommandDatabase<S> {
    let mut db = open(storage);
    run(db.insert_proposed("notes", body("original"), Some("server-note".into()))).unwrap();
    run(db.insert_proposed("notes", body("other"), Some("other-note".into()))).unwrap();
    db
}

/// Rewrites every Queued record as a pre-upgrade Sent record with its exact frozen envelope,
/// directly through the storage capability (independent of the dispatch rule under test).
pub fn force_sent<S: SyncStorage>(mut storage: S) -> S {
    let expected = storage.load_sync().unwrap();
    let mut ledger = SyncLedger::from_rows(expected.as_deref().unwrap()).unwrap();
    for record in ledger.records.values_mut() {
        if record.state == SyncState::Queued {
            record.frozen = Some(record.envelope().canonical().unwrap());
            record.state = SyncState::Sent;
            record.attempts = 1;
            record.due_ms = 0;
        }
    }
    let state = run(storage.catalog()).unwrap().state.unwrap();
    storage
        .apply_sync_plan(SyncPlan {
            transition: SyncTransition::Normal,
            expected_model: state.clone(),
            next_model: state,
            expected_sync: expected,
            next_sync: ledger.to_rows().unwrap(),
            guards: vec![],
            mutations: vec![],
        })
        .unwrap();
    storage
}

/// R1-CTRL: the DEF-0032 pair (both sent before RD existed) confirms in either order.
pub fn r1_ctrl<S: SyncStorage>(fresh: impl Fn() -> S) {
    for b_first in [false, true] {
        let mut db = seeded(fresh());
        let a = edit(&mut db, "server-note", "first offline edit", 100);
        let b = edit(&mut db, "server-note", "second offline edit", 101);
        let mut db = open(force_sent(db.into_storage()));
        db.receive(&a, Some(RemoteOutcome::Applied(Json::Null)))
            .unwrap();
        db.receive(&b, Some(RemoteOutcome::Applied(Json::Null)))
            .unwrap();
        let order = if b_first {
            [&b, &a]
        } else {
            [&a, &b]
        };
        for key in order {
            assert_eq!(
                run(db.reconcile(key, ReconcilePlan::default())),
                Ok(()),
                "b_first={b_first}"
            );
        }
        assert_eq!(db.queue_status().live, 0);
        assert_eq!(
            note(&db, "server-note").unwrap().fields,
            body("second offline edit")
        );
        db.close();
    }
}

struct M1 {
    a: String,
    c: String,
    x: String,
    link: String,
}

fn m1_state<S: SyncStorage>(storage: S) -> (CommandDatabase<S>, M1) {
    let mut db = open(storage);
    let (a, x) = create(&mut db, "created", 100);
    let (c, link) = dependent_edit(&mut db, &x, "dependent edit", 101);
    run(db.replace("notes", &x, body("direct edit"))).unwrap();
    assert_eq!(
        keys(&db.prepare_push(102, 0).unwrap()),
        vec![a.clone()],
        "C waits for A's mapping"
    );
    (
        db,
        M1 {
            a,
            c,
            x,
            link,
        },
    )
}

/// M1: mapping with a later dependent and a later direct edit confirms without loss.
/// Returns (rows, ledger rows) after the confirmation for a restart comparison.
pub fn m1<S: SyncStorage>(storage: S) -> (S, Vec<Row>, Option<Vec<Row>>) {
    let (mut db, m) = m1_state(storage);
    db.receive(&m.a, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    let plan = ReconcilePlan {
        mappings: BTreeMap::from([(m.x.clone(), "srv1".into())]),
        writes: vec![],
    };
    assert_eq!(run(db.reconcile(&m.a, plan)), Ok(()), "M1 confirmation");
    assert!(note(&db, &m.x).is_none());
    assert_eq!(
        note(&db, "srv1").unwrap().fields,
        body("direct edit"),
        "direct edit carried"
    );
    let link = run(db.query("links").fetch(&db)).unwrap();
    assert_eq!(link.len(), 1);
    assert_eq!(link[0].id, m.link);
    assert_eq!(
        link[0].fields["note"],
        Value::Reference("srv1".into()),
        "declared Reference rewritten"
    );
    let sent = db.prepare_push(103, 0).unwrap();
    assert_eq!(keys(&sent), vec![m.c.clone()]);
    let Json::Object(wire) = Json::parse(&sent.envelopes[0]).unwrap() else {
        panic!("object")
    };
    assert_eq!(
        wire["args"],
        Json::Object(BTreeMap::from([(
            "target".into(),
            Json::String("srv1".into())
        )])),
        "queued argument normalized to the final id"
    );
    let rows = all_rows(&db);
    let storage = db.into_storage();
    let ledger = storage.load_sync().unwrap();
    let parsed = SyncLedger::from_rows(ledger.as_deref().unwrap()).unwrap();
    let c_undo = &parsed.records[&m.c].undo;
    assert!(
        c_undo.iter().all(|change| change.id != m.x),
        "C's undo uses final identities"
    );
    assert!(c_undo.iter().any(|change| change.id == "srv1"));
    let reopened = open(storage);
    assert_eq!(all_rows(&reopened), rows, "same rows after reopen");
    let storage = reopened.into_storage();
    assert_eq!(
        storage.load_sync().unwrap(),
        ledger,
        "same ledger after reopen"
    );
    (storage, rows, ledger)
}

/// M2: a correction write under a later direct edit stays guarded; the no-write plan then confirms.
pub fn m2<S: SyncStorage>(storage: S) {
    let (mut db, m) = m1_state(storage);
    db.receive(&m.a, Some(RemoteOutcome::Corrected(Json::Null)))
        .unwrap();
    let rows = all_rows(&db);
    let status = db.queue_status();
    let correction = ReconcilePlan {
        mappings: BTreeMap::from([(m.x.clone(), "srv1".into())]),
        writes: vec![ReconcileWrite::Put {
            table: "notes".into(),
            id: "srv1".into(),
            fields: body("corrected"),
        }],
    };
    assert_eq!(
        run(db.reconcile(&m.a, correction)),
        Err(CommandError::RollbackConflict)
    );
    assert_eq!(all_rows(&db), rows, "no cell or mapping changed");
    assert!(db.queue_status() == status, "ledger unchanged");
    let confirm = ReconcilePlan {
        mappings: BTreeMap::from([(m.x.clone(), "srv1".into())]),
        writes: vec![],
    };
    assert_eq!(
        run(db.reconcile(&m.a, confirm)),
        Ok(()),
        "explicit no-write confirmation"
    );
    run(db.replace("notes", "srv1", body("corrected"))).unwrap();
    assert_eq!(note(&db, "srv1").unwrap().fields, body("corrected"));
    db.close();
}

/// M3: a later delete of a created row before mapping.
pub fn m3<S: SyncStorage>(storage: S) {
    let mut db = open(storage);
    let (a, x) = create(&mut db, "created", 100);
    let d = dependent_remove(&mut db, &x, 101);
    assert_eq!(keys(&db.prepare_push(102, 0).unwrap()), vec![a.clone()]);
    db.receive(&a, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    let plan = ReconcilePlan {
        mappings: BTreeMap::from([(x.clone(), "srv1".into())]),
        writes: vec![],
    };
    assert_eq!(run(db.reconcile(&a, plan)), Ok(()));
    assert!(note(&db, "srv1").is_none() && note(&db, &x).is_none());
    assert_eq!(keys(&db.prepare_push(103, 0).unwrap()), vec![d.clone()]);
    db.receive(&d, Some(RemoteOutcome::Rejected("gone".into())))
        .unwrap();
    run(db.reconcile(&d, ReconcilePlan::default())).unwrap();
    assert_eq!(run(db.rollback(&d)), Ok(()));
    assert_eq!(
        note(&db, "srv1").unwrap().fields,
        body("created"),
        "restored under the final id"
    );
    db.close();
}

/// M4 (characterization): invalid no-write mappings keep InvalidCommand and change nothing.
pub fn m4<S: SyncStorage>(storage: S) {
    let mut db = seeded(storage);
    let (owner, z) = create(&mut db, "owned elsewhere", 99);
    let request = begin(&mut db, None, 100);
    let x = run(db.scope_insert(request, "notes", body("x")))
        .unwrap()
        .id;
    let y = run(db.scope_insert(request, "notes", body("y")))
        .unwrap()
        .id;
    let a = run(db.finish(request, Json::Null)).unwrap().key;
    assert_eq!(
        keys(&db.prepare_push(101, 0).unwrap()),
        vec![owner.clone(), a.clone()]
    );
    db.receive(&owner, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    let owned = ReconcilePlan {
        mappings: BTreeMap::from([(z, "srv-owned".into())]),
        writes: vec![],
    };
    run(db.reconcile(&owner, owned)).unwrap();
    db.receive(&a, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    let rows = all_rows(&db);
    let map = |pairs: &[(&str, &str)]| ReconcilePlan {
        mappings: pairs
            .iter()
            .map(|(l, r)| ((*l).into(), (*r).into()))
            .collect(),
        writes: vec![],
    };
    for plan in [
        map(&[(&x, "server-note"), (&y, "srv-y")]),
        map(&[(&x, "srv-same"), (&y, "srv-same")]),
        map(&[(&x, "local:z"), (&y, "srv-y")]),
        map(&[(&x, "srv-owned"), (&y, "srv-y")]),
        map(&[(&x, "srv-x")]),
    ] {
        assert_eq!(
            run(db.reconcile(&a, plan)),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(all_rows(&db), rows);
    }
    db.close();
}

/// RD1: a later same-row command waits for the earlier one; unrelated work is sent.
pub fn rd1<S: SyncStorage>(fresh: impl Fn() -> S) {
    let mut db = seeded(fresh());
    let a = edit(&mut db, "server-note", "a", 100);
    let b = edit(&mut db, "server-note", "b", 101);
    let c = edit(&mut db, "other-note", "c", 102);
    assert_eq!(
        keys(&db.prepare_push(103, 0).unwrap()),
        vec![a.clone(), c.clone()]
    );
    assert_eq!(entry(&db, &b).state, SyncState::Queued);
    db.receive(&a, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    assert!(
        !keys(&db.prepare_push(104, 0).unwrap()).contains(&b),
        "withheld until reconciled"
    );
    run(db.reconcile(&a, ReconcilePlan::default())).unwrap();
    assert_eq!(keys(&db.prepare_push(105, 0).unwrap()), vec![b.clone()]);
    db.close();
    // Rejected branch: B stays withheld while A is not rolled back.
    let mut db = seeded(fresh());
    let a = edit(&mut db, "server-note", "a", 100);
    let b = edit(&mut db, "server-note", "b", 101);
    assert_eq!(keys(&db.prepare_push(102, 0).unwrap()), vec![a.clone()]);
    db.receive(&a, Some(RemoteOutcome::Rejected("no".into())))
        .unwrap();
    run(db.reconcile(&a, ReconcilePlan::default())).unwrap();
    assert!(keys(&db.prepare_push(103, 0).unwrap()).is_empty());
    assert_eq!(
        run(db.rollback(&a)),
        Err(CommandError::RollbackConflict),
        "B's write blocks A's inverse"
    );
    assert_eq!(entry(&db, &b).state, SyncState::Queued);
    db.close();
}

/// RD2: pre-upgrade sent pairs keep retrying; the possible server reorder is a documented residual.
pub fn rd2<S: SyncStorage>(storage: S) {
    let mut db = seeded(storage);
    let a = edit(&mut db, "server-note", "a", 100);
    let b = edit(&mut db, "server-note", "b", 101);
    let mut db = open(force_sent(db.into_storage()));
    let first = db.prepare_push(200, 0).unwrap();
    assert_eq!(
        keys(&first),
        vec![a.clone(), b.clone()],
        "both legacy records retry together"
    );
    let later = db.prepare_push(200 + 60_000 * 2, 0).unwrap();
    assert_eq!(later.envelopes, first.envelopes, "identical frozen bytes");
    db.receive(&b, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    assert_eq!(run(db.reconcile(&b, ReconcilePlan::default())), Ok(()));
    db.close();
}

/// RD3: without a reconcile callback same-row work waits visibly; nothing is applied automatically.
pub fn rd3<S: SyncStorage>(storage: S) {
    let mut db = seeded(storage);
    let a = edit(&mut db, "server-note", "a", 100);
    let b = edit(&mut db, "server-note", "b", 101);
    db.prepare_push(102, 0).unwrap();
    db.receive(&a, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    for now in [103, 2_000, 200_000] {
        assert!(!keys(&db.prepare_push(now, 0).unwrap()).contains(&b));
    }
    assert_eq!(entry(&db, &b).state, SyncState::Queued);
    assert_eq!(
        db.pending_reconciliations().len(),
        1,
        "no automatic reconcile"
    );
    assert_eq!(
        note(&db, "server-note").unwrap().fields,
        body("b"),
        "no automatic rollback"
    );
    run(db.reconcile(&a, ReconcilePlan::default())).unwrap();
    assert_eq!(keys(&db.prepare_push(200_001, 0).unwrap()), vec![b]);
    db.close();
}

/// X1: correction with later unsent work resolves explicitly; final rows equal the server sequence.
pub fn x1<S: SyncStorage>(storage: S) {
    let mut db = seeded(storage);
    let a = edit(&mut db, "server-note", "a", 100);
    let e = edit(&mut db, "server-note", "e", 101);
    let mut server = vec![];
    let first = db.prepare_push(102, 0).unwrap();
    assert_eq!(keys(&first), vec![a.clone()], "E withheld");
    server.extend(keys(&first));
    db.receive(&a, Some(RemoteOutcome::Corrected(Json::Null)))
        .unwrap();
    let correction = ReconcilePlan {
        mappings: BTreeMap::new(),
        writes: vec![ReconcileWrite::Put {
            table: "notes".into(),
            id: "server-note".into(),
            fields: body("a corrected"),
        }],
    };
    let before = all_rows(&db);
    assert_eq!(
        run(db.reconcile(&a, correction.clone())),
        Err(CommandError::RollbackConflict)
    );
    assert_eq!(all_rows(&db), before);
    assert_eq!(run(db.cancel(&e)), Ok(()));
    assert_eq!(run(db.reconcile(&a, correction)), Ok(()));
    let n = edit(&mut db, "server-note", "e", 103);
    let next = db.prepare_push(104, 0).unwrap();
    assert_eq!(keys(&next), vec![n.clone()]);
    server.extend(keys(&next));
    assert_eq!(
        server,
        vec![a, n],
        "server sequence: corrected A, then E's intent"
    );
    assert_eq!(
        note(&db, "server-note").unwrap().fields,
        body("e"),
        "final rows equal the server sequence"
    );
    db.close();
}

/// X2: rejection with later unsent work never reports a false rollback.
pub fn x2<S: SyncStorage>(storage: S) {
    let mut db = seeded(storage);
    let p = edit(&mut db, "server-note", "p", 100);
    let e = edit(&mut db, "server-note", "e", 101);
    assert_eq!(keys(&db.prepare_push(102, 0).unwrap()), vec![p.clone()]);
    db.receive(&p, Some(RemoteOutcome::Rejected("denied".into())))
        .unwrap();
    run(db.reconcile(&p, ReconcilePlan::default())).unwrap();
    assert_eq!(run(db.rollback(&p)), Err(CommandError::RollbackConflict));
    let status = entry(&db, &p);
    assert!(!status.rolled_back);
    assert!(db.queue_status().live >= 2, "P stays live");
    assert!(
        !keys(&db.prepare_push(103, 0).unwrap()).contains(&e),
        "P still withholds E"
    );
    assert_eq!(run(db.cancel(&e)), Ok(()));
    assert_eq!(run(db.rollback(&p)), Ok(()));
    assert!(entry(&db, &p).rolled_back);
    assert_eq!(note(&db, "server-note").unwrap().fields, body("original"));
    db.close();
}

/// CTRL-K, superseded by SPEC-0039 / TC-0043 GW01 (DEF-0038): an unguarded out-of-closure
/// correction write is refused and the unrecorded direct edit is kept.
pub fn ctrl_k<S: SyncStorage>(storage: S) {
    let mut db = seeded(storage);
    let a = edit(&mut db, "server-note", "a", 100);
    run(db.replace("notes", "other-note", body("direct"))).unwrap();
    db.prepare_push(101, 0).unwrap();
    db.receive(&a, Some(RemoteOutcome::Corrected(Json::Null)))
        .unwrap();
    let plan = ReconcilePlan {
        mappings: BTreeMap::new(),
        writes: vec![ReconcileWrite::Put {
            table: "notes".into(),
            id: "other-note".into(),
            fields: body("backend"),
        }],
    };
    assert_eq!(
        run(db.reconcile(&a, plan)),
        Err(CommandError::RollbackConflict)
    );
    assert_eq!(
        note(&db, "other-note").unwrap().fields,
        body("direct"),
        "DEF-0038: the direct edit is kept"
    );
    db.close();
}

// ---- entity owner cases ----

fn entity_owner<S: SyncStorage>(storage: S) -> (CommandDatabase<S>, Arc<AtomicI64>) {
    let mut db = open(storage);
    let clock = Arc::new(AtomicI64::new(100));
    let captured = clock.clone();
    db.configure_entities(
        EntityConfig {
            scope: "recovery-fixture".into(),
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

/// An entity row whose create has been confirmed, so later patches carry no dependency.
fn entity_row<S: SyncStorage>(db: &mut CommandDatabase<S>) -> String {
    let row = run(db.insert("notes", body("entity original"))).unwrap();
    let create = keys(&db.prepare_push(100, 0).unwrap());
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

fn last_key<S: SyncStorage>(db: &CommandDatabase<S>) -> String {
    db.queue_status().commands.last().unwrap().key.clone()
}

/// ET2 (characterization): identity-only entity receipt with a later edit keeps the edit.
pub fn et2<S: SyncStorage>(storage: S) {
    let (mut db, clock) = entity_owner(storage);
    let id = entity_row(&mut db);
    clock.store(200, Ordering::Relaxed);
    run(db.patch("notes", &id, body("a"))).unwrap();
    let a = last_key(&db);
    clock.store(201, Ordering::Relaxed);
    run(db.patch("notes", &id, body("later"))).unwrap();
    db.prepare_push(202, 0).unwrap();
    db.receive_entity("notes", &id, &a, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    let plan = ReconcilePlan {
        mappings: BTreeMap::new(), // an update patch creates nothing, so no mapping keys
        writes: vec![],
    };
    assert_eq!(run(db.reconcile(&a, plan)), Ok(()));
    assert_eq!(
        note(&db, &id).unwrap().fields["body"],
        Value::String("later".into())
    );
    db.close();
}

/// ET3: processed entity rejection with later entity work; the entity pair is exempt from RD.
/// EXPECTATION CHANGED for SPEC-0038 C4 (TC-0042 ET3): this was a characterization of
/// RollbackConflict. Under C4 the rollback re-bases the pending E, which still wins.
pub fn et3<S: SyncStorage>(storage: S) {
    let (mut db, clock) = entity_owner(storage);
    let id = entity_row(&mut db);
    clock.store(200, Ordering::Relaxed);
    run(db.patch("notes", &id, body("p"))).unwrap();
    let p = last_key(&db);
    clock.store(201, Ordering::Relaxed);
    run(db.patch("notes", &id, body("e"))).unwrap();
    let e = last_key(&db);
    assert_eq!(
        keys(&db.prepare_push(202, 0).unwrap()),
        vec![p.clone(), e.clone()],
        "entity pair exempt"
    );
    db.receive_entity("notes", &id, &p, Some(RemoteOutcome::Rejected("no".into())))
        .unwrap();
    run(db.reconcile(&p, ReconcilePlan::default())).unwrap();
    let (mut db, before, _) = entity_ledger(db, &clock);
    // Today's exact images (these pass on unchanged code).
    let pending = entity_image(&id, &[("body", "e")], &[("body", 201, false)]);
    entity_exact(
        &before,
        &e,
        entity_image(&id, &[("body", "p")], &[("body", 200, false)]),
        pending.clone(),
    );
    assert_eq!(entity_current(&before, &db, &id), pending);
    assert_eq!(
        run(db.rollback(&p)),
        Ok(()),
        "C4 rollback over the pending E"
    );
    assert!(entry(&db, &p).rolled_back, "status reports the rollback");
    let (mut db, after, _) = entity_ledger(db, &clock);
    // E.before carries P.before's value and stamp; E.after and the current row stay E's.
    let restored = entity_image(&id, &[("body", "entity original")], &[("body", 100, true)]);
    entity_exact(&after, &e, restored.clone(), pending.clone());
    entity_preserved(&before, &after, &e);
    entity_others(&before, &after, &[&p, &e]);
    assert_eq!(entity_current(&after, &db, &id), pending);
    assert!(after.records[&p].rolled_back);
    db.receive_entity("notes", &id, &e, Some(RemoteOutcome::Rejected("no".into())))
        .unwrap();
    run(db.reconcile(&e, ReconcilePlan::default())).unwrap();
    assert_eq!(run(db.rollback(&e)), Ok(()));
    let (db, last, _) = entity_ledger(db, &clock);
    assert_eq!(
        entity_current(&last, &db, &id),
        restored,
        "rolling E back restores P.before exactly"
    );
    db.close();
}

/// Reopens an entity owner through its storage and returns the parsed ledger and its raw rows.
fn entity_ledger<S: SyncStorage>(
    db: CommandDatabase<S>,
    clock: &Arc<AtomicI64>,
) -> (CommandDatabase<S>, SyncLedger, Vec<Row>) {
    let storage = db.into_storage();
    let rows = storage.load_sync().unwrap().unwrap();
    let ledger = SyncLedger::from_rows(&rows).unwrap();
    let (db, reopened) = entity_owner(storage);
    reopened.store(clock.load(Ordering::Relaxed), Ordering::Relaxed);
    (db, ledger, rows)
}

type EntityImage = (Option<Row>, Option<entity::RowMetadata>);

/// A whole `notes` image: exactly these fields and exactly these stamps.
fn entity_image(id: &str, values: &[(&str, &str)], stamps: &[(&str, i64, bool)]) -> EntityImage {
    (
        Some(Row {
            id: id.into(),
            fields: values
                .iter()
                .map(|(k, v)| (k.to_string(), Value::String(v.to_string())))
                .collect(),
        }),
        Some(entity::RowMetadata {
            fields: stamps
                .iter()
                .map(|(k, t, a)| {
                    (
                        k.to_string(),
                        entity::FieldStamp {
                            timestamp_ms: *t,
                            authoritative: *a,
                        },
                    )
                })
                .collect(),
            tombstone: None,
        }),
    )
}

/// A record's single row and metadata change, before and after, compared whole.
fn entity_exact(ledger: &SyncLedger, key: &str, before: EntityImage, after: EntityImage) {
    let record = &ledger.records[key];
    assert_eq!((record.undo.len(), record.metadata_undo.len()), (1, 1));
    assert_eq!(
        (
            record.undo[0].before.clone(),
            record.metadata_undo[0].before.clone()
        ),
        before,
        "whole before image"
    );
    assert_eq!(
        (
            record.undo[0].after.clone(),
            record.metadata_undo[0].after.clone()
        ),
        after,
        "whole after image"
    );
}

/// Every field of a pending record except its undo and metadata undo is byte-identical.
fn entity_preserved(before: &SyncLedger, after: &SyncLedger, key: &str) {
    let actual = &after.records[key];
    let mut expected = before.records[key].clone();
    expected.undo = actual.undo.clone();
    expected.metadata_undo = actual.metadata_undo.clone();
    assert!(expected == *actual, "only undo and metadata undo change");
}

/// On C4 success every record outside K and L is byte-identical, and none is added or removed.
fn entity_others(before: &SyncLedger, after: &SyncLedger, touched: &[&String]) {
    assert!(before.records.keys().eq(after.records.keys()));
    for (key, record) in &before.records {
        if !touched.contains(&key) {
            assert!(
                after.records[key] == *record,
                "a record outside K and L is identical"
            );
        }
    }
}

/// The whole current row and metadata of an entity row.
fn entity_current<S: SyncStorage>(
    ledger: &SyncLedger,
    db: &CommandDatabase<S>,
    id: &str,
) -> EntityImage {
    (
        note(db, id),
        ledger
            .entity
            .as_ref()
            .unwrap()
            .metadata
            .get(&("notes".to_string(), id.to_string()))
            .cloned(),
    )
}

/// ET4: the RD exemption applies only to entity-entity pairs.
pub fn et4<S: SyncStorage>(storage: S) {
    let (mut db, clock) = entity_owner(storage);
    let ee = entity_row(&mut db);
    let ce = entity_row(&mut db);
    let ec = entity_row(&mut db);
    clock.store(200, Ordering::Relaxed);
    run(db.patch("notes", &ee, body("ee-1"))).unwrap();
    let ee1 = last_key(&db);
    run(db.patch("notes", &ee, body("ee-2"))).unwrap();
    let ee2 = last_key(&db);
    let ce1 = edit(&mut db, &ce, "ce-command", 201);
    run(db.patch("notes", &ce, body("ce-entity"))).unwrap();
    let ce2 = last_key(&db);
    run(db.patch("notes", &ec, body("ec-entity"))).unwrap();
    let ec1 = last_key(&db);
    let ec2 = edit(&mut db, &ec, "ec-command", 202);
    let sent = keys(&db.prepare_push(203, 0).unwrap());
    assert!(
        sent.contains(&ee1) && sent.contains(&ee2),
        "entity-entity pair sent together"
    );
    assert!(
        sent.contains(&ce1) && !sent.contains(&ce2),
        "command then entity: entity withheld"
    );
    assert!(
        sent.contains(&ec1) && !sent.contains(&ec2),
        "entity then command: command withheld"
    );
    db.close();
}

/// ET1: an entity typed correction over a later pending entity patch.
/// EXPECTATION CHANGED for SPEC-0038 C4 (TC-0042 ET1): both variants were a characterization of
/// RollbackConflict. Under C4 the different-field variant succeeds (the pending patch is re-based);
/// the same-field variant (patch at 201, older than the correction at 300) still refuses (rule G2)
/// with the ledger, rows, status and pending outcomes exactly unchanged and no notification.
pub fn et1<S: SyncStorage>(fresh: impl Fn() -> S) {
    for same_field in [true, false] {
        let (mut db, clock) = entity_owner(fresh());
        let id = entity_row(&mut db);
        clock.store(200, Ordering::Relaxed);
        run(db.patch("notes", &id, body("a"))).unwrap();
        let a = last_key(&db);
        clock.store(201, Ordering::Relaxed);
        let later = if same_field {
            body("later")
        } else {
            BTreeMap::from([("extra".into(), Value::String("later".into()))])
        };
        run(db.patch("notes", &id, later)).unwrap();
        let later_key = last_key(&db);
        db.prepare_push(202, 0).unwrap();
        db.receive_entity("notes", &id, &a, Some(RemoteOutcome::Corrected(Json::Null)))
            .unwrap();
        let plan = ReconcilePlan {
            mappings: BTreeMap::new(), // an update patch creates nothing, so no mapping keys
            writes: vec![ReconcileWrite::Entity {
                change: EntityChange {
                    table: "notes".into(),
                    record_id: id.clone(),
                    op: EntityOperation::Update,
                    changes: body("backend"),
                    removed_fields: BTreeSet::new(),
                    timestamp_ms: 300,
                    field_timestamps: BTreeMap::from([("body".into(), 300)]),
                },
            }],
        };
        let (mut db, prior, prior_rows) = entity_ledger(db, &clock);
        if same_field {
            let (status, pending, rows) = (
                db.queue_status(),
                db.pending_reconciliations(),
                all_rows(&db),
            );
            let query = db.query("notes");
            let data = run(db.observe(&query)).unwrap();
            let outcome = db.observe_outcome(&a).unwrap();
            db.drain_events(0);
            assert_eq!(
                run(db.reconcile(&a, plan)),
                Err(CommandError::RollbackConflict),
                "same field: the pending patch at 201 is older than the correction (G2)"
            );
            assert!(
                !db.drain_events(0).iter().any(|event| matches!(
                    event,
                    CommandEvent::Data(n) if n.subscription == data
                ) || matches!(
                    event,
                    CommandEvent::Outcome { subscription, .. } if *subscription == outcome
                )),
                "a refusal publishes nothing"
            );
            assert_eq!(all_rows(&db), rows, "rows identical");
            assert!(db.queue_status() == status && db.pending_reconciliations() == pending);
            let (db, _, after_rows) = entity_ledger(db, &clock);
            assert_eq!(after_rows, prior_rows, "ledger rows byte-identical");
            db.close();
            continue;
        }
        // Today's exact images (these pass on unchanged code).
        let pending = |base: &str, time: i64, authoritative: bool| {
            entity_image(
                &id,
                &[("body", base), ("extra", "later")],
                &[("body", time, authoritative), ("extra", 201, false)],
            )
        };
        entity_exact(
            &prior,
            &later_key,
            entity_image(&id, &[("body", "a")], &[("body", 200, false)]),
            pending("a", 200, false),
        );
        assert_eq!(entity_current(&prior, &db, &id), pending("a", 200, false));
        assert_eq!(
            run(db.reconcile(&a, plan)),
            Ok(()),
            "different field: C4 re-bases E"
        );
        let (db, after, _) = entity_ledger(db, &clock);
        entity_exact(
            &after,
            &later_key,
            entity_image(&id, &[("body", "backend")], &[("body", 300, true)]),
            pending("backend", 300, true),
        );
        entity_preserved(&prior, &after, &later_key);
        entity_others(&prior, &after, &[&a, &later_key]);
        assert_eq!(
            entity_current(&after, &db, &id),
            pending("backend", 300, true)
        );
        assert!(after.records[&a].reconciled);
        db.close();
    }
}
