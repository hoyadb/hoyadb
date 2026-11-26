//! TC-0041 SI-Y03 / SI-M03: genuine delta proofs preserve exact memory state on unwind.
use super::{SnapshotStorage, arm_fault};
use crate::{
    command::{CommandDatabase, CommandDefinition, CommandError, Json, SyncOptions},
    model::{Database, SchemaDefinition, TableDefinition},
    storage::*,
};
use std::{
    collections::BTreeMap,
    future::Future,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("memory operation suspended"),
    }
}

#[derive(Default)]
struct Control {
    armed: bool,
    proof: Option<SyncDelta>,
    /// The next observation read (query or projection) suspends once.
    suspend_read: bool,
}

/// Pending exactly once: a host read that completes on a later poll.
struct SuspendOnce(bool);
impl Future for SuspendOnce {
    type Output = ();
    fn poll(mut self: std::pin::Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
        if std::mem::replace(&mut self.0, true) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

impl Capture {
    async fn observation_read(&self) {
        let suspend = std::mem::take(&mut self.control.lock().unwrap().suspend_read);
        if suspend {
            SuspendOnce(false).await;
        }
    }
}

/// Test-only forwarding adapter: stop immediately before applying the proof
/// produced by the real owner. No test can construct or alter that proof.
struct Capture {
    inner: SnapshotStorage,
    control: Arc<Mutex<Control>>,
}

impl Storage for Capture {
    async fn create_table(&mut self, name: &str) -> Result<(), StorageError> {
        self.inner.create_table(name).await
    }
    async fn insert(&mut self, table: &str, row: Row) -> Result<(), StorageError> {
        self.inner.insert(table, row).await
    }
    async fn get(&self, table: &str, id: &str) -> Result<Option<Row>, StorageError> {
        self.inner.get(table, id).await
    }
    async fn replace(&mut self, table: &str, row: Row) -> Result<(), StorageError> {
        self.inner.replace(table, row).await
    }
    async fn remove(&mut self, table: &str, id: &str) -> Result<bool, StorageError> {
        self.inner.remove(table, id).await
    }
}
impl TransactionalStorage for Capture {
    async fn apply_batch(&mut self, writes: Vec<Mutation>) -> Result<(), StorageError> {
        self.inner.apply_batch(writes).await
    }
}
impl TypedStorage for Capture {
    async fn query(&self, schema: &Schema, query: &Query) -> Result<Vec<Row>, StorageError> {
        self.observation_read().await;
        self.inner.query(schema, query).await
    }
    async fn query_validated(&self, query: ValidatedQuery<'_>) -> Result<Vec<Row>, StorageError> {
        self.observation_read().await;
        self.inner.query_validated(query).await
    }
}
impl TypedTransactionalStorage for Capture {
    async fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        writes: Vec<Mutation>,
    ) -> Result<Vec<CommittedChange>, StorageError> {
        self.inner.apply_typed_batch(schema, writes).await
    }
}
impl SchemaStorage for Capture {
    async fn catalog(&self) -> Result<ModelCatalog, StorageError> {
        self.inner.catalog().await
    }
    async fn scan(&self, table: &str) -> Result<Vec<Row>, StorageError> {
        self.inner.scan(table).await
    }
    async fn apply_schema_plan(&mut self, plan: SchemaPlan) -> Result<(), StorageError> {
        self.inner.apply_schema_plan(plan).await
    }
}
impl ObservationStorage for Capture {
    async fn project(
        &self,
        schema: &Schema,
        query: &Query,
        fields: &[String],
    ) -> Result<Vec<ProjectedRow>, StorageError> {
        self.observation_read().await;
        self.inner.project(schema, query, fields).await
    }
}
impl SyncStorage for Capture {
    fn ownership(&self) -> StoreOwnership {
        self.inner.ownership()
    }
    fn reads_ready(&self) -> bool {
        self.inner.reads_ready()
    }
    fn load_sync(&self) -> Result<Option<Vec<Row>>, StorageError> {
        self.inner.load_sync()
    }
    fn apply_sync_plan(&mut self, plan: SyncPlan) -> Result<Vec<CommittedChange>, StorageError> {
        assert!(
            !self.control.lock().unwrap().armed,
            "owner must supply a delta"
        );
        self.inner.apply_sync_plan(plan)
    }
    fn apply_sync_delta(&mut self, proof: SyncDelta) -> Result<Vec<CommittedChange>, StorageError> {
        let mut control = self.control.lock().unwrap();
        if std::mem::take(&mut control.armed) {
            assert!(control.proof.replace(proof).is_none());
            return Err(captured());
        }
        drop(control);
        self.inner.apply_sync_delta(proof)
    }
}

fn captured() -> StorageError {
    StorageError::BackendFailure {
        message: "test captured the owner's delta before apply".into(),
    }
}

fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([(
            "items".into(),
            TableDefinition {
                columns: BTreeMap::from([(
                    "label".into(),
                    Column {
                        kind: ColumnType::String,
                        optional: false,
                    },
                )]),
                ..Default::default()
            },
        )]),
    }
}

