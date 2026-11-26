//! API-0004 / TC-0026: runnable local schema, relation and migration example.
use hoyadb_core::{
    model::*,
    storage::{Column, ColumnType, Direction, Operator, Value},
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

fn column(kind: ColumnType) -> Column {
    Column {
        kind,
        optional: false,
    }
}

fn main() {
    let schema = SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([
            (
                "customers".into(),
                TableDefinition {
                    columns: BTreeMap::from([("country".into(), column(ColumnType::String))]),
                    relations: BTreeMap::from([(
                        "orders".into(),
                        RelationDefinition::HasMany {
                            table: "orders".into(),
                            foreign_key: "customer_id".into(),
                        },
                    )]),
                },
            ),
            (
                "orders".into(),
                TableDefinition {
                    columns: BTreeMap::from([("total_cents".into(), column(ColumnType::Integer))]),
                    relations: BTreeMap::from([(
                        "customer".into(),
                        RelationDefinition::ToOne {
                            table: "customers".into(),
                            foreign_key: "customer_id".into(),
                            optional: false,
                        },
                    )]),
                },
            ),
        ]),
    };
    let mut db = run(Database::open(
        MemoryStorage::default(),
        schema.clone(),
        vec![],
    ))
    .unwrap();
    let customer = run(db.insert(
        "customers",
        BTreeMap::from([("country".into(), Value::String("FR".into()))]),
    ))
    .unwrap();
    let order = run(db.insert(
        "orders",
        BTreeMap::from([
            ("total_cents".into(), Value::Integer(4200)),
            ("customer_id".into(), Value::Reference(customer.id.clone())),
        ]),
    ))
    .unwrap();
    let related = db.related("customers", &customer.id, "orders");
    assert_eq!(run(related.fetch(&db)).unwrap(), vec![order]);
    let mut next = schema;
    next.version = 2;
    next.tables
        .get_mut("orders")
        .unwrap()
        .columns
        .insert("archived".into(), column(ColumnType::Boolean));
    let migration = Migration {
        from_version: 1,
        to: next.clone(),
        steps: vec![MigrationStep::AddColumn {
            table: "orders".into(),
            name: "archived".into(),
            column: column(ColumnType::Boolean),
            default: Some(Value::Boolean(false)),
        }],
    };
    let db = run(Database::open(db.into_storage(), next, vec![migration])).unwrap();
    let orders = run(db
        .query("orders")
        .where_("customer.country", Operator::Eq, Value::String("FR".into()))
        .where_("total_cents", Operator::Ge, Value::Integer(1000))
        .order_by("total_cents", Direction::Desc)
        .limit(20)
        .fetch(&db))
    .unwrap();
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].fields["archived"], Value::Boolean(false));
    println!(
        "Phase 2 example: {} order; schema version {}",
        orders.len(),
        db.schema().version
    );
}
