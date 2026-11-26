//! SPEC-0028 / TC-0031 Q04: real SQLite decoding faults after a successful commit.
#![cfg(not(target_arch = "wasm32"))]

use hoyadb_core::{model::*, storage::*};
use hoyadb_storage_sqlite::SqliteStorage;
use rusqlite::{Connection, params};
use std::{
    collections::BTreeMap,
    future::{Future, ready},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Waker},
};

fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("synchronous SQLite fixture unexpectedly suspended"),
    }
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "hoya-reactive-real-fault-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&directory).unwrap();
        Self(directory)
    }

    fn path(&self) -> PathBuf {
        self.0.join("fixture.sqlite")
    }

    // Called only between synchronous owner calls, so the owner is idle.
    fn side_channel(&self) -> Connection {
        inspect_idle_owner(&self.path())
    }

    fn assert_committed_identity_and_metadata(&self) {
        let connection = self.side_channel();
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM __hoya_rows WHERE table_name=?1 AND id=?2",
                    params!["orders", "local:1"],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );
        assert_eq!(connection.query_row(
            "SELECT int_value FROM __hoya_fields WHERE table_name=?1 AND row_id=?2 AND name=?3",
            params!["__hoya_model", "state", "next_id"], |row| row.get::<_, i64>(0),
        ).unwrap(), 2);
        assert_eq!(
            connection
                .query_row("SELECT version FROM __hoya_meta", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[derive(Clone, Copy)]
enum Fault {
    InvalidNotesUtf8,
    InvalidTimestampStorage,
}

#[derive(Default)]
struct Control {
    after_commit: Mutex<Option<Fault>>,
    // SPEC-0037 R1: only the live owner's connection can reach the file, so a
    // restore is applied through it before the owner's next read.
    restore: Mutex<Option<String>>,
    physical_corruptions: AtomicUsize,
    real_projection_errors: AtomicUsize,
    real_row_errors: AtomicUsize,
}

struct PostCommitStorage {
    inner: SqliteStorage,
    control: Arc<Control>,
}

impl Control {
    fn restore_notes(&self, expected: &str) {
        *self.restore.lock().unwrap() = Some(format!(
            "UPDATE __hoya_fields SET kind=1,text_value='{expected}',int_value=NULL WHERE table_name='orders' AND row_id='local:1' AND name='notes'"
        ));
    }

    fn restore_time(&self, expected: i64) {
        *self.restore.lock().unwrap() = Some(format!(
            "UPDATE __hoya_fields SET kind=5,text_value=NULL,int_value={expected} WHERE table_name='orders' AND row_id='local:1' AND name='placed_at'"
        ));
    }
}

impl PostCommitStorage {
    fn new(fixture: &Fixture, control: Arc<Control>) -> Self {
        Self {
            inner: SqliteStorage::open_unprotected(fixture.path()).unwrap(),
            control,
        }
    }

    fn apply_restore(&self) {
        if let Some(sql) = self.control.restore.lock().unwrap().take() {
            assert_eq!(self.inner.execute_test_sql(&sql).unwrap(), 1);
        }
    }

    fn corrupt_after_success(&self) {
        let Some(fault) = self.control.after_commit.lock().unwrap().take() else {
            return;
        };
        // The production transaction has already committed. No manufactured error
        // is returned: later production SQLite decoders must encounter these bytes.
        // SPEC-0037 R1/R2.3: the owner holds the file, so corrupt through its connection.
        let updated = match fault {
            Fault::InvalidNotesUtf8 => self.inner.execute_test_sql(
                "UPDATE __hoya_fields SET kind=1,text_value=CAST(x'80' AS TEXT),int_value=NULL WHERE table_name='orders' AND row_id='local:1' AND name='notes'",
            ).unwrap(),
            Fault::InvalidTimestampStorage => self.inner.execute_test_sql(
                "UPDATE __hoya_fields SET kind=5,text_value=NULL,int_value=x'80' WHERE table_name='orders' AND row_id='local:1' AND name='placed_at'",
            ).unwrap(),
        };
        assert_eq!(
            updated, 1,
            "post-commit corruption must reach its owned row"
        );
        self.control
            .physical_corruptions
            .fetch_add(1, Ordering::SeqCst);
    }
}

impl Storage for PostCommitStorage {
    fn create_table(
        &mut self,
        name: &str,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.create_table(name)
    }

    fn insert(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.insert(table, row)
    }

    fn get(
        &self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<Option<Row>, StorageError>> + Send {
        self.inner.get(table, id)
    }

    fn replace(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.replace(table, row)
    }

    fn remove(
        &mut self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send {
        self.inner.remove(table, id)
    }
}

impl TransactionalStorage for PostCommitStorage {
    fn apply_batch(
        &mut self,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.apply_batch(mutations)
    }
}

impl TypedStorage for PostCommitStorage {
    fn query(
        &self,
        schema: &Schema,
        query: &Query,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.apply_restore();
        let result = run(self.inner.query(schema, query));
        if matches!(&result, Err(StorageError::BackendFailure { .. })) {
            self.control.real_row_errors.fetch_add(1, Ordering::SeqCst);
        }
        ready(result)
    }
}

impl TypedTransactionalStorage for PostCommitStorage {
    fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send {
        let committed = run(self.inner.apply_typed_batch(schema, mutations));
        if committed.is_ok() {
            self.corrupt_after_success();
        }
        ready(committed)
    }
}

impl SchemaStorage for PostCommitStorage {
    fn catalog(&self) -> impl Future<Output = Result<ModelCatalog, StorageError>> + Send {
        self.inner.catalog()
    }

    fn scan(&self, table: &str) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.inner.scan(table)
    }

    fn apply_schema_plan(
        &mut self,
        plan: SchemaPlan,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.apply_schema_plan(plan)
    }
}

impl ObservationStorage for PostCommitStorage {
    fn project(
        &self,
        schema: &Schema,
        query: &Query,
        fields: &[String],
    ) -> impl Future<Output = Result<Vec<ProjectedRow>, StorageError>> + Send {
        self.apply_restore();
        let result = run(self.inner.project(schema, query, fields));
        if matches!(&result, Err(StorageError::BackendFailure { .. })) {
            self.control
                .real_projection_errors
                .fetch_add(1, Ordering::SeqCst);
        }
        ready(result)
    }
}

fn definition() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([(
            "orders".into(),
            TableDefinition {
                columns: [
                    ("status", ColumnType::String),
                    ("notes", ColumnType::String),
                    ("total_cents", ColumnType::Integer),
                    ("placed_at", ColumnType::Timestamp),
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
    }
}

fn input() -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("status".into(), Value::String("pending".into())),
        ("notes".into(), Value::String("owned 🌿 notes".into())),
        ("total_cents".into(), Value::Integer(7)),
        ("placed_at".into(), Value::Timestamp(7)),
    ])
}

fn errors(events: &[Notification], token: Subscription) -> usize {
    events
        .iter()
        .filter(|event| event.subscription == token)
        .map(|event| {
            assert!(
                matches!(&event.value, Err(StorageError::BackendFailure { .. })),
                "expected real SQLite backend error: {event:?}"
            );
            1
        })
        .sum()
}

fn row_events(events: &[Notification], token: Subscription) -> Vec<Vec<Row>> {
    events
        .iter()
        .filter(|event| event.subscription == token)
        .map(|event| match &event.value {
            Ok(ObservationValue::Rows(rows)) => rows.clone(),
            other => panic!("expected rows, got {other:?}"),
        })
        .collect()
}

fn count_events(events: &[Notification], token: Subscription) -> Vec<usize> {
    events
        .iter()
        .filter(|event| event.subscription == token)
        .map(|event| match &event.value {
            Ok(ObservationValue::Count(count)) => *count,
            other => panic!("count poisoned by row-only fault: {other:?}"),
        })
        .collect()
}

fn assert_retained_state(store: &PostCommitStorage, expected: &Row) {
    let catalog = run(store.catalog()).unwrap();
    let state = catalog.state.unwrap();
    assert_eq!(state.version, 1);
    assert_eq!(state.next_id, 2);
    assert_eq!(state.definition, definition().canonical().unwrap());
    assert_eq!(catalog.tables, vec!["orders".to_owned()]);
    let mut persisted = expected.clone();
    persisted
        .fields
        .insert(ROW_VERSION.into(), Value::Integer(1));
    assert_eq!(run(store.scan("orders")).unwrap(), vec![persisted]);
}

#[test]
fn q04_real_postcommit_row_decode_failure_keeps_projected_count_live() {
    let fixture = Fixture::new();
    let control = Arc::new(Control::default());
    let mut db = run(Database::open(
        PostCommitStorage::new(&fixture, control.clone()),
        definition(),
        vec![],
    ))
    .unwrap()
    .into_reactive();
    let query = db
        .query("orders")
        .where_("status", Operator::Eq, Value::String("pending".into()));
    let ordinary = run(query.observe(&mut db)).unwrap();
    let columns = run(query.observe_with_columns(&mut db, vec!["total_cents".into()])).unwrap();
    let count = run(query.observe_count(&mut db)).unwrap();
    let initial = db.drain_notifications(100);
    assert_eq!(row_events(&initial, ordinary), vec![Vec::<Row>::new()]);
    assert_eq!(row_events(&initial, columns), vec![Vec::<Row>::new()]);
    assert_eq!(count_events(&initial, count), vec![0]);
    assert_eq!(db.observation_stats().groups, 1);
    *control.after_commit.lock().unwrap() = Some(Fault::InvalidNotesUtf8);
    let committed = run(db.insert("orders", input()))
        .expect("physical fault occurs only after the mutation commits successfully");
    assert_eq!(
        committed,
        Row {
            id: "local:1".into(),
            fields: input()
        }
    );
    assert_eq!(control.physical_corruptions.load(Ordering::SeqCst), 1);
    fixture.assert_committed_identity_and_metadata();
    assert!(
        control.real_row_errors.load(Ordering::SeqCst) >= 1,
        "production full-row decoder must actually fail"
    );
    assert_eq!(
        control.real_projection_errors.load(Ordering::SeqCst),
        0,
        "unrequested notes must not be decoded for projected count"
    );
    let failures = db.drain_notifications(116);
    assert_eq!(errors(&failures, ordinary), 1);
    assert_eq!(errors(&failures, columns), 1);
    assert_eq!(count_events(&failures, count), vec![1]);
    let extra_count = run(query.observe_count(&mut db))
        .expect("valid projected keys must remain shareable despite row delivery failure");
    assert_eq!(
        count_events(&db.drain_notifications(117), extra_count),
        vec![1]
    );
    assert_eq!(db.observation_stats().groups, 1);
    let before = db.observation_stats();
    assert!(matches!(
        run(query.observe(&mut db)),
        Err(StorageError::BackendFailure { .. })
    ));
    let after = db.observation_stats();
    assert_eq!(after.groups, before.groups);
    assert_eq!(after.observers, before.observers);
    assert!(db.unsubscribe(extra_count));
    for now in [118, 119] {
        run(db.refresh_notifications());
        let failed = db.drain_notifications(now);
        assert_eq!(errors(&failed, ordinary), 1);
        assert_eq!(errors(&failed, columns), 1);
        assert!(count_events(&failed, count).is_empty());
        assert_eq!(failed.len(), 2);
    }
    control.restore_notes("owned 🌿 notes");
    run(db.refresh_notifications());
    let recovered = db.drain_notifications(120);
    assert_eq!(
        row_events(&recovered, ordinary),
        vec![vec![committed.clone()]]
    );
    assert_eq!(
        row_events(&recovered, columns),
        vec![vec![committed.clone()]]
    );
    assert!(
        count_events(&recovered, count).is_empty(),
        "valid unchanged count must not duplicate on row recovery"
    );
    assert_eq!(run(query.fetch(&db)).unwrap(), vec![committed.clone()]);
    assert!(db.drain_notifications(132).is_empty());
    assert_retained_state(&db.into_storage(), &committed);
}

#[test]
fn q04_real_postcommit_projection_decode_failure_retries_and_retains_count_deadline() {
    let fixture = Fixture::new();
    let control = Arc::new(Control::default());
    let mut db = run(Database::open(
        PostCommitStorage::new(&fixture, control.clone()),
        definition(),
        vec![],
    ))
    .unwrap()
    .into_reactive();
    let query = db
        .query("orders")
        .where_("status", Operator::Eq, Value::String("pending".into()))
        .order_by("placed_at", Direction::Asc);
    let ordinary = run(query.observe(&mut db)).unwrap();
    let columns = run(query.observe_with_columns(&mut db, vec!["total_cents".into()])).unwrap();
    let count = run(query.observe_count(&mut db)).unwrap();
    db.drain_notifications(100);
    *control.after_commit.lock().unwrap() = Some(Fault::InvalidTimestampStorage);
    let committed = run(db.insert("orders", input()))
        .expect("committed write must survive a later actual projection decoder failure");
    fixture.assert_committed_identity_and_metadata();
    assert!(control.real_projection_errors.load(Ordering::SeqCst) >= 1);
    let initial_failure = db.drain_notifications(101);
    for token in [ordinary, columns, count] {
        assert_eq!(errors(&initial_failure, token), 1);
    }
    assert_eq!(initial_failure.len(), 3);
    for now in [102, 103] {
        run(db.refresh_notifications());
        let failed = db.drain_notifications(now);
        for token in [ordinary, columns, count] {
            assert_eq!(errors(&failed, token), 1);
        }
        assert_eq!(failed.len(), 3);
    }
    assert!(
        control.real_projection_errors.load(Ordering::SeqCst) >= 3,
        "each retry must reach the actual SQLite decoder"
    );
    control.restore_time(7);
    run(db.refresh_notifications());
    let rows = db.drain_notifications(104);
    assert_eq!(row_events(&rows, ordinary), vec![vec![committed.clone()]]);
    assert_eq!(row_events(&rows, columns), vec![vec![committed.clone()]]);
    assert!(count_events(&rows, count).is_empty());
    assert!(db.drain_notifications(115).is_empty());
    assert_eq!(count_events(&db.drain_notifications(116), count), vec![1]);
    assert_eq!(run(query.fetch(&db)).unwrap(), vec![committed.clone()]);
    assert_retained_state(&db.into_storage(), &committed);
}

/// Read-only, lock-free inspection of a live owner's committed pages, for task
/// fixtures only (SPEC-0037 R1 excludes every locking reader). Preconditions: the
/// same owner is idle between synchronous calls, with no open, queued or
/// concurrent write, so the main file holds only committed pages. The fixture path
/// is a known temp path of an unprotected task fixture; it is checked to need no
/// URI escaping. The connection is read-only and query-only, a write attempt is
/// proven to fail, no `immutable` flag is used, and every error is fatal. Each
/// inspection opens a fresh connection that drops before the next owner call. This
/// supplements, and never replaces, production close/reopen assertions.
fn inspect_idle_owner(path: &std::path::Path) -> rusqlite::Connection {
    let text = path.to_str().unwrap();
    assert!(
        text.bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/._-".contains(&byte))
    );
    let sql = rusqlite::Connection::open_with_flags(
        format!("file:{text}?mode=ro&nolock=1"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .unwrap();
    sql.execute_batch("PRAGMA query_only=1").unwrap();
    let write = sql.execute_batch("CREATE TABLE inspection_write_probe(x)");
    assert_eq!(
        write.err().and_then(|error| error.sqlite_error_code()),
        Some(rusqlite::ErrorCode::ReadOnly)
    );
    sql
}