fn fields(label: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([("label".into(), Value::String(label.into()))])
}

fn open<S: SyncStorage>(store: S) -> CommandDatabase<S> {
    run(CommandDatabase::open(
        store,
        schema(),
        vec![],
        [0x71; 16],
        SyncOptions::default(),
    ))
    .unwrap()
}

/// The first command inserts two rows; the next replaces, removes and inserts.
/// Capture ends the owner normally before the proof is subjected to a panic.
fn capture(store: SnapshotStorage, first: bool) -> (SnapshotStorage, SyncDelta) {
    let control = Arc::new(Mutex::new(Control::default()));
    let mut owner = open(Capture {
        inner: store,
        control: control.clone(),
    });
    let request = owner.request().unwrap();
    run(owner.begin(
        request,
        CommandDefinition {
            name: "delta-atomicity-v1".into(),
            arguments: BTreeMap::new(),
            reference_paths: vec![],
        },
        Json::Object(BTreeMap::new()),
        Some(Json::Null),
        1,
    ))
    .unwrap();
    if first {
        run(owner.scope_insert(request, "items", fields("original"))).unwrap();
        run(owner.scope_insert(request, "items", fields("remove"))).unwrap();
    } else {
        run(owner.scope_replace(request, "items", "local:1", fields("replaced"))).unwrap();
        run(owner.scope_remove(request, "items", "local:2")).unwrap();
        run(owner.scope_insert(request, "items", fields("inserted"))).unwrap();
    }
    control.lock().unwrap().armed = true;
    assert_eq!(
        run(owner.finish(request, Json::Null)).unwrap_err(),
        CommandError::Storage(captured())
    );
    let proof = control
        .lock()
        .unwrap()
        .proof
        .take()
        .expect("genuine owner delta captured");
    (owner.into_storage().inner, proof)
}

fn seeded() -> SnapshotStorage {
    let (mut store, proof) = capture(SnapshotStorage::default(), true);
    store.apply_sync_delta(proof).unwrap();
    store
}

fn assert_same(actual: &SnapshotStorage, before: &SnapshotStorage, context: &str) {
    assert_eq!(actual.tables, before.tables, "{context}: raw rows");
    assert_eq!(actual.indexes, before.indexes, "{context}: exact indexes");
}

/// Each skip is derived from the genuine proof's actual write inventory; model
/// and packet writes therefore cannot silently disappear from the fault matrix.
fn points(store: &SnapshotStorage, proof: &SyncDelta) -> Vec<(&'static str, u32)> {
    let mut inserts = 0;
    let mut replaces = 1; // model
    let mut removes = proof.deletes().len();
    for mutation in proof.mutations() {
        match mutation {
            Mutation::Insert {
                ..
            } => inserts += 1,
            Mutation::Replace {
                ..
            } => replaces += 1,
            Mutation::Remove {
                ..
            } => removes += 1,
        }
    }
    for row in proof.upserts() {
        if store
            .tables
            .get(SYNC_TABLE)
            .is_some_and(|rows| rows.contains_key(&row.id))
        {
            replaces += 1;
        } else {
            inserts += 1;
        }
    }
    let mut result = Vec::new();
    for (name, count) in [
        ("map-written", inserts + replaces + removes),
        ("index-partial-add", inserts + replaces),
        ("index-partial-remove", replaces + removes),
        ("index-removed", replaces),
    ] {
        for skip in 0..count {
            result.push((name, u32::try_from(skip).unwrap()));
        }
    }
    if !store.tables.contains_key(SYNC_TABLE) {
        result.push(("table-created", 0));
    }
    result.push(("before-commit", 0));
    result
}

fn check_faults(base: SnapshotStorage, first: bool) {
    let (before, proof) = capture(base, first);
    assert!(!proof.mutations().is_empty());
    assert!(!proof.upserts().is_empty());
    let faults = points(&before, &proof);
    let mut expected = before.clone();
    expected.apply_sync_delta(proof).unwrap();
    for (point, skip) in faults {
        let context = format!("first={first} at {point}+{skip}");
        let (mut actual, proof) = capture(before.clone(), first);
        assert_same(&actual, &before, "capture changes no storage");
        arm_fault(point, skip);
        let unwound = catch_unwind(AssertUnwindSafe(|| actual.apply_sync_delta(proof)));
        assert!(unwound.is_err(), "{context}: fault was not reached");
        assert!(
            super::FAULT.with(|fault| fault.get().is_none()),
            "{context}: hook consumed"
        );
        assert_same(&actual, &before, &context);

        // A consumed proof is never reused, and no owner crosses the panic.
        // Strict open validates the restored store, then produces a fresh proof.
        let (mut actual, retry) = capture(actual, first);
        actual.apply_sync_delta(retry).unwrap();
        assert_same(&actual, &expected, &format!("{context}: fresh proof retry"));
        let reopened = open(actual).into_storage();
        assert_same(&reopened, &expected, &format!("{context}: strict reopen"));
    }
}

