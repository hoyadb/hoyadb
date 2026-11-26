//! DEF-0061 / TC-0051: real post-commit Pending publication retains subscriptions.
#[allow(dead_code)]
#[path = "../../../tests/conformance/work_counters.rs"]
mod cases;
#[allow(dead_code)]
#[path = "support/yielding.rs"]
mod yielding;

use hoyadb_core::{command::*, model::*, storage::*};
use hoyadb_storage_memory::MemoryStorage;
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    task::{Context, Poll, Waker},
};
use yielding::{Adapter, Counts};

fn run<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    for _ in 0..10_000 {
        if let Poll::Ready(value) = future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            return value;
        }
    }
    panic!("yielding adapter did not finish");
}

struct Watch {
    query: QueryBuilder,
    rows: Subscription,
    count: Subscription,
}

fn fixture() -> (CommandDatabase<Adapter<false>>, Counts, Vec<Watch>) {
    let store = cases::owner(MemoryStorage::default(), 2).into_storage();
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
    let queries = [
        db.query("items"),
        db.query("items")
            .where_("body", Operator::Eq, Value::String("patched".into())),
        db.query("items")
            .where_("id", Operator::Eq, Value::String("r0000".into())),
    ];
    let watches = queries
        .into_iter()
        .map(|query| Watch {
            rows: run(db.observe_with_columns(&query, vec!["body".into()])).unwrap(),
            count: run(db.observe_count(&query)).unwrap(),
            query,
        })
        .collect();
    assert_eq!(db.drain_notifications(100).len(), 6);
    assert_live(&db, 3);
    (db, counts, watches)
}

fn body(value: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([("body".into(), Value::String(value.into()))])
}

#[derive(Clone, Copy, Debug)]
enum Suspension {
    Projection,
    Rows,
}

fn cancel_publication(db: &mut CommandDatabase<Adapter<false>>, counts: &Counts, at: Suspension) {
    let commits = counts.delta_commits();
    let projects = counts.projects();
    let mut patch = Box::pin(db.patch("items", "r0000", body("patched")));
    for _ in 0..10_000 {
        assert!(
            patch
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending(),
            "publication must suspend before the mutation returns"
        );
        if counts.delta_commits() == commits + 1 && counts.projects() == projects + 2 {
            // The first group completed; the second group is Pending in its
            // real projection. No third-group read has started.
            if matches!(at, Suspension::Rows) {
                let reads = counts.reads();
                assert!(
                    patch
                        .as_mut()
                        .poll(&mut Context::from_waker(Waker::noop()))
                        .is_pending()
                );
                assert_eq!(counts.projects(), projects + 2);
                assert_eq!(
                    counts.reads(),
                    reads + 1,
                    "projection completed and full-row materialization now suspends"
                );
            }
            return; // Drop an actually Pending future after the real delta commit.
        }
    }
    panic!("did not reach the second group's post-commit read: {at:?}");
}

