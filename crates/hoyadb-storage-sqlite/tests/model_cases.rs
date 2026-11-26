//! TC-0026 D15: real SQL failures preserve data, metadata and ID allocation.
use hoyadb_core::{
    model::{Database, Migration},
    storage::*,
};
use hoyadb_storage_sqlite::SqliteStorage;
use rusqlite::Connection;
use std::{
    collections::BTreeMap,
    future::Future,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
};

#[path = "../../../tests/conformance/model_restart.rs"]
mod fixture_model;
fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("synchronous fixture suspended"),
    }
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "hoya-model-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        Self(directory)
    }

    fn path(&self) -> PathBuf {
        self.0.join("model.sqlite")
    }

    fn open(&self) -> SqliteStorage {
        SqliteStorage::open_unprotected(self.path()).unwrap()
    }

    fn sql(&self) -> Connection {
        Connection::open(self.path()).unwrap()
    }

    fn abort_metadata(&self) {
        self.sql().execute_batch("CREATE TRIGGER abort_model BEFORE INSERT ON __hoya_fields WHEN NEW.table_name='__hoya_model' BEGIN SELECT RAISE(ABORT,'fixture metadata write failure'); END").unwrap();
    }

    fn release(&self) {
        self.sql()
            .execute_batch("DROP TRIGGER abort_model")
            .unwrap();
    }

    // SPEC-0037 R2: a deferred foreign key fails the owner's own COMMIT. The
    // bundled SQLite enables foreign keys by default on every connection. Open
    // accepts only the exact storage tables, so the fixture creates its fault
    // tables through the store it still holds directly (R2.3 test seam).
    fn fail_commit(&self) -> SqliteStorage {
        let store = self.open();
        for sql in [
            "CREATE TABLE commit_parent(id INTEGER PRIMARY KEY)",
            "CREATE TABLE commit_child(parent INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED)",
            "CREATE TRIGGER commit_fault AFTER INSERT ON __hoya_fields WHEN NEW.table_name='customers' BEGIN INSERT INTO commit_child(parent) VALUES(1); END",
        ] {
            store.execute_test_sql(sql).unwrap();
        }
        store
    }

    fn release_commit(&self) {
        self.sql()
            .execute_batch(
                "DROP TRIGGER commit_fault; DROP TABLE commit_child; DROP TABLE commit_parent",
            )
            .unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn snapshot(store: &SqliteStorage) -> (ModelCatalog, BTreeMap<String, Vec<Row>>) {
    let catalog = run(store.catalog()).unwrap();
    let rows = catalog
        .tables
        .iter()
        .map(|name| (name.clone(), run(store.scan(name)).unwrap()))
        .collect();
    (catalog, rows)
}

fn backend<T>(value: Result<T, StorageError>) {
    assert!(matches!(value, Err(StorageError::BackendFailure { .. })));
}

fn recover(value: Result<Database<SqliteStorage>, (StorageError, SqliteStorage)>) -> SqliteStorage {
    match value {
        Err((error, store)) => {
            backend::<()>(Err(error));
            store
        }
        Ok(_) => panic!("SQL fault unexpectedly committed"),
    }
}

#[test]
fn d15_initialization_and_legacy_adoption_roll_back_after_metadata_abort() {
    for adopt in [false, true] {
        let fixture = Fixture::new();
        let mut store = fixture.open();
        if adopt {
            run(store.create_table("customers")).unwrap();
            run(store.insert(
                "customers",
                Row {
                    id: "legacy".into(),
                    fields: BTreeMap::from([("country".into(), Value::String("FR".into()))]),
                },
            ))
            .unwrap();
        }
        let original = snapshot(&store);
        // SPEC-0037 R1: arm the fault while the owner is closed.
        drop(store);
        fixture.abort_metadata();
        let store = fixture.open();
        let store = recover(run(Database::open_recoverable(
            store,
            fixture_model::schema(1),
            vec![],
        )));
        assert_eq!(snapshot(&store), original);
        drop(store);
        assert_eq!(snapshot(&fixture.open()), original);
        fixture.release();
        assert!(
            run(Database::open(
                fixture.open(),
                fixture_model::schema(1),
                vec![]
            ))
            .is_ok()
        );
    }
}

#[test]
fn d15_public_insert_counter_and_row_share_one_commit() {
    let fixture = Fixture::new();
    let db = run(Database::open(
        fixture.open(),
        fixture_model::schema(1),
        vec![],
    ))
    .unwrap();
    // SPEC-0037 R1: arm the fault while the owner is closed.
    drop(db);
    fixture.abort_metadata();
    let mut db = run(Database::open(
        fixture.open(),
        fixture_model::schema(1),
        vec![],
    ))
    .unwrap();
    backend(run(db.insert(
        "customers",
        BTreeMap::from([("country".into(), Value::String("FR".into()))]),
    )));
    let store = db.into_storage();
    assert_eq!(run(store.catalog()).unwrap().state.unwrap().next_id, 1);
    assert_eq!(run(store.scan("customers")), Ok(vec![]));
    drop(store);
    fixture.release();
    let mut db = run(Database::open(
        fixture.open(),
        fixture_model::schema(1),
        vec![],
    ))
    .unwrap();
    assert_eq!(
        run(db.insert(
            "customers",
            BTreeMap::from([("country".into(), Value::String("FR".into()))])
        ))
        .unwrap()
        .id,
        "local:1"
    );
}

#[test]
fn d15_migration_and_missing_chain_reset_abort_preserve_reopened_state() {
    for reset in [false, true] {
        let fixture = Fixture::new();
        run(fixture_model::write(fixture.open())).unwrap();
        let original = snapshot(&fixture.open());
        // SPEC-0037 R1: arm the fault while the owner is closed.
        fixture.abort_metadata();
        let store = fixture.open();
        let target = if reset {
            4
        } else {
            3
        };
        let migrations = if reset {
            vec![]
        } else {
            vec![Migration {
                from_version: 2,
                to: fixture_model::schema(3),
                steps: vec![],
            }]
        };
        let store = recover(run(Database::open_recoverable(
            store,
            fixture_model::schema(target),
            migrations.clone(),
        )));
        assert_eq!(snapshot(&store), original);
        drop(store);
        let reopened = fixture.open();
        assert_eq!(snapshot(&reopened), original);
        run(fixture_model::read(reopened, false)).unwrap();
        fixture.release();
        let db = run(Database::open(
            fixture.open(),
            fixture_model::schema(target),
            migrations,
        ))
        .unwrap();
        let store = db.into_storage();
        assert_eq!(run(store.catalog()).unwrap().state.unwrap().version, target);
        if reset {
            assert!(run(store.scan("orders")).unwrap().is_empty());
        }
        drop(store);
        let c = fixture.sql();
        assert_eq!(
            c.query_row("SELECT version FROM __hoya_meta", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(c.query_row("SELECT count(*) FROM sqlite_schema WHERE type='table' AND name NOT GLOB 'sqlite_*'",[],|r|r.get::<_,i64>(0)).unwrap(),4);
    }
}

#[test]
fn d15_actual_commit_failure_preserves_counter_and_retry_identity() {
    let fixture = Fixture::new();
    drop(
        run(Database::open(
            fixture.open(),
            fixture_model::schema(1),
            vec![],
        ))
        .unwrap(),
    );
    // SPEC-0037 R1: the owner holds the file, so arm a real COMMIT failure on the
    // store before the owner takes it.
    let mut db = run(Database::open(
        fixture.fail_commit(),
        fixture_model::schema(1),
        vec![],
    ))
    .unwrap();
    backend(run(db.insert(
        "customers",
        BTreeMap::from([("country".into(), Value::String("FR".into()))]),
    )));
    let store = db.into_storage();
    assert_eq!(run(store.catalog()).unwrap().state.unwrap().next_id, 1);
    assert!(run(store.scan("customers")).unwrap().is_empty());
    drop(store);
    fixture.release_commit();
    let mut db = run(Database::open(
        fixture.open(),
        fixture_model::schema(1),
        vec![],
    ))
    .unwrap();
    assert_eq!(
        run(db.insert(
            "customers",
            BTreeMap::from([("country".into(), Value::String("FR".into()))])
        ))
        .unwrap()
        .id,
        "local:1"
    );
}

#[test]
fn d15_orphan_physical_bookkeeping_is_never_adopted_or_reset() {
    for sql in [
        "INSERT INTO __hoya_rows VALUES('ghost','orphan')",
        "INSERT INTO __hoya_fields VALUES('ghost','orphan','field',1,'preserve',NULL)",
    ] {
        let fixture = Fixture::new();
        // SPEC-0037 R1: write the orphan while the owner is closed.
        drop(fixture.open());
        fixture.sql().execute_batch(sql).unwrap();
        let store = fixture.open();
        let store = recover(run(Database::open_recoverable(
            store,
            fixture_model::schema(4),
            vec![],
        )));
        drop(store);
        let c = fixture.sql();
        assert_eq!(
            c.query_row("SELECT count(*) FROM __hoya_tables", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        let preserved: i64 = c
            .query_row(
                "SELECT (SELECT count(*) FROM __hoya_rows)+(SELECT count(*) FROM __hoya_fields)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(preserved, 1);
    }
}