#[test]
fn si_y03_initial_delta_panics_restore_rows_indexes_and_allow_fresh_owner_retry() {
    check_faults(SnapshotStorage::default(), true);
}

#[test]
fn si_y03_existing_delta_panics_restore_rows_indexes_and_allow_fresh_owner_retry() {
    check_faults(seeded(), false);
}

#[test]
fn si_y03_delta_rollback_preserves_preexisting_empty_index_maps() {
    let mut db = run(Database::open(SnapshotStorage::default(), schema(), vec![])).unwrap();
    let row = run(db.insert("items", fields("removed"))).unwrap();
    assert!(run(db.remove("items", &row.id)).unwrap());
    let store = db.into_storage();
    assert!(store.tables["items"].is_empty());
    assert_eq!(store.indexes["items"].len(), 3);
    assert!(store.indexes["items"].values().all(BTreeMap::is_empty));
    check_faults(store, true);
}

#[test]
fn si_y03_delta_rollback_removes_only_new_field_index_maps() {
    let mut store = run(Database::open(SnapshotStorage::default(), schema(), vec![]))
        .unwrap()
        .into_storage();
    // Raw Storage permits arbitrary row fields. Remove this task-owned row
    // before strict open, leaving a valid empty table with only its id index.
    run(store.insert(
        "items",
        Row {
            id: "removed".into(),
            fields: BTreeMap::new(),
        },
    ))
    .unwrap();
    assert!(run(store.remove("items", "removed")).unwrap());
    assert_eq!(
        store.indexes["items"]
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["id"]
    );
    assert!(store.indexes["items"]["id"].is_empty());
    check_faults(store, true);
}

mod publication_suspension {
    use super::*;
    use crate::command::{
        EntityChange, EntityConfig, EntityContext, EntityOperation, MergeStep, PullPage,
    };
    use std::collections::BTreeSet;

    fn observed() -> (CommandDatabase<Capture>, Arc<Mutex<Control>>) {
        let control = Arc::new(Mutex::new(Control::default()));
        let mut owner = open(Capture {
            inner: SnapshotStorage::default(),
            control: control.clone(),
        });
        owner
            .configure_entities(
                EntityConfig {
                    scope: "publication".into(),
                    tables: BTreeSet::from(["items".into()]),
                },
                || {
                    Ok(EntityContext {
                        identity: Json::Null,
                        now_ms: 10,
                    })
                },
            )
            .unwrap();
        let query = owner.query("items");
        run(owner.observe(&query)).unwrap();
        owner.drain_events(0);
        (owner, control)
    }

    fn page(checkpoint: &str) -> PullPage {
        PullPage {
            checkpoint: checkpoint.into(),
            complete: true,
            changes: vec![EntityChange {
                table: "items".into(),
                record_id: "remote".into(),
                op: EntityOperation::Update,
                changes: fields("pulled"),
                removed_fields: BTreeSet::new(),
                timestamp_ms: 5,
                field_timestamps: BTreeMap::from([("label".into(), 5)]),
            }],
        }
    }

    /// Poll once and abandon the future, as a cancelled host task does.
    fn abandon_suspended<F: Future>(future: F) {
        let mut future = std::pin::pin!(future);
        assert!(
            future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending(),
            "the publication read did not suspend"
        );
    }

    // The page commit is synchronous and durable; a future abandoned while the following
    // publication read is suspended must not leave the consumed pull ticket outstanding.
    #[test]
    fn e12_committed_pull_retires_its_ticket_before_publication_can_suspend() {
        let (mut owner, control) = observed();
        let request = owner.begin_pull().unwrap().request;
        control.lock().unwrap().suspend_read = true;
        abandon_suspended(owner.apply_pull(request, page("one")));
        assert_eq!(owner.sync_progress().pages, 1, "the page is committed");
        assert_eq!(
            owner
                .begin_pull()
                .expect("the committed page consumed its ticket")
                .since,
            Some("one".into())
        );
    }

    #[test]
    fn e12_committed_merge_retires_its_session_before_publication_can_suspend() {
        let (mut owner, control) = observed();
        let request = owner.begin_pull().unwrap().request;
        let MergeStep::Ready {
            session,
            ..
        } = run(owner.merge_prepare(request, page("one"), BTreeSet::new())).unwrap()
        else {
            panic!("no host table was offered")
        };
        control.lock().unwrap().suspend_read = true;
        abandon_suspended(owner.merge_commit(session));
        assert_eq!(owner.sync_progress().pages, 1, "the page is committed");
        assert_eq!(
            owner.merge_abort(session),
            Err(CommandError::InvalidTicket),
            "the committed session is retired"
        );
        assert_eq!(
            owner
                .begin_pull()
                .expect("the committed page consumed its ticket")
                .since,
            Some("one".into())
        );
    }
}
