//! TC-0041 SI-O06-SI-O08: Exclusive adapters whose reads may suspend.
// The script, opener and driver are shared; the bounded-path cases run elsewhere.
#[allow(dead_code)]
#[path = "../../../tests/conformance/overlay.rs"]
mod cases;
// This adapter is also used by ledger_cancellation_cases, which consumes the
// additional commit/projection observations.
#[allow(dead_code)]
#[path = "support/yielding.rs"]
mod yielding;
use hoyadb_core::{
    command::{CommandError, Json},
    model::Write,
    storage::{Direction, Operator, StorageError, SyncStorage, Value, counters},
};
use hoyadb_storage_memory::MemoryStorage;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    task::{Context, Waker},
};
use yielding::{Adapter, Counts};

fn country(value: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([("country".into(), Value::String(value.into()))])
}

fn product(name: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([("name".into(), Value::String(name.into()))])
}

/// SI-O06: the full SI-O02 script, then begin's full copy and validation, and
/// scope reads that never reach the live store.
#[test]
fn si_o06_yielding_exclusive_adapter_keeps_awaited_snapshot_staging() {
    let lines = cases::handler_script(Adapter::<false>::new(Counts::default(), ""));
    cases::assert_pinned("memory", &lines);
    let counts = Counts::default();
    let mut db = cases::open(Adapter::<false>::new(counts.clone(), ""));
    cases::seed(&mut db, 0);
    counters::reset();
    let request = cases::begin(&mut db);
    let work = counters::snapshot();
    // 3 customers, 6 orders, 2 products, 1 item and 1 product link.
    assert_eq!((work.rows_cloned, work.inventory_rows_validated), (13, 13));
    let reads = counts.reads();
    let top = db
        .query("orders")
        .order_by("total_cents", Direction::Desc)
        .limit(3);
    assert_eq!(cases::run(db.scope_fetch(request, &top)).unwrap().len(), 3);
    cases::run(db.scope_replace(request, "customers", "c2", country("FR"))).unwrap();
    assert_eq!(
        counts.reads(),
        reads,
        "scope base reads use only the private snapshot"
    );
    cases::run(db.finish(request, Json::Null)).unwrap();
}

/// SI-O07: dropping begin while its snapshot read is Pending releases
/// admission and changes nothing.
#[test]
fn si_o07_cancellation_during_snapshot_acquisition_releases_admission() {
    let mut db = cases::open(Adapter::<false>::new(Counts::default(), ""));
    cases::seed(&mut db, 0);
    let tables = [
        "customers",
        "orders",
        "products",
        "order_items",
        "order_products",
    ];
    let state = |db: &hoyadb_core::command::CommandDatabase<Adapter<false>>| {
        let rows: Vec<_> = tables
            .iter()
            .map(|table| cases::run(db.fetch(&db.query(table))).unwrap())
            .collect();
        (rows, db.queue_status().pending, db.sync_progress())
    };
    let before = state(&db);
    let request = db.request().unwrap();
    {
        let counter = Arc::new(cases::CountingWaker::default());
        let waker = Waker::from(counter);
        let mut context = Context::from_waker(&waker);
        let mut begin = Box::pin(db.begin(
            request,
            cases::definition(),
            Json::Object(BTreeMap::new()),
            Some(Json::Null),
            1000,
        ));
        assert!(
            begin.as_mut().poll(&mut context).is_pending(),
            "snapshot acquisition suspends on the yielding store"
        );
    }
    assert_eq!(
        cases::run(db.scope_fetch(request, &db.query("orders"))),
        Err(CommandError::InvalidTicket),
        "no active scope remains"
    );
    assert_eq!(state(&db), before, "rows, model and ledger are unchanged");
    let next = cases::begin(&mut db);
    cases::run(db.scope_replace(next, "customers", "c2", country("FR"))).unwrap();
    cases::run(db.finish(next, Json::Null)).unwrap();
    let c2 = db
        .query("customers")
        .where_("id", Operator::Eq, Value::String("c2".into()));
    assert_eq!(
        cases::run(db.fetch(&c2)).unwrap()[0].fields["country"],
        Value::String("FR".into())
    );
}

fn backend<T>(result: Result<T, CommandError>) -> bool {
    matches!(
        result,
        Err(CommandError::Storage(StorageError::BackendFailure { .. }))
    )
}

/// SI-O08: a false readiness promise fails once, without retry, and leaves no
/// partial staging; the scope stays usable and commits exactly its later work.
#[test]
fn si_o08_false_readiness_promise_fails_without_partial_staging() {
    let counts = Counts::default();
    let mut db = cases::open(Adapter::<true>::new(counts.clone(), "local:pend"));
    cases::seed(&mut db, 0);
    let request = cases::begin(&mut db);
    let polls = counts.pending_polls();
    assert!(backend(cases::run(db.scope_insert_proposed(
        request,
        "products",
        product("pending"),
        Some("local:pend".into())
    ))));
    assert_eq!(
        counts.pending_polls(),
        polls + 1,
        "polled once, never retried"
    );
    let staged = cases::run(db.scope_apply_batch_proposed(
        request,
        vec![
            (
                Write::Insert {
                    table: "products".into(),
                    fields: product("earlier"),
                },
                Some("local:a".into()),
            ),
            (
                Write::Insert {
                    table: "products".into(),
                    fields: product("pending"),
                },
                Some("local:pend".into()),
            ),
        ],
    ));
    assert!(backend(staged));
    assert_eq!(
        counts.pending_polls(),
        polls + 2,
        "polled once, never retried"
    );
    let earlier = db
        .query("products")
        .where_("id", Operator::Eq, Value::String("local:a".into()));
    assert!(
        cases::run(db.scope_fetch(request, &earlier))
            .unwrap()
            .is_empty(),
        "the batch's earlier primitive is restored"
    );
    cases::run(db.scope_insert_proposed(
        request,
        "products",
        product("later"),
        Some("local:b".into()),
    ))
    .unwrap();
    cases::run(db.finish(request, Json::Null)).unwrap();
    let stored: BTreeSet<String> = cases::run(db.fetch(&db.query("products")))
        .unwrap()
        .into_iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(
        stored,
        BTreeSet::from(["local:b".into(), "p1".into(), "p2".into()])
    );
}

/// SI-O08: readiness is a capability of known ready engines only.
#[test]
fn si_o08_readiness_defaults_false_for_unknown_adapters() {
    assert!(!Adapter::<false>::new(Counts::default(), "").reads_ready());
    assert!(Adapter::<true>::new(Counts::default(), "").reads_ready());
    assert!(
        MemoryStorage::default().reads_ready(),
        "SnapshotStorage reads complete immediately"
    );
}