fn cancel_refresh(db: &mut CommandDatabase<Adapter<false>>, counts: &Counts) {
    let projects = counts.projects();
    let mut refresh = Box::pin(db.refresh_notifications());
    assert!(
        refresh
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    assert_eq!(
        counts.projects(),
        projects + 1,
        "dirty candidate retry actually suspended"
    );
}

fn assert_live(db: &CommandDatabase<Adapter<false>>, groups: usize) {
    assert_eq!(
        db.observation_stats().groups,
        groups,
        "all group owners must survive cancellation"
    );
    assert_eq!(
        db.observation_stats().observers,
        groups * 2,
        "all tokens must survive cancellation"
    );
}

fn assert_rows(db: &CommandDatabase<Adapter<false>>, events: &[Notification], watch: &Watch) {
    let actual: Vec<_> = events
        .iter()
        .filter(|event| event.subscription == watch.rows)
        .collect();
    assert_eq!(
        actual.len(),
        1,
        "exactly one current row recovery for {:?}: {events:?}",
        watch.rows
    );
    assert_eq!(
        actual[0].value,
        Ok(ObservationValue::Rows(run(db.fetch(&watch.query)).unwrap()))
    );
}

fn assert_count(events: &[Notification], watch: &Watch, expected: usize) {
    let actual: Vec<_> = events
        .iter()
        .filter(|event| event.subscription == watch.count)
        .collect();
    assert_eq!(actual.len(), 1, "one current count recovery: {events:?}");
    assert_eq!(actual[0].value, Ok(ObservationValue::Count(expected)));
}

fn assert_no_counts(events: &[Notification]) {
    assert!(
        events
            .iter()
            .all(|event| matches!(event.value, Ok(ObservationValue::Rows(_)))),
        "recovery must preserve the original 116ms count deadline: {events:?}"
    );
}

#[test]
fn cancelled_projection_and_rows_preserve_all_groups_and_refresh_exactly() {
    for at in [Suspension::Projection, Suspension::Rows] {
        let (mut db, counts, watches) = fixture();
        cancel_publication(&mut db, &counts, at);
        assert_eq!(counts.delta_commits(), 1);
        assert_live(&db, 3);
        let completed = db.drain_notifications(101);
        assert_eq!(
            completed.len(),
            1,
            "only the completed first group may publish"
        );
        assert_rows(&db, &completed, &watches[0]);
        cancel_refresh(&mut db, &counts);
        assert_live(&db, 3);
        assert!(db.drain_notifications(102).is_empty());
        run(db.refresh_notifications());
        let recovered = db.drain_notifications(115);
        assert_eq!(recovered.len(), 2);
        assert_no_counts(&recovered);
        for watch in &watches[1..] {
            assert_rows(&db, &recovered, watch);
        }
        let counts = db.drain_notifications(116);
        assert_eq!(counts.len(), 2);
        assert_count(&counts, &watches[1], 1);
        // Same-cardinality recovery is required for the never-visited group.
        assert_count(&counts, &watches[2], 1);
        for watch in watches {
            assert!(db.unsubscribe(watch.rows));
            assert!(db.unsubscribe(watch.count));
        }
        assert_live(&db, 0);
        assert!(db.drain_notifications(200).is_empty());
    }
}

#[test]
fn repeated_cancellation_then_unsubscribe_and_later_commit_recovers_remaining_tokens() {
    let (mut db, counts, watches) = fixture();
    cancel_publication(&mut db, &counts, Suspension::Rows);
    assert_live(&db, 3);
    cancel_refresh(&mut db, &counts);
    cancel_refresh(&mut db, &counts);
    assert_live(&db, 3);
    // Cancel a completed group's queued notification and both its listeners.
    assert!(db.unsubscribe(watches[0].rows));
    assert!(db.unsubscribe(watches[0].count));
    assert_live(&db, 2);
    assert!(db.drain_notifications(101).is_empty());
    run(db.patch("items", "r0001", body("patched"))).unwrap();
    assert_eq!(counts.delta_commits(), 2);
    let rows = db.drain_notifications(115);
    assert_eq!(rows.len(), 2);
    assert_no_counts(&rows);
    for watch in &watches[1..] {
        assert_rows(&db, &rows, watch);
    }
    let events = db.drain_notifications(116);
    assert_eq!(events.len(), 2);
    assert_count(&events, &watches[1], 2);
    assert_count(&events, &watches[2], 1);
    for watch in &watches[1..] {
        assert!(db.unsubscribe(watch.rows));
        assert!(db.unsubscribe(watch.count));
        assert!(!db.unsubscribe(watch.rows));
    }
    assert_live(&db, 0);
}

#[test]
fn cancellation_retains_earlier_queued_snapshots_but_clears_incomplete_pending_counts() {
    let (mut db, counts, watches) = fixture();
    // These initial events predate the cancelled publication and must survive.
    let rows = run(db.observe(&watches[1].query)).unwrap();
    let count = run(db.observe_count(&watches[1].query)).unwrap();
    cancel_publication(&mut db, &counts, Suspension::Rows);
    assert_eq!(db.observation_stats().groups, 3);
    assert_eq!(db.observation_stats().observers, 8);
    let queued = db.drain_notifications(101);
    assert_eq!(queued.len(), 3);
    assert_eq!(
        queued[0],
        Notification {
            subscription: rows,
            value: Ok(ObservationValue::Rows(vec![]))
        }
    );
    assert_eq!(
        queued[1],
        Notification {
            subscription: count,
            value: Ok(ObservationValue::Count(0))
        }
    );
    assert_rows(&db, &queued, &watches[0]);
    // The second group's candidates changed before full-row Pending. Its
    // newly staged pending count is incomplete and cannot drain at 116ms.
    assert!(db.drain_notifications(116).is_empty());
    run(db.refresh_notifications());
    let recovered = db.drain_notifications(117);
    assert_eq!(recovered.len(), 6);
    assert_rows(&db, &recovered, &watches[1]);
    assert_rows(&db, &recovered, &watches[2]);
    assert_count(&recovered, &watches[1], 1);
    assert_count(&recovered, &watches[2], 1);
    let extra = Watch {
        query: watches[1].query.clone(),
        rows,
        count,
    };
    assert_rows(&db, &recovered, &extra);
    assert_count(&recovered, &extra, 1);
    assert!(db.unsubscribe(rows));
    assert!(db.unsubscribe(count));
}
