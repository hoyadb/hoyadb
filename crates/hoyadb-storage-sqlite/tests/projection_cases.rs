//! TC-0029 R11: projection reads consulted physical cells, never whole records.
use hoyadb_core::storage::*;
use hoyadb_storage_sqlite::SqliteStorage;
use rusqlite::Connection;
use std::{
    collections::BTreeMap,
    future::Future,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
};

fn run<F: Future + Send>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("SQLite projection must return ready Send futures"),
    }
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "hoya-project-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        Self(directory)
    }

    fn path(&self) -> PathBuf {
        self.0.join("fixture.sqlite")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn r11_unrequested_corrupt_payload_is_not_materialized() {
    let f = Fixture::new();
    let mut engine = SqliteStorage::open_unprotected(f.path()).unwrap();
    run(engine.create_table("items")).unwrap();
    let schema = Schema {
        tables: BTreeMap::from([(
            "items".into(),
            TableSchema {
                columns: [
                    ("number", ColumnType::Integer),
                    ("noise", ColumnType::String),
                ]
                .into_iter()
                .map(|(name, kind)| {
                    (
                        name.into(),
                        Column {
                            kind,
                            optional: false,
                        },
                    )
                })
                .collect(),
                relations: BTreeMap::new(),
            },
        )]),
    };
    let row = Row {
        id: "a".into(),
        fields: BTreeMap::from([
            ("number".into(), Value::Integer(7)),
            ("noise".into(), Value::String("valid".into())),
        ]),
    };
    run(engine.insert("items", row)).unwrap();
    let query = Query {
        table: "items".into(),
        predicates: vec![],
        order: None,
        limit: None,
    };
    // SPEC-0037 R1: the owner holds the file, so corrupt it while closed.
    drop(engine);
    Connection::open(f.path())
        .unwrap()
        .execute_batch("UPDATE __hoya_fields SET text_value=CAST(x'80' AS TEXT) WHERE name='noise'")
        .unwrap();
    let engine = SqliteStorage::open_unprotected(f.path()).unwrap();
    assert_eq!(
        run(engine.project(&schema, &query, &[])),
        Ok(vec![ProjectedRow {
            order_key: None,
            id: "a".into(),
            values: BTreeMap::new()
        }])
    );
    assert_eq!(
        run(engine.project(&schema, &query, &["number".into()])),
        Ok(vec![ProjectedRow {
            order_key: None,
            id: "a".into(),
            values: BTreeMap::from([("number".into(), Value::Integer(7))])
        }])
    );
    assert!(matches!(
        run(engine.query(&schema, &query)),
        Err(StorageError::BackendFailure { .. })
    ));
    assert!(matches!(
        run(engine.project(&schema, &query, &["noise".into()])),
        Err(StorageError::BackendFailure { .. })
    ));
    let mut zero = query.clone();
    zero.limit = Some(0);
    assert_eq!(
        run(engine.project(&schema, &zero, &["noise".into()])),
        Ok(vec![])
    );
    let mut hidden = query;
    hidden.order = Some(OrderBy {
        field: "noise".into(),
        direction: Direction::Asc,
    });
    assert!(matches!(
        run(engine.project(&schema, &hidden, &[])),
        Err(StorageError::BackendFailure { .. })
    ));
}
