//! TC-0025 P11/P12: fixture-owned physical compatibility, rollback and plans.
use hoyadb_core::storage::{
    Column, ColumnType, Direction, FiniteFloat, Mutation, Operator, OrderBy, Predicate, Query, Row,
    Schema, Storage, StorageError, TableSchema, TypedStorage, TypedTransactionalStorage, Value,
};
use hoyadb_storage_sqlite::SqliteStorage;
use rusqlite::Connection;
use std::{
    collections::BTreeMap,
    future::Future,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
    time::Instant,
};

fn ready<F: Future + Send>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("SQLite operation must return a ready Send future"),
    }
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "hoya-typed-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> PathBuf {
        self.0.join("fixture.sqlite")
    }

    fn open(&self) -> SqliteStorage {
        SqliteStorage::open_unprotected(self.path()).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn schema() -> Schema {
    Schema {
        tables: BTreeMap::from([(
            "items".into(),
            TableSchema {
                columns: [
                    ("text", ColumnType::String),
                    ("number", ColumnType::Integer),
                    ("flag", ColumnType::Boolean),
                    ("float", ColumnType::Float),
                    ("time", ColumnType::Timestamp),
                    ("ref", ColumnType::Reference),
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
    }
}

fn row(id: &str, fields: impl IntoIterator<Item = (&'static str, Value)>) -> Row {
    Row {
        id: id.into(),
        fields: fields.into_iter().map(|(k, v)| (k.into(), v)).collect(),
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

fn backend<T>(result: Result<T, StorageError>) {
    assert!(matches!(result, Err(StorageError::BackendFailure { .. })));
}

fn ddl(c: &Connection) -> Vec<(String, String)> {
    c.prepare("SELECT name,sql FROM sqlite_schema WHERE type='table' AND name NOT GLOB 'sqlite_*' ORDER BY name").unwrap().query_map([],|r|Ok((r.get(0)?,r.get(1)?))).unwrap().collect::<Result<_,_>>().unwrap()
}

#[test]
fn p11_new_tags_roundtrip_and_reopen_preserve_v1_tables_and_old_rows() {
    let f = Fixture::new();
    let mut store = f.open();
    ready(store.create_table("items")).unwrap();
    let old = row(
        "old",
        [
            ("text", Value::String("保留\0old".into())),
            ("number", Value::Integer(i64::MIN)),
            ("flag", Value::Boolean(true)),
        ],
    );
    ready(store.insert("items", old.clone())).unwrap();
    // A preextension v1 fixture has no secondary indexes.
    drop(store);
    let c = Connection::open(f.path()).unwrap();
    c.execute_batch("DROP INDEX __hoya_fields_numeric; DROP INDEX __hoya_fields_text")
        .unwrap();
    let before = ddl(&c);
    drop(c);
    let mut store = f.open();
    let new = row(
        "new",
        [
            ("time", Value::Timestamp(i64::MAX)),
            ("ref", Value::Reference("\0exact".into())),
            ("text", Value::Null),
        ],
    );
    let changes = ready(store.apply_typed_batch(
        &schema(),
        vec![Mutation::Insert {
            table: "items".into(),
            row: new.clone(),
        }],
    ))
    .unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].after, Some(new.clone()));
    drop(store);
    let store = f.open();
    assert_eq!(ready(store.get("items", "old")), Ok(Some(old)));
    assert_eq!(ready(store.get("items", "new")), Ok(Some(new)));
    // SPEC-0037 R1: inspect the file once its owner is closed.
    drop(store);
    let c = Connection::open(f.path()).unwrap();
    assert_eq!(ddl(&c), before);
    assert_eq!(
        c.query_row("SELECT version FROM __hoya_meta", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    let tags: Vec<i64> = c
        .prepare("SELECT kind FROM __hoya_fields WHERE row_id='new' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(tags, vec![6, 7, 5]);
}

#[test]
fn p11_strict_new_tag_decoding_never_coerces_payloads() {
    for assignment in [
        "kind=4,int_value=NULL,text_value=NULL",
        "kind=4,int_value=1e999,text_value=NULL",
        "kind=4,int_value=-1e999,text_value=NULL",
        "kind=4,int_value=9007199254740993,text_value=NULL",
        "kind=4,int_value=9223372036854775807,text_value=NULL",
        "kind=4,int_value=x'31',text_value=NULL",
        "kind=4,int_value='invalid',text_value=NULL",
        "kind=4,int_value=1.5,text_value='extra'",
        "kind=5,int_value=1.5,text_value=NULL",
        "kind=5,int_value=NULL,text_value=NULL",
        "kind=5,int_value=1,text_value='extra'",
        "kind=6,int_value=NULL,text_value=x'80'",
        "kind=6,int_value=1,text_value='extra'",
        "kind=6,int_value=NULL,text_value=NULL",
        "kind=7,int_value=0,text_value=NULL",
        "kind=7,int_value=NULL,text_value='extra'",
    ] {
        let f = Fixture::new();
        let mut store = f.open();
        ready(store.create_table("items")).unwrap();
        ready(store.insert(
            "items",
            row("bad", [("text", Value::String("valid".into()))]),
        ))
        .unwrap();
        // SPEC-0037 R1: corrupt the file while its owner is closed.
        drop(store);
        Connection::open(f.path())
            .unwrap()
            .execute_batch(&format!(
                "UPDATE __hoya_fields SET {assignment} WHERE row_id='bad'"
            ))
            .unwrap();
        let store = f.open();
        backend(ready(store.get("items", "bad")));
    }
}

#[test]
fn p11_float_integer_affinity_is_checked_without_saturating_casts() {
    let f = Fixture::new();
    let mut store = f.open();
    ready(store.create_table("items")).unwrap();
    for (id, number) in [
        ("zero", -0.0),
        ("integral", 42.0),
        ("min", i64::MIN as f64),
        ("beyond", i64::MAX as f64),
        ("subnormal", f64::from_bits(1)),
        ("max", f64::MAX),
        ("negative", -f64::MAX),
    ] {
        let expected = row(
            id,
            [("float", Value::Float(FiniteFloat::new(number).unwrap()))],
        );
        ready(store.apply_typed_batch(
            &schema(),
            vec![Mutation::Insert {
                table: "items".into(),
                row: expected.clone(),
            }],
        ))
        .unwrap();
        assert_eq!(ready(store.get("items", id)), Ok(Some(expected)));
    }
    // SPEC-0037 R1: inspect the file once its owner is closed.
    drop(store);
    let c = Connection::open(f.path()).unwrap();
    assert_eq!(
        c.query_row(
            "SELECT typeof(int_value) FROM __hoya_fields WHERE row_id='integral'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "integer"
    );
    assert_eq!(
        c.query_row(
            "SELECT typeof(int_value) FROM __hoya_fields WHERE row_id='beyond'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "real"
    );
}

#[test]
fn p11_typed_sql_write_and_commit_failures_preserve_snapshots_and_indexes() {
    let f = Fixture::new();
    let mut store = f.open();
    ready(store.create_table("items")).unwrap();
    let original = row("seed", [("number", Value::Integer(7))]);
    ready(store.insert("items", original.clone())).unwrap();
    // SPEC-0037 R1: the owner holds the file, so arm each fault while it is closed.
    drop(store);
    let c = Connection::open(f.path()).unwrap();
    c.execute_batch("CREATE TRIGGER abort_typed BEFORE INSERT ON __hoya_fields WHEN NEW.row_id='fail' BEGIN SELECT RAISE(ABORT,'fixture failure'); END").unwrap();
    let mut store = f.open();
    let batch = || {
        vec![
            Mutation::Replace {
                table: "items".into(),
                row: row("seed", [("number", Value::Integer(9))]),
            },
            Mutation::Insert {
                table: "items".into(),
                row: row("fail", [("number", Value::Integer(9))]),
            },
        ]
    };
    backend(ready(store.apply_typed_batch(&schema(), batch())));
    drop(store);
    // A deferred foreign key fails the owner's own COMMIT; the bundled SQLite
    // enables foreign keys by default on every connection. Open accepts only the
    // exact storage tables, so the fault tables are created through the held store.
    c.execute_batch("DROP TRIGGER abort_typed").unwrap();
    let mut store = f.open();
    for sql in [
        "CREATE TABLE commit_parent(id INTEGER PRIMARY KEY)",
        "CREATE TABLE commit_child(parent INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED)",
        "CREATE TRIGGER commit_fault AFTER INSERT ON __hoya_fields WHEN NEW.row_id='fail' BEGIN INSERT INTO commit_child(parent) VALUES(1); END",
    ] {
        store.execute_test_sql(sql).unwrap();
    }
    backend(ready(store.apply_typed_batch(&schema(), batch())));
    drop(store);
    c.execute_batch("DROP TRIGGER commit_fault; DROP TABLE commit_child; DROP TABLE commit_parent")
        .unwrap();
    drop(c);
    let store = f.open();
    assert_eq!(
        ready(store.query(&schema(), &query("number", Value::Integer(7)))),
        Ok(vec![original])
    );
    assert_eq!(
        ready(store.query(&schema(), &query("number", Value::Integer(9)))),
        Ok(vec![])
    );
}

#[test]
fn p12_actual_numeric_and_text_lookup_plans_and_recorded_costs() {
    let f = Fixture::new();
    let mut store = f.open();
    ready(store.create_table("items")).unwrap();
    let initial = (0..256)
        .map(|i| Mutation::Insert {
            table: "items".into(),
            row: row(
                &format!("{i:08}"),
                [
                    ("number", Value::Integer(i)),
                    ("text", Value::String(format!("key-{i:08}"))),
                ],
            ),
        })
        .collect();
    let started = Instant::now();
    ready(hoyadb_core::storage::TransactionalStorage::apply_batch(
        &mut store, initial,
    ))
    .unwrap();
    let initial_population = started.elapsed();
    drop(store);
    Connection::open(f.path())
        .unwrap()
        .execute_batch("DROP INDEX __hoya_fields_numeric; DROP INDEX __hoya_fields_text")
        .unwrap();
    let started = Instant::now();
    store = f.open();
    let construction = started.elapsed();
    eprintln!(
        "P12 native initial_population=256 raw_population={initial_population:?} populated_index_build_on_open={construction:?}"
    );
    for size in [256usize, 2048] {
        let started = Instant::now();
        let mutations = (256..size)
            .map(|i| Mutation::Insert {
                table: "items".into(),
                row: row(
                    &format!("{i:08}"),
                    [
                        ("number", Value::Integer(i as i64)),
                        ("text", Value::String(format!("key-{i:08}"))),
                    ],
                ),
            })
            .collect();
        ready(store.apply_typed_batch(&schema(), mutations)).unwrap();
        let population = started.elapsed();
        for (field, value, index) in [
            ("number", Value::Integer(42), "__hoya_fields_numeric"),
            (
                "text",
                Value::String("key-00000042".into()),
                "__hoya_fields_text",
            ),
        ] {
            let q = query(field, value);
            let plan = store.query_plan(&schema(), &q).unwrap();
            assert!(
                plan.iter()
                    .any(|p| p.contains(index) && p.contains("SEARCH")),
                "{plan:?}"
            );
            let started = Instant::now();
            for _ in 0..25 {
                let rows = ready(store.query(&schema(), &q)).unwrap();
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].id, "00000042");
            }
            eprintln!(
                "P12 native size={size} field={field} populated_index_build_on_open={construction:?} additional_population={population:?} repeated_25_lookup={:?} plan={plan:?}",
                started.elapsed()
            );
        }
        let started = Instant::now();
        ready(store.apply_typed_batch(
            &schema(),
            vec![Mutation::Replace {
                table: "items".into(),
                row: row(
                    "00000042",
                    [
                        ("number", Value::Integer(-1)),
                        ("text", Value::String("changed".into())),
                    ],
                ),
            }],
        ))
        .unwrap();
        let update = started.elapsed();
        assert!(
            ready(store.query(&schema(), &query("number", Value::Integer(42))))
                .unwrap()
                .is_empty()
        );
        let mut ordering = Query {
            table: "items".into(),
            predicates: vec![],
            order: Some(OrderBy {
                field: "number".into(),
                direction: Direction::Desc,
            }),
            limit: Some(3),
        };
        let started = Instant::now();
        let ordered = ready(store.query(&schema(), &ordering)).unwrap();
        assert_eq!(ordered.len(), 3);
        let order_time = started.elapsed();
        let started = Instant::now();
        ready(store.replace(
            "items",
            row(
                "00000042",
                [
                    ("number", Value::Integer(42)),
                    ("text", Value::String("key-00000042".into())),
                ],
            ),
        ))
        .unwrap();
        let raw_update = started.elapsed();
        let bad = vec![
            Mutation::Remove {
                table: "items".into(),
                id: "00000042".into(),
            },
            Mutation::Insert {
                table: "items".into(),
                row: row("00000000", []),
            },
        ];
        let started = Instant::now();
        assert!(matches!(
            ready(store.apply_typed_batch(&schema(), bad)),
            Err(StorageError::RowAlreadyExists { .. })
        ));
        let rollback = started.elapsed();
        assert_eq!(
            ready(store.query(&schema(), &query("number", Value::Integer(42))))
                .unwrap()
                .len(),
            1
        );
        drop(store);
        let started = Instant::now();
        store = f.open();
        let reopen = started.elapsed();
        ordering.limit = Some(1);
        assert_eq!(ready(store.query(&schema(), &ordering)).unwrap().len(), 1);
        let q = query("number", Value::Integer(42));
        assert_eq!(ready(store.query(&schema(), &q)).unwrap().len(), 1);
        let plan = store.query_plan(&schema(), &q).unwrap();
        assert!(
            plan.iter()
                .any(|p| p.contains("__hoya_fields_numeric") && p.contains("SEARCH")),
            "{plan:?}"
        );
        eprintln!(
            "P12 native size={size} typed_update={update:?} raw_update={raw_update:?} rollback={rollback:?} ordering={order_time:?} reopen_index_check={reopen:?}"
        );
    }
}

// SPEC-0028 / TC-0031 Q01: large operands are transient, including plan/read errors.
#[test]
fn review_large_operands_preserve_disk_inventory_and_reopen() {
    let f = Fixture::new();
    let mut storage = f.open();
    let declared = schema();
    ready(storage.create_table("items")).unwrap();
    ready(storage.insert(
        "items",
        row("retained", [("text", Value::String("owned".into()))]),
    ))
    .unwrap();
    // SPEC-0037 R1: read the on-disk DDL while closed, before the live sequence.
    drop(storage);
    let disk_before = ddl(&Connection::open(f.path()).unwrap());
    let storage = f.open();
    let mut q = query("id", Value::String("retained".into()));
    q.predicates[0].operator = Operator::In;
    q.predicates[0].values = (0..33_005)
        .map(|n| Value::String(format!("missing-{n}")))
        .chain([Value::String("retained".into())])
        .collect();
    assert!(!storage.query_plan(&declared, &q).unwrap().is_empty());
    assert_eq!(
        ready(storage.query(&declared, &q)).unwrap()[0].id,
        "retained"
    );
    // An actual decoder error after operand selection cannot contaminate the next query.
    // The live owner excludes other connections, so corrupt and restore through
    // its one connection (SPEC-0037 R2.3 test seam).
    storage
        .execute_test_sql(
            "UPDATE __hoya_fields SET text_value=CAST(x'80' AS TEXT) WHERE name='text'",
        )
        .unwrap();
    backend(ready(storage.query(&declared, &q)));
    storage
        .execute_test_sql("UPDATE __hoya_fields SET text_value='owned' WHERE name='text'")
        .unwrap();
    q.predicates[0].values = vec![Value::String("absent".into()); 33_005];
    assert!(ready(storage.query(&declared, &q)).unwrap().is_empty());
    drop(storage);
    assert_eq!(ddl(&Connection::open(f.path()).unwrap()), disk_before);
    let storage = f.open();
    assert_eq!(
        ready(storage.get("items", "retained"))
            .unwrap()
            .unwrap()
            .fields["text"],
        Value::String("owned".into())
    );
}
