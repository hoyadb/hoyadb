//! TC-0025 P12: bounded, repeatable measurements, without a latency budget.
use hoyadb_core::storage::*;
use hoyadb_storage_memory::MemoryStorage;
use std::collections::BTreeMap;
use std::future::Future;
use std::task::{Context, Poll, Waker};
use std::time::Instant;

fn ready<T>(future: impl Future<Output = T>) -> T {
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("memory operation unexpectedly suspended"),
    }
}

fn row(id: usize, number: i64) -> Row {
    Row {
        id: format!("{id:08}"),
        fields: BTreeMap::from([
            ("number".into(), Value::Integer(number)),
            ("text".into(), Value::String(format!("key-{number:08}"))),
        ]),
    }
}

fn schema() -> Schema {
    Schema {
        tables: BTreeMap::from([(
            "items".into(),
            TableSchema {
                columns: BTreeMap::from([
                    (
                        "number".into(),
                        Column {
                            kind: ColumnType::Integer,
                            optional: false,
                        },
                    ),
                    (
                        "text".into(),
                        Column {
                            kind: ColumnType::String,
                            optional: false,
                        },
                    ),
                ]),
                relations: BTreeMap::new(),
            },
        )]),
    }
}

fn query(field: &str, value: Value) -> Query {
    Query {
        table: "items".into(),
        predicates: vec![Predicate {
            field: field.into(),
            operator: Operator::Eq,
            values: vec![value],
        }],
        order: None,
        limit: None,
    }
}

#[test]
fn p12_selective_lookup_survives_growth_update_raw_writes_and_rollback() {
    let schema = schema();
    let mut store = MemoryStorage::default();
    ready(store.create_table("items")).unwrap();
    let started = Instant::now();
    for id in 0..256 {
        ready(store.insert("items", row(id, id as i64))).unwrap();
    }
    eprintln!(
        "P12 memory initial_population=256 incremental_index_build_and_population={:?}",
        started.elapsed()
    );
    for size in [256usize, 2048] {
        let started = Instant::now();
        ready(
            store.apply_typed_batch(
                &schema,
                (256..size)
                    .map(|id| Mutation::Insert {
                        table: "items".into(),
                        row: row(id, id as i64),
                    })
                    .collect(),
            ),
        )
        .unwrap();
        let additional_population = started.elapsed();
        for (field, value) in [
            ("number", Value::Integer(42)),
            ("text", Value::String("key-00000042".into())),
        ] {
            let query = query(field, value);
            assert_eq!(store.candidate_count(&schema, &query), Ok(1));
            let started = Instant::now();
            for _ in 0..25 {
                assert_eq!(ready(store.query(&schema, &query)), Ok(vec![row(42, 42)]));
            }
            eprintln!(
                "P12 memory size={size} field={field} candidates=1 additional_population={additional_population:?} repeated_25_lookup={:?}",
                started.elapsed()
            );
        }
        let old = query("number", Value::Integer(42));
        let changed = query("number", Value::Integer(-1));
        let started = Instant::now();
        ready(store.apply_typed_batch(
            &schema,
            vec![Mutation::Replace {
                table: "items".into(),
                row: row(42, -1),
            }],
        ))
        .unwrap();
        let typed_update = started.elapsed();
        assert_eq!(store.candidate_count(&schema, &old), Ok(0));
        assert_eq!(ready(store.query(&schema, &changed)), Ok(vec![row(42, -1)]));
        let started = Instant::now();
        ready(store.replace("items", row(42, 42))).unwrap();
        let raw_update = started.elapsed();
        assert_eq!(store.candidate_count(&schema, &changed), Ok(0));
        let started = Instant::now();
        assert!(matches!(
            ready(store.apply_typed_batch(
                &schema,
                vec![
                    Mutation::Remove {
                        table: "items".into(),
                        id: "00000042".into()
                    },
                    Mutation::Insert {
                        table: "items".into(),
                        row: row(0, 0)
                    },
                ]
            )),
            Err(StorageError::RowAlreadyExists { .. })
        ));
        let rollback = started.elapsed();
        assert_eq!(store.candidate_count(&schema, &old), Ok(1));
        assert_eq!(ready(store.query(&schema, &old)), Ok(vec![row(42, 42)]));
        let ordering = Query {
            table: "items".into(),
            predicates: vec![],
            order: Some(OrderBy {
                field: "number".into(),
                direction: Direction::Desc,
            }),
            limit: Some(3),
        };
        assert_eq!(store.candidate_count(&schema, &ordering), Ok(size));
        let started = Instant::now();
        assert_eq!(
            ready(store.query(&schema, &ordering)),
            Ok((size - 3..size)
                .rev()
                .map(|id| row(id, id as i64))
                .collect())
        );
        eprintln!(
            "P12 memory size={size} typed_update={typed_update:?} raw_update={raw_update:?} rollback={rollback:?} full_ordering={:?} ordering_candidates={size}",
            started.elapsed()
        );
    }
}
