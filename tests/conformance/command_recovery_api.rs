//! SPEC-0038 / TC-0042 API-surface cases added after GREEN. They use the new Rust symbols
//! (`QueueEntry::awaiting_key`, `CommandDatabase::request_redelivery`), so they cannot be a
//! compile-free RED; their behavioral RED is the facade regression in hoyadb-bindings/tests/recovery.rs.
#[allow(dead_code)]
#[path = "command_recovery.rs"]
mod base;
pub use base::{body, edit, keys, open, run};
use hoyadb_core::command::records::SyncLedger;
use hoyadb_core::{command::*, storage::*};

fn status_entry<S: SyncStorage>(db: &CommandDatabase<S>, key: &str) -> QueueEntry {
    db.queue_status()
        .commands
        .into_iter()
        .find(|e| e.key == key)
        .expect("known key")
}

fn reopen_ledger<S: SyncStorage>(db: CommandDatabase<S>) -> (CommandDatabase<S>, SyncLedger) {
    let storage = db.into_storage();
    let ledger = SyncLedger::from_rows(storage.load_sync().unwrap().as_deref().unwrap()).unwrap();
    (open(storage), ledger)
}

/// RD status: `awaiting_key` names the earliest withholding record and clears once A is Sent.
pub fn awaiting_key<S: SyncStorage>(storage: S) {
    let mut db = open(storage);
    run(db.insert_proposed("notes", body("original"), Some("server-note".into()))).unwrap();
    let a = edit(&mut db, "server-note", "a", 100);
    let b = edit(&mut db, "server-note", "b", 101);
    let c = edit(&mut db, "server-note", "c", 102);
    assert_eq!(status_entry(&db, &a).awaiting_key, None);
    assert_eq!(status_entry(&db, &b).awaiting_key, Some(a.clone()));
    assert_eq!(
        status_entry(&db, &c).awaiting_key,
        Some(a.clone()),
        "earliest blocker"
    );
    db.prepare_push(103, 0).unwrap();
    assert_eq!(
        status_entry(&db, &a).awaiting_key,
        None,
        "Sent is never withheld"
    );
    assert_eq!(status_entry(&db, &b).awaiting_key, Some(a));
    db.close();
}

/// CL04: explicit redelivery in a mixed queue, at a raw sample below the identity floor, with
/// a restart before the resend. Returns the storage and the final status for a reopen check.
pub fn cl04<S: SyncStorage>(storage: S) -> (S, QueueStatus) {
    let mut db = open(storage);
    for (id, value) in [("na", "a0"), ("nb", "b0"), ("nc", "c0"), ("nd", "d0")] {
        run(db.insert_proposed("notes", body(value), Some(id.into()))).unwrap();
    }
    let a = edit(&mut db, "na", "a", 1_000);
    let b = edit(&mut db, "nb", "b", 1_001);
    let c = edit(&mut db, "nc", "c", 1_002);
    let first = db.prepare_push(5_000, 0).unwrap();
    assert_eq!(keys(&first), vec![a.clone(), b.clone(), c.clone()]);
    db.receive(&a, None).unwrap();
    db.receive(&c, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    let withheld = edit(&mut db, "na", "behind a", 5_001);
    let d = edit(&mut db, "nd", "d", 5_002);
    let (mut db, before) = reopen_ledger(db);
    let floor = before.header.last_ms;
    let r = 4_000;
    assert!(r < floor, "sample {r} below the identity floor {floor}");
    assert!(db.request_redelivery(&a, r).is_ok());
    let (mut db, after) = reopen_ledger(db);
    let (old, new) = (&before.records[&a], &after.records[&a]);
    assert_eq!(new.state, SyncState::Sent);
    assert_eq!(
        new.due_ms, r,
        "due at the raw sample, not the identity floor"
    );
    assert_eq!(
        (&new.frozen, new.attempts, &new.key),
        (&old.frozen, old.attempts, &old.key)
    );
    assert_eq!(after.header.last_ms, floor, "identity floor unchanged");
    assert!(db.request_redelivery(&b, r).is_ok(), "Sent is a no-op");
    let (mut db, unchanged) = reopen_ledger(db);
    assert!(
        unchanged == after,
        "no ledger change for an already Sent record"
    );
    for key in [c.as_str(), withheld.as_str(), "unknown"] {
        assert!(
            db.request_redelivery(key, r) == Err(CommandError::Protocol),
            "{key}: only Acknowledged-without-outcome or Sent are redeliverable"
        );
    }
    assert!(db.request_redelivery(&a, -1) == Err(CommandError::InvalidCommand));
    let (mut db, _) = reopen_ledger(db);
    let resent = db.prepare_push(r, 0).unwrap();
    assert_eq!(
        keys(&resent),
        vec![a.clone(), d.clone()],
        "exactly A and the unrelated D; not B, C or the withheld same-row record"
    );
    let original = &first.envelopes[0];
    assert_eq!(&resent.envelopes[0], original, "identical frozen bytes");
    db.receive(&a, None).unwrap();
    assert_eq!(status_entry(&db, &a).state, SyncState::Acknowledged);
    db.receive(&a, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    assert_eq!(status_entry(&db, &withheld).awaiting_key, Some(a.clone()));
    run(db.reconcile(&a, ReconcilePlan::default())).unwrap();
    assert_eq!(status_entry(&db, &withheld).awaiting_key, None);
    assert_eq!(keys(&db.prepare_push(r + 1, 0).unwrap()), vec![withheld]);
    let status = db.queue_status();
    (db.into_storage(), status)
}
