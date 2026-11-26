//! Separate-process TC-0022 Q01/Q02 fixture driver; no application executable.
use hoyadb_core::storage::{
    Column, ColumnType, FiniteFloat, Mutation, Query, Row, Schema, Storage, StorageError,
    TableSchema, TransactionalStorage, TypedStorage, TypedTransactionalStorage, Value,
};
use hoyadb_storage_sqlite::SqliteStorage;
use std::{
    collections::BTreeMap,
    future::Future,
    task::{Context, Poll, Waker},
};

fn run<F: Future + Send>(future: F) -> F::Output {
    let waker = Waker::noop();
    match std::pin::pin!(future).poll(&mut Context::from_waker(waker)) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("operation must resolve immediately"),
    }
}

fn row(id: &str, number: i64, paid: bool) -> Row {
    Row {
        id: id.into(),
        fields: BTreeMap::from([
            ("text".into(), Value::String("保留 🌿\0exact".into())),
            ("number".into(), Value::Integer(number)),
            ("paid".into(), Value::Boolean(paid)),
        ]),
    }
}

#[path = "../../../../tests/conformance/model_restart.rs"]
mod model_restart;
fn run_model<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("synchronous model fixture suspended"),
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 4);
    let mut store = SqliteStorage::open_unprotected(&args[3]).unwrap();
    // TC-0041 SI-L03: own the store, report the barrier, then wait to be killed.
    if args[1] == "hold" {
        use std::io::{Read, Write};
        run(store.create_table("notes")).unwrap();
        let held = Row {
            id: "held".into(),
            fields: BTreeMap::from([(
                "text".into(),
                Value::String("synthetic-ownership-canary-東京-🌿".into()),
            )]),
        };
        run(store.insert("notes", held)).unwrap();
        let mut stdout = std::io::stdout();
        stdout.write_all(b"held\n").unwrap();
        stdout.flush().unwrap();
        std::io::stdin().read_to_end(&mut Vec::new()).unwrap();
        return;
    }
    if args[2] == "model" {
        match args[1].as_str() {
            "write" => run_model(model_restart::write(store)).unwrap(),
            "read" => {
                run_model(model_restart::read(store, true)).unwrap();
            }
            _ => panic!("unknown model fixture mode"),
        }
        return;
    }
    if args[2] == "typed" {
        typed_restart(&mut store, &args[1]);
        return;
    }
    let seed = row("seed", i64::MIN, false);
    let keeper = row("keeper", i64::MAX, true);
    let replacement = Row {
        id: "seed".into(),
        fields: BTreeMap::from([("number".into(), Value::Integer(i64::MAX))]),
    };
    match (args[1].as_str(), args[2].as_str()) {
        ("write", scenario) => {
            run(store.create_table("orders")).unwrap();
            run(store.create_table("audit")).unwrap();
            run(store.insert("orders", seed.clone())).unwrap();
            run(store.insert("audit", keeper.clone())).unwrap();
            run(store.insert("orders", row("removed", 1, true))).unwrap();
            let mut batch = vec![
                Mutation::Replace {
                    table: "orders".into(),
                    row: replacement.clone(),
                },
                Mutation::Insert {
                    table: "orders".into(),
                    row: row("added", i64::MIN, false),
                },
                Mutation::Remove {
                    table: "orders".into(),
                    id: "removed".into(),
                },
            ];
            if scenario == "rollback" {
                batch.push(Mutation::Insert {
                    table: "audit".into(),
                    row: keeper,
                });
                batch.push(Mutation::Insert {
                    table: "audit".into(),
                    row: row("later", 0, false),
                });
                assert_eq!(
                    run(store.apply_batch(batch)),
                    Err(StorageError::RowAlreadyExists {
                        table: "audit".into(),
                        id: "keeper".into()
                    })
                );
            } else {
                assert_eq!(scenario, "commit");
                run(store.apply_batch(batch)).unwrap();
            }
        }
        ("read", "commit") => {
            assert_eq!(run(store.get("orders", "seed")), Ok(Some(replacement)));
            assert_eq!(
                run(store.get("orders", "added")),
                Ok(Some(row("added", i64::MIN, false)))
            );
            assert_eq!(run(store.get("orders", "removed")), Ok(None));
            assert_eq!(run(store.get("audit", "keeper")), Ok(Some(keeper)));
        }
        ("read", "rollback") => {
            assert_eq!(run(store.get("orders", "seed")), Ok(Some(seed)));
            assert_eq!(run(store.get("audit", "keeper")), Ok(Some(keeper)));
            assert_eq!(
                run(store.get("orders", "removed")),
                Ok(Some(row("removed", 1, true)))
            );
            assert_eq!(run(store.get("orders", "added")), Ok(None));
            assert_eq!(run(store.get("audit", "later")), Ok(None));
        }
        _ => panic!("unknown fixture mode"),
    }
}

// TC-0025 P11: the writer and reader are separate OS processes.
fn typed_restart(store: &mut SqliteStorage, mode: &str) {
    let schema = Schema {
        tables: BTreeMap::from([(
            "typed".into(),
            TableSchema {
                columns: [
                    ("float", ColumnType::Float),
                    ("integral", ColumnType::Float),
                    ("time", ColumnType::Timestamp),
                    ("ref", ColumnType::Reference),
                    ("nullable", ColumnType::String),
                ]
                .into_iter()
                .map(|(name, kind)| {
                    (
                        name.into(),
                        Column {
                            kind,
                            optional: true,
                        },
                    )
                })
                .collect(),
                relations: BTreeMap::new(),
            },
        )]),
    };
    let expected = Row {
        id: "new-types".into(),
        fields: BTreeMap::from([
            (
                "float".into(),
                Value::Float(FiniteFloat::new(f64::from_bits(1)).unwrap()),
            ),
            (
                "integral".into(),
                Value::Float(FiniteFloat::new(42.0).unwrap()),
            ),
            ("time".into(), Value::Timestamp(i64::MIN)),
            ("ref".into(), Value::Reference("保留\0exact".into())),
            ("nullable".into(), Value::Null),
        ]),
    };
    match mode {
        "write" => {
            run(store.create_table("typed")).unwrap();
            let changes = run(store.apply_typed_batch(
                &schema,
                vec![Mutation::Insert {
                    table: "typed".into(),
                    row: expected.clone(),
                }],
            ))
            .unwrap();
            assert_eq!(changes.len(), 1);
            assert_eq!(changes[0].before, None);
            assert_eq!(changes[0].after, Some(expected));
        }
        "read" => {
            let query = Query {
                table: "typed".into(),
                predicates: vec![],
                order: None,
                limit: None,
            };
            assert_eq!(run(store.query(&schema, &query)), Ok(vec![expected]));
        }
        _ => panic!("unknown fixture mode"),
    }
}
