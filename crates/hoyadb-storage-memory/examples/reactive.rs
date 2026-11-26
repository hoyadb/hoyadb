//! API-0005 / SPEC-0027: owned local events and an explicit host drain clock.
use hoyadb_core::{
    model::*,
    storage::{Column, ColumnType, Operator, Value},
};
use hoyadb_storage_memory::MemoryStorage;
use std::{
    collections::BTreeMap,
    future::Future,
    task::{Context, Poll, Waker},
};

fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("memory operation unexpectedly suspended"),
    }
}

fn main() {
    let schema = SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([(
            "orders".into(),
            TableDefinition {
                columns: BTreeMap::from([(
                    "status".into(),
                    Column {
                        kind: ColumnType::String,
                        optional: false,
                    },
                )]),
                relations: BTreeMap::new(),
            },
        )]),
    };
    let mut db = run(Database::open(MemoryStorage::default(), schema, vec![]))
        .unwrap()
        .into_reactive();
    let pending =
        db.query("orders")
            .where_("status", Operator::Eq, Value::String("pending".into()));
    let rows = run(pending.observe(&mut db)).unwrap();
    let count = run(pending.observe_count(&mut db)).unwrap();
    assert_eq!(db.observation_stats().groups, 1);
    let initial = db.drain_notifications(0);
    assert_eq!(initial.len(), 2);
    let order = run(db.insert(
        "orders",
        BTreeMap::from([("status".into(), Value::String("pending".into()))]),
    ))
    .unwrap();
    let events = db.drain_notifications(1);
    assert_eq!(
        events,
        vec![Notification {
            subscription: rows,
            value: Ok(ObservationValue::Rows(vec![order.clone()]))
        }]
    );
    assert_eq!(run(pending.fetch(&db)).unwrap(), vec![order]);
    // The host schedules a later drain; core owns no timer or callback runtime.
    assert_eq!(
        db.drain_notifications(16),
        vec![Notification {
            subscription: count,
            value: Ok(ObservationValue::Count(1))
        }]
    );
    assert!(db.unsubscribe(rows));
    assert!(db.unsubscribe(count));
    assert_eq!(db.observation_stats().groups, 0);
    println!("Phase 3 example: owned row snapshot, throttled count, shared cache released");
}
