//! TC-0022 Q03–Q08: dedicated, owned SQLite fixtures.
use hoyadb_core::storage::{Mutation, Row, Storage, StorageError, TransactionalStorage, Value};
use hoyadb_storage_sqlite::SqliteStorage;
use rusqlite::Connection;
use std::{
    collections::BTreeMap,
    future::Future,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
};

fn ready<F: Future + Send>(future: F) -> F::Output {
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    match std::pin::pin!(future).poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("SQLite operation must be ready on its first poll"),
    }
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "hoya-sqlite-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> PathBuf {
        self.0.join("database.sqlite")
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

fn row(id: &str) -> Row {
    Row {
        id: id.into(),
        fields: BTreeMap::from([
            ("a".into(), Value::String("保留\0'; DROP TABLE x;--".into())),
            ("b".into(), Value::Integer(i64::MIN)),
            ("c".into(), Value::Boolean(true)),
        ]),
    }
}

fn backend<T>(result: Result<T, StorageError>) {
    assert!(matches!(result, Err(StorageError::BackendFailure { .. })));
}

fn seeded(f: &Fixture) -> SqliteStorage {
    let mut store = f.open();
    ready(store.create_table("orders")).unwrap();
    ready(store.insert("orders", row("seed"))).unwrap();
    store
}

#[test]
fn q03_bound_names_and_empty_rows() {
    let f = Fixture::new();
    let mut store = f.open();
    for table in ["", "表\0'; DROP TABLE __hoya_rows;--"] {
        ready(store.create_table(table)).unwrap();
        let mut expected = row("\0é'--");
        expected
            .fields
            .insert("\0名'--".into(), Value::Integer(i64::MAX));
        ready(store.insert(table, expected.clone())).unwrap();
        assert_eq!(
            ready(store.get(table, &expected.id)),
            Ok(Some(expected.clone()))
        );
        expected.fields.clear();
        ready(store.replace(table, expected.clone())).unwrap();
        assert_eq!(
            ready(store.get(table, &expected.id)),
            Ok(Some(expected.clone()))
        );
        assert_eq!(ready(store.remove(table, &expected.id)), Ok(true));
        let empty = Row {
            id: String::new(),
            fields: BTreeMap::new(),
        };
        ready(store.insert(table, empty.clone())).unwrap();
        assert_eq!(ready(store.get(table, "")), Ok(Some(empty)));
    }
}

#[test]
fn q04_open_rejects_paths_and_unowned_formats_without_mutation() {
    backend(SqliteStorage::open_unprotected(""));
    backend(SqliteStorage::open_unprotected(":memory:"));
    let uri_rejections: Vec<_> = ["file::memory:", "file:hoya-memory?mode=memory&cache=shared"]
        .into_iter()
        .map(|uri| {
            matches!(
                SqliteStorage::open_unprotected(uri),
                Err(StorageError::BackendFailure { .. })
            )
        })
        .collect();
    assert_eq!(
        uri_rejections,
        vec![true, true],
        "SQLite URIs must not silently open a memory-backed adapter"
    );
    let f = Fixture::new();
    backend(SqliteStorage::open_unprotected(&f.0));
    backend(SqliteStorage::open_unprotected(
        f.0.join("missing/database"),
    ));
    assert!(!f.0.join("missing").exists());
    for sql in [
        "CREATE TABLE unrelated(x); INSERT INTO unrelated VALUES ('retained')",
        "CREATE TABLE sqliteXunrelated(x); INSERT INTO sqliteXunrelated VALUES ('retained')",
        "CREATE TABLE __hoya_meta(version INTEGER NOT NULL); INSERT INTO __hoya_meta VALUES (99)",
        "CREATE TABLE __hoya_meta(version INTEGER NOT NULL); INSERT INTO __hoya_meta VALUES (1)",
        "CREATE TABLE __hoya_meta(version INTEGER NOT NULL); INSERT INTO __hoya_meta VALUES (1); CREATE TABLE __hoya_tables(name TEXT)",
        "CREATE TABLE __hoya_meta(version INTEGER NOT NULL)",
        "CREATE TABLE __hoya_meta(version INTEGER NOT NULL); INSERT INTO __hoya_meta VALUES (1),(1)",
        "CREATE TABLE __hoya_meta(version TEXT NOT NULL); INSERT INTO __hoya_meta VALUES ('1')",
    ] {
        let path = f.path();
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch(sql).unwrap();
        }
        let before = std::fs::read(&path).unwrap();
        backend(SqliteStorage::open_unprotected(&path));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::remove_file(&path).unwrap();
    }
    for incompatible in [
        "UPDATE __hoya_meta SET version=99",
        "UPDATE __hoya_meta SET version='bad'",
        "DELETE FROM __hoya_meta",
        "INSERT INTO __hoya_meta VALUES (1)",
        "ALTER TABLE __hoya_rows RENAME COLUMN id TO incompatible",
    ] {
        drop(f.open());
        Connection::open(f.path())
            .unwrap()
            .execute_batch(incompatible)
            .unwrap();
        let before = std::fs::read(f.path()).unwrap();
        backend(SqliteStorage::open_unprotected(f.path()));
        assert_eq!(std::fs::read(f.path()).unwrap(), before);
        std::fs::remove_file(f.path()).unwrap();
    }
    // An existing empty SQLite file is eligible; unrelated schemas above are not.
    Connection::open(f.path())
        .unwrap()
        .execute_batch("VACUUM")
        .unwrap();
    let mut initialized = f.open();
    ready(initialized.create_table("empty-file")).unwrap();
    drop(initialized);
    std::fs::remove_file(f.path()).unwrap();
    std::fs::write(f.path(), b"not a SQLite database\0retained").unwrap();
    let before = std::fs::read(f.path()).unwrap();
    backend(SqliteStorage::open_unprotected(f.path()));
    assert_eq!(std::fs::read(f.path()).unwrap(), before);
}

