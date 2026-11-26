//! R9/SI-Y03 public adapter races. Only genuine owner-created deltas are forwarded.
use hoyadb_core::{command::*, model::*, storage::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};
static OWNERS: Mutex<()> = Mutex::new(());
fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("native concrete adapter suspended"),
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Race {
    UntouchedMalformed,
    CommandHeader,
    EntityHeader,
    ChangedMetadata,
    GuardedRow,
}
type Snapshot = BTreeMap<String, Vec<Row>>;
#[derive(Clone, Debug)]
struct Attempt {
    before: Snapshot,
    raced: Snapshot,
    table: String,
    row: Row,
}
#[derive(Default)]
struct Control {
    armed: Option<Race>,
    attempt: Option<Attempt>,
    capture_next: bool,
    captured: Option<SyncDelta>,
}
pub struct Racing<S> {
    inner: S,
    ownership: StoreOwnership,
    control: Arc<Mutex<Control>>,
    before_apply: Box<dyn FnMut(&mut S) + Send>,
}
/// An actual-adapter hook immediately before forwarding a full plan or genuine
/// owner proof. Callers arm their captured flag after fixture setup.
pub fn hooked<S: SyncStorage>(inner: S, hook: impl FnMut(&mut S) + Send + 'static) -> Racing<S> {
    let ownership = inner.ownership();
    Racing {
        inner,
        ownership,
        control: Arc::new(Mutex::new(Control::default())),
        before_apply: Box::new(hook),
    }
}

enum Images<'a> {
    Full(&'a SyncPlan),
    Delta(&'a SyncDelta),
}
impl Images<'_> {
    fn prior(&self, id: &str) -> Option<&Row> {
        match self {
            Self::Full(plan) => plan
                .expected_sync
                .as_deref()
                .unwrap_or_default()
                .iter()
                .find(|r| r.id == id),
            Self::Delta(delta) => delta
                .expected_packets()
                .iter()
                .find(|(key, _)| key == id)
                .and_then(|(_, row)| row.as_ref()),
        }
    }
    fn next(&self, id: &str) -> Option<&Row> {
        match self {
            Self::Full(plan) => plan.next_sync.iter().find(|r| r.id == id),
            Self::Delta(delta) => {
                if delta.deletes().iter().any(|key| key == id) {
                    None
                } else {
                    delta
                        .upserts()
                        .iter()
                        .find(|r| r.id == id)
                        .or_else(|| self.prior(id))
                }
            }
        }
    }
    fn guards(&self) -> &[RowGuard] {
        match self {
            Self::Full(plan) => &plan.guards,
            Self::Delta(delta) => delta.guards(),
        }
    }
    fn assert_changed(&self, row: &Row) {
        assert_eq!(
            self.prior(&row.id),
            Some(row),
            "selected changed packet has its exact prior image"
        );
        let next = self
            .next(&row.id)
            .expect("selected changed metadata is retained");
        assert_ne!(row, next, "selected packet really changes");
    }
    fn assert_untouched(&self, row: &Row) {
        match self {
            Self::Full(_) => {
                assert_eq!(self.prior(&row.id), Some(row));
                assert_eq!(
                    self.next(&row.id),
                    Some(row),
                    "selected packet is logically untouched"
                );
            }
            Self::Delta(delta) => {
                assert!(
                    !delta.expected_packets().iter().any(|(id, _)| id == &row.id),
                    "untouched metadata is outside the delta expectations"
                );
                assert!(
                    !delta.upserts().iter().any(|r| r.id == row.id),
                    "untouched metadata is not upserted"
                );
                assert!(
                    !delta.deletes().contains(&row.id),
                    "untouched metadata is not deleted"
                );
            }
        }
    }
}

