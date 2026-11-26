//! TC-0041 SI-Y01: dropping real Pending operations before/after delta commit.
#[allow(dead_code)]
#[path = "../../../tests/conformance/work_counters.rs"]
mod cases;
#[allow(dead_code)]
#[path = "support/yielding.rs"]
mod yielding;

use hoyadb_core::{command::*, storage::*};
use hoyadb_storage_memory::MemoryStorage;
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    task::{Context, Poll, Waker},
};
use yielding::{Adapter, Counts};

fn run<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    // Every test-adapter read wakes and yields exactly once. This driver needs
    // no timers, threads or runtime and fails closed if progress stops.
    for _ in 0..10_000 {
        if let Poll::Ready(value) = future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            return value;
        }
    }
    panic!("yielding test adapter did not finish");
}

fn raw(store: &MemoryStorage) -> BTreeMap<String, Vec<Row>> {
    ["items", MODEL_TABLE, SYNC_TABLE]
        .into_iter()
        .map(|table| (table.to_owned(), run(store.scan(table)).unwrap()))
        .collect()
}

fn fixture() -> (
    CommandDatabase<Adapter<false>>,
    Counts,
    BTreeMap<String, Vec<Row>>,
) {
    let store = cases::owner(MemoryStorage::default(), 2).into_storage();
    let before = raw(&store);
    let schema = model_definition(&run(store.catalog()).unwrap().state.unwrap()).unwrap();
    let counts = Counts::default();
    let mut db = run(CommandDatabase::open(
        Adapter {
            inner: store,
            counts: counts.clone(),
            pending_id: "",
        },
        schema,
        vec![],
        [0x5d; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    db.configure_entities(
        EntityConfig {
            scope: "counters".into(),
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
    assert_eq!(counts.delta_commits(), 0, "rebind commits no new ledger");
    (db, counts, before)
}

fn body(value: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([("body".into(), Value::String(value.into()))])
}

fn cancel_during_publication(db: &mut CommandDatabase<Adapter<false>>, counts: &Counts) {
    let commits = counts.delta_commits();
    let projects = counts.projects();
    let mut patch = Box::pin(db.patch("items", "r0000", body("patched")));
    for _ in 0..10_000 {
        assert!(
            patch
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending(),
            "notification projection must suspend before patch returns"
        );
        if counts.delta_commits() == commits + 1 && counts.projects() > projects {
            return; // Drop the still-Pending future, after the real commit.
        }
    }
    panic!("genuine delta did not commit then suspend in projection");
}

#[test]
fn si_y01_pending_before_persist_cancellation_preserves_owner_and_store() {
    let (mut db, counts, before) = fixture();
    let ledger = db.ledger_work();
    let status = db.queue_status();
    let progress = db.sync_progress();
    {
        let mut patch = Box::pin(db.patch("items", "r0000", body("cancelled")));
        assert!(
            patch
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        assert_eq!(
            counts.delta_commits(),
            0,
            "read suspends before actual storage apply"
        );
    }
    assert_eq!(db.ledger_work(), ledger);
    assert_eq!(db.queue_status(), status);
    assert_eq!(db.sync_progress(), progress);
    let store = db.into_storage().inner;
    assert_eq!(
        raw(&store),
        before,
        "model, application and ledger bytes unchanged"
    );

    // A separate live-owner control proves cancellation releases admission.
    let (mut db, _, _) = fixture();
    {
        let mut patch = Box::pin(db.patch("items", "r0000", body("cancelled")));
        assert!(
            patch
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
    }
    run(db.patch("items", "r0000", body("retry"))).unwrap();
    assert_eq!(db.ledger_work().epoch, 1);
    assert_eq!(db.queue_status().pending, 1);
}

#[test]
fn si_y01_pending_after_commit_cancellation_keeps_owner_equal_to_storage() {
    let (mut db, counts, before) = fixture();
    let initial = db.ledger_work();
    let query = db
        .query("items")
        .where_("body", Operator::Eq, Value::String("patched".into()));
    run(db.observe_count(&query)).unwrap();
    db.drain_events(0);
    cancel_during_publication(&mut db, &counts);
    let committed = db.ledger_work();
    assert_eq!(committed.epoch, initial.epoch + 1);
    assert_eq!(
        committed.cached_tombstones,
        Some(db.ledger_tombstone_count())
    );
    assert_eq!(
        committed.metadata_keys,
        BTreeSet::from([("items".into(), "r0000".into())])
    );
    assert_eq!(db.queue_status().pending, 1);
    let rows = run(db.fetch(&db.query("items"))).unwrap();
    assert_eq!(
        rows.iter().find(|r| r.id == "r0000").unwrap().fields["body"],
        Value::String("patched".into())
    );
    let store = db.into_storage().inner;
    assert_ne!(raw(&store), before);
    let (mut control, _, _) = fixture();
    run(control.patch("items", "r0000", body("patched"))).unwrap();
    assert_eq!(
        raw(&store),
        raw(&control.into_storage().inner),
        "cancelled publication retains exactly the ordinary successful commit"
    );

    // No notification-delivery promise is made after cancellation. A second
    // mutation on a cancelled-publication owner must use the committed base.
    let (mut db, counts, _) = fixture();
    run(db.observe_count(&query)).unwrap();
    cancel_during_publication(&mut db, &counts);
    run(db.patch("items", "r0000", body("after cancellation"))).unwrap();
    assert_eq!(db.ledger_work().epoch, 2);
    assert_eq!(db.queue_status().pending, 2);
    let store = db.into_storage().inner;
    let schema = model_definition(&run(store.catalog()).unwrap().state.unwrap()).unwrap();
    let reopened = run(CommandDatabase::open(
        store,
        schema,
        vec![],
        [0x5d; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    assert_eq!(reopened.queue_status().pending, 2);
}