// TC-0022 Q05 moved under SPEC-0037 R1/R2: its real COMMIT failure assertions are
// SI-K01 in src/lib.rs, and its lock-held write assertions are SI-L05.

#[test]
fn q06_corrupt_fields_never_return_partial_rows() {
    for assignment in [
        "kind=99",
        "kind=3,int_value=2,text_value=NULL",
        "kind=2,int_value=1.5,text_value=NULL",
        "kind=1,text_value=x'80',int_value=NULL",
        "kind=1,text_value=NULL,int_value=NULL",
        "kind=2,int_value=NULL,text_value=NULL",
        "kind=3,int_value=NULL,text_value=NULL",
        "kind=1,text_value='ok',int_value=1",
        "kind=2,int_value=1,text_value='forbidden'",
        "kind=3,int_value=1,text_value='forbidden'",
    ] {
        let f = Fixture::new();
        let mut store = seeded(&f);
        ready(store.insert("orders", row("valid"))).unwrap();
        // SPEC-0037 R1: the owner holds the file, so corrupt it while closed.
        drop(store);
        let connection = Connection::open(f.path()).unwrap();
        connection.execute_batch(&format!("UPDATE __hoya_fields SET {assignment} WHERE table_name='orders' AND row_id='seed' AND name='a'" )).unwrap();
        drop(connection);
        let store = f.open();
        backend(ready(store.get("orders", "seed")));
        assert_eq!(ready(store.get("orders", "valid")), Ok(Some(row("valid"))));
    }
}

#[test]
fn q07_trigger_failure_rolls_back_single_mutations_and_batch() {
    let f = Fixture::new();
    // SPEC-0037 R1: the owner holds the file, so install the trigger while closed.
    drop(seeded(&f));
    let connection = Connection::open(f.path()).unwrap();
    connection.execute_batch("CREATE TRIGGER abort_field BEFORE INSERT ON __hoya_fields WHEN NEW.name='fail' BEGIN SELECT RAISE(ABORT, 'fixture failure'); END;").unwrap();
    drop(connection);
    let mut store = f.open();
    let failing = |id| {
        let mut r = row(id);
        r.fields.insert("fail".into(), Value::Boolean(false));
        r
    };
    backend(ready(store.insert("orders", failing("partial"))));
    backend(ready(store.replace("orders", failing("seed"))));
    backend(ready(store.apply_batch(vec![
        Mutation::Remove {
            table: "orders".into(),
            id: "seed".into(),
        },
        Mutation::Insert {
            table: "orders".into(),
            row: row("prefix"),
        },
        Mutation::Insert {
            table: "orders".into(),
            row: failing("partial"),
        },
    ])));
    drop(store);
    let store = f.open();
    assert_eq!(ready(store.get("orders", "seed")), Ok(Some(row("seed"))));
    for id in ["partial", "prefix"] {
        assert_eq!(ready(store.get("orders", id)), Ok(None));
    }
}

#[test]
fn q08_owned_connection_moves_and_operations_are_eager_ready_send() {
    let f = Fixture::new();
    let store = f.open();
    std::thread::spawn(move || {
        let mut store = store;
        drop(store.create_table("orders"));
        drop(store.insert("orders", row("seed")));
        assert_eq!(ready(store.get("orders", "seed")), Ok(Some(row("seed"))));
        ready(store.create_table("orders")).unwrap();
        ready(store.replace("orders", row("seed"))).unwrap();
        let mut changed = row("seed");
        changed.fields.clear();
        drop(store.replace("orders", changed.clone()));
        assert_eq!(ready(store.get("orders", "seed")), Ok(Some(changed)));
        drop(store.apply_batch(vec![Mutation::Replace {
            table: "orders".into(),
            row: row("seed"),
        }]));
        assert_eq!(ready(store.get("orders", "seed")), Ok(Some(row("seed"))));
        drop(store.insert("orders", row("discarded")));
        drop(store.remove("orders", "discarded"));
        assert_eq!(ready(store.get("orders", "discarded")), Ok(None));
        ready(store.apply_batch(vec![Mutation::Insert {
            table: "orders".into(),
            row: row("new"),
        }]))
        .unwrap();
        assert_eq!(ready(store.remove("orders", "new")), Ok(true));
        assert_eq!(
            ready(store.insert("orders", row("seed"))),
            Err(StorageError::RowAlreadyExists {
                table: "orders".into(),
                id: "seed".into()
            })
        );
    })
    .join()
    .unwrap();
    assert_eq!(ready(f.open().get("orders", "seed")), Ok(Some(row("seed"))));
}