impl<S: SyncStorage> Storage for Racing<S> {
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
impl<S: SyncStorage> TransactionalStorage for Racing<S> {
    fn apply_batch(
        &mut self,
        m: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.apply_batch(m)
    }
}
impl<S: SyncStorage> TypedStorage for Racing<S> {
    fn query(
        &self,
        s: &Schema,
        q: &Query,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.inner.query(s, q)
    }
    fn query_validated(
        &self,
        query: ValidatedQuery<'_>,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.inner.query_validated(query)
    }
}
impl<S: SyncStorage> TypedTransactionalStorage for Racing<S> {
    fn apply_typed_batch(
        &mut self,
        s: &Schema,
        m: Vec<Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send {
        self.inner.apply_typed_batch(s, m)
    }
}
impl<S: SyncStorage> SchemaStorage for Racing<S> {
    fn catalog(&self) -> impl Future<Output = Result<ModelCatalog, StorageError>> + Send {
        self.inner.catalog()
    }
    fn scan(&self, t: &str) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.inner.scan(t)
    }
    fn apply_schema_plan(
        &mut self,
        p: SchemaPlan,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.apply_schema_plan(p)
    }
}
impl<S: SyncStorage> ObservationStorage for Racing<S> {
    fn project(
        &self,
        s: &Schema,
        q: &Query,
        f: &[String],
    ) -> impl Future<Output = Result<Vec<ProjectedRow>, StorageError>> + Send {
        self.inner.project(s, q, f)
    }
}
impl<S: SyncStorage> SyncStorage for Racing<S> {
    fn ownership(&self) -> StoreOwnership {
        self.ownership
    }
    fn reads_ready(&self) -> bool {
        self.inner.reads_ready()
    }
    fn load_sync(&self) -> Result<Option<Vec<Row>>, StorageError> {
        self.inner.load_sync()
    }
    fn apply_sync_plan(&mut self, plan: SyncPlan) -> Result<Vec<CommittedChange>, StorageError> {
        if std::mem::take(&mut self.control.lock().unwrap().capture_next) {
            return Err(capture_failure());
        }
        self.apply_fault(Images::Full(&plan))?;
        (self.before_apply)(&mut self.inner);
        self.inner.apply_sync_plan(plan)
    }
    fn apply_sync_delta(&mut self, delta: SyncDelta) -> Result<Vec<CommittedChange>, StorageError> {
        {
            let mut control = self.control.lock().unwrap();
            if std::mem::take(&mut control.capture_next) {
                assert!(control.captured.is_none());
                control.captured = Some(delta);
                return Err(capture_failure());
            }
        }
        self.apply_fault(Images::Delta(&delta))?;
        (self.before_apply)(&mut self.inner);
        self.inner.apply_sync_delta(delta)
    }
}
impl<S: SyncStorage> Racing<S> {
    /// Return the actual adapter for exact fixture inspection and strict reopen.
    pub fn into_inner(self) -> S {
        self.inner
    }

