//! SPEC-0039 / TC-0043: cross-row corrections require exact caller expectations.
//! GW01 and GW02 drive only the existing `reconcile`, so unchanged code compiles and fails by
//! assertion (it overwrites or removes the unrelated row and returns Ok).
#[allow(dead_code)]
#[path = "command_recovery.rs"]
mod base;
pub use base::{body, edit, keys, open, run};
use hoyadb_core::command::records::SyncLedger;
use hoyadb_core::{command::*, storage::*};
use std::collections::BTreeMap;

pub fn seeded<S: SyncStorage>(storage: S) -> CommandDatabase<S> {
    let mut db = open(storage);
    run(db.insert_proposed("notes", body("original"), Some("server-note".into()))).unwrap();
    run(db.insert_proposed("notes", body("other"), Some("other-note".into()))).unwrap();
    db
}

pub fn rows<S: SyncStorage>(db: &CommandDatabase<S>) -> Vec<Row> {
    let mut rows = run(db.query("notes").fetch(db)).unwrap();
    rows.extend(run(db.query("links").fetch(db)).unwrap());
    rows
}

/// Everything a failed reconciliation must preserve: rows, exact ledger rows, queue status
/// and pending outcomes. Round-trips the storage so the ledger is read from the adapter.
pub struct State {
    pub rows: Vec<Row>,
    pub ledger: Option<Vec<Row>>,
    pub status: QueueStatus,
    pub pending: Vec<(String, RemoteOutcome)>,
}

pub fn capture<S: SyncStorage>(db: CommandDatabase<S>) -> (CommandDatabase<S>, State) {
    let (rows, status, pending) = (rows(&db), db.queue_status(), db.pending_reconciliations());
    let storage = db.into_storage();
    let ledger = storage.load_sync().unwrap();
    SyncLedger::from_rows(ledger.as_deref().unwrap()).unwrap();
    (
        open(storage),
        State {
            rows,
            ledger,
            status,
            pending,
        },
    )
}

#[track_caller]
pub fn assert_unchanged<S: SyncStorage>(
    db: CommandDatabase<S>,
    before: &State,
) -> CommandDatabase<S> {
    let (db, after) = capture(db);
    assert_eq!(after.rows, before.rows, "rows unchanged");
    assert_eq!(after.ledger, before.ledger, "ledger rows unchanged");
    assert!(after.status == before.status, "queue status unchanged");
    assert!(after.pending == before.pending, "outcome still pending");
    db
}

fn put_other(value: &str) -> ReconcilePlan {
    ReconcilePlan {
        mappings: BTreeMap::new(),
        writes: vec![ReconcileWrite::Put {
            table: "notes".into(),
            id: "other-note".into(),
            fields: body(value),
        }],
    }
}

/// GW01: an unguarded Put on unrelated existing s cannot overwrite a later direct edit.
/// Unchanged code returns Ok and replaces s (the DEF-0038 diagnostic; TC-0042 CTRL-K).
pub fn gw01<S: SyncStorage>(storage: S) {
    let mut db = seeded(storage);
    let a = edit(&mut db, "server-note", "a", 100);
    run(db.replace("notes", "other-note", body("direct"))).unwrap();
    db.prepare_push(101, 0).unwrap();
    db.receive(&a, Some(RemoteOutcome::Corrected(Json::Null)))
        .unwrap();
    let (mut db, before) = capture(db);
    let query = db.query("notes");
    let _data = run(db.observe(&query)).unwrap();
    let _outcome = db.observe_outcome(&a).unwrap();
    db.drain_events(0);
    assert_eq!(
        run(db.reconcile(&a, put_other("backend"))),
        Err(CommandError::RollbackConflict),
        "unguarded outside-closure Put"
    );
    assert!(
        db.drain_events(0).is_empty(),
        "a refused plan publishes nothing"
    );
    let db = assert_unchanged(db, &before);
    db.close();
}

/// GW02: an unguarded Remove of s edited by another live command E.
pub fn gw02<S: SyncStorage>(storage: S) {
    let mut db = seeded(storage);
    let a = edit(&mut db, "server-note", "a", 100);
    let e = edit(&mut db, "other-note", "e", 101);
    assert_eq!(
        keys(&db.prepare_push(102, 0).unwrap()),
        vec![a.clone(), e.clone()]
    );
    db.receive(&a, Some(RemoteOutcome::Corrected(Json::Null)))
        .unwrap();
    let (mut db, before) = capture(db);
    let plan = ReconcilePlan {
        mappings: BTreeMap::new(),
        writes: vec![ReconcileWrite::Remove {
            table: "notes".into(),
            id: "other-note".into(),
        }],
    };
    assert_eq!(
        run(db.reconcile(&a, plan)),
        Err(CommandError::RollbackConflict),
        "unguarded outside-closure Remove of another live command's row"
    );
    let db = assert_unchanged(db, &before);
    db.close();
}
