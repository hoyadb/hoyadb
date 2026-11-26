//! TC-0041 SI-O05b: an Unproven owner keeps full-snapshot merge staleness,
//! because another writer may edit any row. One test: Unproven owners share the
//! process-wide coordinator.
#[allow(dead_code)]
#[path = "../../../tests/conformance/entity_scaling.rs"]
mod cases;
#[path = "support/shared.rs"]
mod shared;
use hoyadb_core::{
    command::{CommandDatabase, CommandError, MergeSession, MergeStep, PullPage},
    storage::{Storage, StoreOwnership, SyncStorage, Value},
};
use shared::Shared;
use std::collections::BTreeSet;

fn prepare(db: &mut CommandDatabase<Shared>) -> (MergeSession, u64) {
    let request = db.begin_pull().unwrap().request;
    let MergeStep::Offer {
        session,
        step,
        ..
    } = cases::run(db.merge_prepare(
        request,
        PullPage {
            checkpoint: "cp2".into(),
            complete: true,
            changes: vec![cases::change("merged", 20)],
        },
        BTreeSet::from(["items".into()]),
    ))
    .unwrap()
    else {
        panic!("expected an offer")
    };
    (session, step)
}

/// Another writer edits row `id` of `local`, which the page does not touch.
fn edit_elsewhere(handle: &Shared, id: &str) {
    let mut writer = handle.clone();
    let mut row = cases::run(writer.get("local", id)).unwrap().unwrap();
    row.fields
        .insert("note".into(), Value::String("edited elsewhere".into()));
    cases::run(writer.replace("local", row)).unwrap();
}

#[test]
fn si_o05b_unproven_merge_retains_full_snapshot_staleness() {
    // Between prepare and commit: the commit is stale and changes nothing.
    let handle = Shared::default();
    assert_eq!(handle.ownership(), StoreOwnership::Unproven);
    let (mut db, filler) = cases::configured(handle.clone(), 3);
    let observed = db.query("items");
    let subscription = cases::run(db.observe(&observed)).unwrap();
    db.drain_events(0);
    let (session, step) = prepare(&mut db);
    let default = db.merge_input(session, step).unwrap().default.row;
    assert!(matches!(
        cases::run(db.merge_reply(session, step, default)).unwrap(),
        MergeStep::Ready { .. }
    ));
    let ledger = handle.load_sync().unwrap();
    let items = cases::run(db.fetch(&observed)).unwrap();
    edit_elsewhere(&handle, &filler[0]);
    assert_eq!(
        cases::run(db.merge_commit(session)),
        Err(CommandError::StalePull)
    );
    assert_eq!(
        handle.load_sync().unwrap(),
        ledger,
        "no ledger or metadata change"
    );
    assert_eq!(
        cases::run(db.fetch(&observed)).unwrap(),
        items,
        "no row change"
    );
    assert!(
        db.drain_events(0)
            .iter()
            .all(|event| !matches!(event, hoyadb_core::command::CommandEvent::Data(n) if n.subscription == subscription)),
        "no notification"
    );
    // Between prepare and the host reply: the reply is stale.
    let handle = Shared::default();
    let (mut db, filler) = cases::configured(handle.clone(), 3);
    let (session, step) = prepare(&mut db);
    edit_elsewhere(&handle, &filler[0]);
    let default = db.merge_input(session, step).unwrap().default.row;
    assert_eq!(
        cases::run(db.merge_reply(session, step, default)),
        Err(CommandError::StalePull)
    );
}