    fn apply_fault(&mut self, images: Images<'_>) -> Result<(), StorageError> {
        let armed = self.control.lock().unwrap().armed.take();
        if let Some(race) = armed {
            let before = snapshot(&self.inner);
            let (table, mut row) = match race {
                Race::GuardedRow => (
                    "notes",
                    run(self.inner.get("notes", "touched"))?.expect("guarded row exists"),
                ),
                Race::CommandHeader | Race::EntityHeader => {
                    let id = if matches!(race, Race::CommandHeader) {
                        "state"
                    } else {
                        "entity:state"
                    };
                    (SYNC_TABLE, run(self.inner.get(SYNC_TABLE, id))?.unwrap())
                }
                Race::ChangedMetadata | Race::UntouchedMalformed => {
                    let id = if matches!(race, Race::ChangedMetadata) {
                        "touched"
                    } else {
                        "untouched"
                    };
                    let row = before[SYNC_TABLE]
                        .iter()
                        .find(|row| {
                            row.id.starts_with("entity:metadata:")
                                && json(row).get("record_id") == Some(&Json::String(id.into()))
                        })
                        .expect("metadata exists")
                        .clone();
                    if matches!(race, Race::ChangedMetadata) {
                        images.assert_changed(&row);
                    } else {
                        images.assert_untouched(&row);
                    }
                    (SYNC_TABLE, row)
                }
            };
            match race {
                Race::UntouchedMalformed => {
                    row.fields.insert("data".into(), Value::String("{".into()));
                }
                Race::CommandHeader => edit(&mut row, |d| {
                    let Json::Integer(v) = d["last_ms"] else {
                        panic!("integer clock")
                    };
                    d.insert("last_ms".into(), Json::Integer(v + 1));
                }),
                Race::EntityHeader => edit(&mut row, |d| {
                    let Json::Integer(v) = d["revision"] else {
                        panic!("integer revision")
                    };
                    d.insert("revision".into(), Json::Integer(v + 1));
                }),
                Race::ChangedMetadata => edit(&mut row, |d| {
                    let Json::Object(metadata) = d.get_mut("metadata").unwrap() else {
                        panic!("metadata")
                    };
                    let Json::Object(fields) = metadata.get_mut("fields").unwrap() else {
                        panic!("fields")
                    };
                    let Json::Object(stamp) = fields.get_mut("body").unwrap() else {
                        panic!("stamp")
                    };
                    stamp.insert("timestamp_ms".into(), Json::Integer(11));
                }),
                Race::GuardedRow => {
                    assert!(
                        images.guards().iter().any(|guard| guard.table == "notes"
                            && guard.id == "touched"
                            && guard.expected.as_ref() == Some(&row)),
                        "changed row has exact storage guard"
                    );
                    row.fields
                        .insert("body".into(), Value::String("racing writer".into()));
                }
            }
            run(self.inner.replace(table, row.clone()))?;
            let raced = snapshot(&self.inner);
            assert_ne!(before, raced, "the out-of-band write occurred");
            self.control.lock().unwrap().attempt = Some(Attempt {
                before,
                raced,
                table: table.into(),
                row,
            });
        }
        Ok(())
    }
}
fn snapshot<S: SyncStorage>(store: &S) -> Snapshot {
    ["notes", MODEL_TABLE, SYNC_TABLE]
        .into_iter()
        .map(|table| {
            let mut rows = run(store.scan(table)).unwrap();
            rows.sort_by(|a, b| a.id.cmp(&b.id));
            (table.into(), rows)
        })
        .collect()
}
fn json(row: &Row) -> BTreeMap<String, Json> {
    let Value::String(value) = &row.fields["data"] else {
        panic!("packet string")
    };
    let Json::Object(data) = Json::parse(value).unwrap() else {
        panic!("packet object")
    };
    data
}
fn edit(row: &mut Row, change: impl FnOnce(&mut BTreeMap<String, Json>)) {
    let mut data = json(row);
    change(&mut data);
    row.fields.insert(
        "data".into(),
        Value::String(Json::Object(data).canonical().unwrap()),
    );
}
fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([(
            "notes".into(),
            TableDefinition {
                columns: BTreeMap::from([(
                    "body".into(),
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
fn open<S: SyncStorage>(store: S) -> Result<CommandDatabase<S>, CommandError> {
    run(CommandDatabase::open(
        store,
        schema(),
        vec![],
        [0x62; 16],
        SyncOptions::default(),
    ))
}
fn setup<S: SyncStorage>(
    inner: S,
    ownership: StoreOwnership,
) -> (CommandDatabase<Racing<S>>, Arc<Mutex<Control>>) {
    let mut store = hooked(inner, |_| {});
    store.ownership = ownership;
    let control = store.control.clone();
    let mut db = open(store).unwrap();
    db.configure_entities(
        EntityConfig {
            scope: "race".into(),
            tables: BTreeSet::from(["notes".into()]),
        },
        || {
            Ok(EntityContext {
                identity: Json::Null,
                now_ms: 100,
            })
        },
    )
    .unwrap();
    let request = db.begin_pull().unwrap().request;
    let changes = ["touched", "untouched"]
        .into_iter()
        .map(|id| EntityChange {
            table: "notes".into(),
            record_id: id.into(),
            op: EntityOperation::Update,
            changes: BTreeMap::from([("body".into(), Value::String(id.into()))]),
            removed_fields: BTreeSet::new(),
            timestamp_ms: 10,
            field_timestamps: BTreeMap::from([("body".into(), 10)]),
        })
        .collect();
    run(db.apply_pull(
        request,
        PullPage {
            checkpoint: "first".into(),
            complete: true,
            changes,
        },
    ))
    .unwrap();
    (db, control)
}
fn patch<S: SyncStorage>(db: &mut CommandDatabase<S>) -> Result<Row, CommandError> {
    run(db.patch(
        "notes",
        "touched",
        BTreeMap::from([("body".into(), Value::String("patched".into()))]),
    ))
}
fn good<S: SyncStorage>(fresh: &impl Fn() -> S, ownership: StoreOwnership) -> Snapshot {
    let (mut db, _) = setup(fresh(), ownership);
    patch(&mut db).unwrap();
    assert_eq!(db.queue_status().pending, 1);
    let storage = db.into_storage();
    let result = snapshot(&storage);
    assert!(open(storage.into_inner()).is_ok(), "clean control reopens");
    result
}
fn reject<S: SyncStorage>(fresh: &impl Fn() -> S, ownership: StoreOwnership, race: Race) {
    let (mut db, control) = setup(fresh(), ownership);
    let queue = db.queue_status();
    let query = db.query("notes");
    let subscription = run(db.observe(&query)).unwrap();
    db.drain_events(0);
    control.lock().unwrap().armed = Some(race);
    let result = patch(&mut db);
    let attempt = control
        .lock()
        .unwrap()
        .attempt
        .clone()
        .expect("fault reached real apply");
    assert!(
        result.is_err(),
        "guarded race must reject: {race:?} {ownership:?}"
    );
    assert!(db.queue_status() == queue, "owner queue must not advance");
    assert!(
        !db.drain_events(0).iter().any(
            |event| matches!(event,CommandEvent::Data(note) if note.subscription==subscription)
        ),
        "no committed data notification"
    );
    let storage = db.into_storage();
    let after = snapshot(&storage);
    assert_eq!(
        after, attempt.raced,
        "exact model, ledger, application rows keep only the race"
    );
    let mut expected = attempt.before.clone();
    let rows = expected.get_mut(&attempt.table).unwrap();
    *rows
        .iter_mut()
        .find(|row| row.id == attempt.row.id)
        .unwrap() = attempt.row.clone();
    assert_eq!(after, expected, "race changes exactly one raw row");
    if matches!(race, Race::UntouchedMalformed) {
        assert!(
            open(storage).is_err(),
            "strict reopen rejects corrupt untouched packet"
        );
    } else {
        assert!(open(storage).is_ok(), "valid racing image remains usable");
    }
}

pub fn untouched_exclusive<S: SyncStorage>(fresh: impl Fn() -> S) {
    let _lock = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    let mut expected = good(&fresh, StoreOwnership::Exclusive);
    let (mut db, control) = setup(fresh(), StoreOwnership::Exclusive);
    control.lock().unwrap().armed = Some(Race::UntouchedMalformed);
    let result = patch(&mut db);
    let attempt = control
        .lock()
        .unwrap()
        .attempt
        .clone()
        .expect("fault reached real apply");
    let storage = db.into_storage();
    let after = snapshot(&storage);
    // Always establish strict-open rejection even on the current RED path.
    assert!(
        open(storage).is_err(),
        "strict reopen must reject forged untouched metadata"
    );
    *expected
        .get_mut(&attempt.table)
        .unwrap()
        .iter_mut()
        .find(|row| row.id == attempt.row.id)
        .unwrap() = attempt.row.clone();
    assert!(
        result.is_ok(),
        "SI-Y03 Exclusive bounded delta must not inspect untouched metadata; current full path returns {result:?}"
    );
    assert_eq!(
        after, expected,
        "successful bounded write matches clean control plus the forged untouched packet"
    );
}
pub fn untouched_unproven<S: SyncStorage>(fresh: impl Fn() -> S) {
    let _lock = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    good(&fresh, StoreOwnership::Unproven);
    reject(&fresh, StoreOwnership::Unproven, Race::UntouchedMalformed);
}
pub fn guarded_races<S: SyncStorage>(fresh: impl Fn() -> S, ownership: StoreOwnership) {
    let _lock = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    good(&fresh, ownership);
    for race in [
        Race::CommandHeader,
        Race::EntityHeader,
        Race::ChangedMetadata,
        Race::GuardedRow,
    ] {
        reject(&fresh, ownership, race);
    }
}

fn capture_failure() -> StorageError {
    StorageError::BackendFailure {
        message: "owned test captured genuine delta".into(),
    }
}

fn capture<S: SyncStorage>(fresh: &impl Fn() -> S) -> (S, SyncDelta) {
    let (mut db, control) = setup(fresh(), StoreOwnership::Exclusive);
    control.lock().unwrap().capture_next = true;
    assert!(matches!(patch(&mut db), Err(CommandError::Storage(_))));
    assert_eq!(
        db.queue_status().pending,
        0,
        "injected pre-commit error retains owner state"
    );
    let proof = control
        .lock()
        .unwrap()
        .captured
        .take()
        .expect("Exclusive owner must supply a genuine delta proof");
    (db.into_storage().into_inner(), proof)
}

struct Conservative<S> {
    inner: S,
    full_prior: Option<Vec<Row>>,
    called: bool,
    race: bool,
    raced: Option<Snapshot>,
}
impl<S: SyncStorage> Storage for Conservative<S> {
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
impl<S: SyncStorage> TransactionalStorage for Conservative<S> {
    fn apply_batch(
        &mut self,
        m: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.apply_batch(m)
    }
}
impl<S: SyncStorage> TypedStorage for Conservative<S> {
    fn query(
        &self,
        s: &Schema,
        q: &Query,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.inner.query(s, q)
    }
    fn query_validated(
        &self,
        query: ValidatedQuery<'_>,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.inner.query_validated(query)
    }
}
impl<S: SyncStorage> TypedTransactionalStorage for Conservative<S> {
    fn apply_typed_batch(
        &mut self,
        s: &Schema,
        m: Vec<Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send {
        self.inner.apply_typed_batch(s, m)
    }
}
impl<S: SyncStorage> SchemaStorage for Conservative<S> {
    fn catalog(&self) -> impl Future<Output = Result<ModelCatalog, StorageError>> + Send {
        self.inner.catalog()
    }
    fn scan(&self, t: &str) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.inner.scan(t)
    }
    fn apply_schema_plan(
        &mut self,
        p: SchemaPlan,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.apply_schema_plan(p)
    }
}
impl<S: SyncStorage> ObservationStorage for Conservative<S> {
    fn project(
        &self,
        s: &Schema,
        q: &Query,
        f: &[String],
    ) -> impl Future<Output = Result<Vec<ProjectedRow>, StorageError>> + Send {
        self.inner.project(s, q, f)
    }
}

impl<S: SyncStorage> SyncStorage for Conservative<S> {
    fn reads_ready(&self) -> bool {
        self.inner.reads_ready()
    }
    fn load_sync(&self) -> Result<Option<Vec<Row>>, StorageError> {
        self.inner.load_sync()
    }
    fn apply_sync_plan(&mut self, plan: SyncPlan) -> Result<Vec<CommittedChange>, StorageError> {
        self.called = true;
        assert_eq!(
            plan.expected_sync, self.full_prior,
            "provided delta method supplies the FULL prior ledger"
        );
        if self.race {
            let before = snapshot(&self.inner);
            let mut row = before[SYNC_TABLE]
                .iter()
                .find(|row| {
                    row.id.starts_with("entity:metadata:")
                        && json(row).get("record_id") == Some(&Json::String("untouched".into()))
                })
                .unwrap()
                .clone();
            edit(&mut row, |data| {
                let Json::Object(metadata) = data.get_mut("metadata").unwrap() else {
                    unreachable!()
                };
                let Json::Object(fields) = metadata.get_mut("fields").unwrap() else {
                    unreachable!()
                };
                let Json::Object(stamp) = fields.get_mut("body").unwrap() else {
                    unreachable!()
                };
                stamp.insert("timestamp_ms".into(), Json::Integer(11));
            });
            run(self.inner.replace(SYNC_TABLE, row.clone()))?;
            let after = snapshot(&self.inner);
            assert_ne!(
                before, after,
                "valid untouched race happened after load_sync"
            );
            let mut expected = before;
            *expected
                .get_mut(SYNC_TABLE)
                .unwrap()
                .iter_mut()
                .find(|r| r.id == row.id)
                .unwrap() = row.clone();
            assert_eq!(after, expected, "race changes one exact raw packet");
            // This must remain a semantically valid ledger: rejection is full
            // CAS, not malformed payload detection.
            records::SyncLedger::from_rows(&after[SYNC_TABLE]).unwrap();
            self.raced = Some(after);
        }
        self.inner.apply_sync_plan(plan)
    }
    // Inherit Unproven ownership and the provided apply_sync_delta unchanged.
}

pub fn default_delta_matches_override<S: SyncStorage>(fresh: impl Fn() -> S) {
    let _lock = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    let (mut actual, proof) = capture(&fresh);
    let expected_packets = proof.expected_packets().to_vec();
    let upserts = proof.upserts().to_vec();
    let deletes = proof.deletes().to_vec();
    let changes = actual.apply_sync_delta(proof).unwrap();
    let expected = snapshot(&actual);
    assert!(
        open(actual).is_ok(),
        "concrete committed result strictly reopens"
    );
    let (inner, proof) = capture(&fresh);
    assert_eq!(proof.expected_packets(), expected_packets);
    assert_eq!(proof.upserts(), upserts);
    assert_eq!(
        proof.deletes(),
        deletes,
        "independently captured proofs describe the identical transition"
    );
    let full_prior = inner.load_sync().unwrap();
    assert!(
        full_prior
            .as_ref()
            .unwrap()
            .iter()
            .any(|r| !proof.expected_packets().iter().any(|(id, _)| id == &r.id)),
        "full prior contains untouched packets beyond the delta"
    );
    let mut conservative = Conservative {
        inner,
        full_prior,
        called: false,
        race: false,
        raced: None,
    };
    assert_eq!(conservative.ownership(), StoreOwnership::Unproven);
    assert_eq!(conservative.apply_sync_delta(proof).unwrap(), changes);
    assert!(
        conservative.called,
        "provided method delegates to full apply_sync_plan"
    );
    assert_eq!(snapshot(&conservative.inner), expected);
    assert!(
        open(conservative).is_ok(),
        "default committed result strictly reopens"
    );
}

pub fn default_delta_rejects_valid_untouched_race<S: SyncStorage>(fresh: impl Fn() -> S) {
    let _lock = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    let (inner, proof) = capture(&fresh);
    let full_prior = inner.load_sync().unwrap();
    let mut conservative = Conservative {
        inner,
        full_prior,
        called: false,
        race: true,
        raced: None,
    };
    assert_eq!(conservative.ownership(), StoreOwnership::Unproven);
    assert_eq!(
        conservative.apply_sync_delta(proof).unwrap_err(),
        StorageError::SchemaConflict {
            message: "command recovery changed before commit".into()
        },
        "full CAS rejects valid untouched race"
    );
    assert!(conservative.called);
    assert_eq!(
        snapshot(&conservative.inner),
        conservative.raced.take().unwrap(),
        "failed commit preserves only the valid race, exactly"
    );
    assert!(
        open(conservative).is_ok(),
        "valid racing state reopens and did not fail merely by parsing"
    );
}
