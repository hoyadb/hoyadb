//! TC-0029 R01/R03: public integration RED before observer implementation.
use hoyadb_core::{model::*, storage::*};
use hoyadb_storage_memory::MemoryStorage;
use std::{
    collections::BTreeMap,
    future::Future,
    task::{Context, Poll, Waker},
};

fn run<F: Future>(f: F) -> F::Output {
    match std::pin::pin!(f).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("unexpected pending"),
    }
}

#[test]
fn public_query_observation_tracks_only_committed_membership() {
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
    let query = db
        .query("orders")
        .where_("status", Operator::Eq, Value::String("pending".into()));
    let token = run(query.observe(&mut db)).unwrap();
    assert_eq!(
        db.drain_notifications(0),
        vec![Notification {
            subscription: token,
            value: Ok(ObservationValue::Rows(vec![]))
        }]
    );
    let fields = BTreeMap::from([("status".into(), Value::String("pending".into()))]);
    let row = run(db.insert("orders", fields.clone())).unwrap();
    assert_eq!(
        db.drain_notifications(1),
        vec![Notification {
            subscription: token,
            value: Ok(ObservationValue::Rows(vec![row.clone()]))
        }]
    );
    run(db.replace("orders", &row.id, fields)).unwrap();
    assert!(db.drain_notifications(2).is_empty());
    assert!(
        run(db.apply_batch(vec![
            Write::Remove {
                table: "orders".into(),
                id: row.id.clone()
            },
            Write::Insert {
                table: "orders".into(),
                fields: BTreeMap::new()
            }
        ]))
        .is_err()
    );
    assert!(db.drain_notifications(3).is_empty());
    assert_eq!(run(query.fetch(&db)).unwrap(), vec![row]);
    assert!(db.unsubscribe(token));
    assert_eq!(db.observation_stats().groups, 0);
}
