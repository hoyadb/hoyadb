//! TC-0041 SI-W03: the named R4 counters measure real work on unchanged
//! planners. Every workload runs on an entity owner whose ledger holds exactly
//! M metadata entries, one per pulled row.
use hoyadb_core::{command::entity::*, command::*, model::*, storage::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    task::{Context, Poll, Waker},
};

// TC-0043 GW18: independent Unproven fixtures share the process coordinator.
// Serialize those cases without changing their real admission behavior.
pub static UNPROVEN_OWNERS: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("synchronous adapter unexpectedly suspended"),
    }
}

fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([(
            "items".into(),
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

fn change(record_id: String, body: &str, time: i64) -> EntityChange {
    EntityChange {
        table: "items".into(),
        record_id,
        op: EntityOperation::Update,
        changes: BTreeMap::from([("body".into(), Value::String(body.into()))]),
        removed_fields: BTreeSet::new(),
        timestamp_ms: time,
        field_timestamps: BTreeMap::from([("body".into(), time)]),
    }
}

/// The id of the k-th pulled row.
pub fn id(k: usize) -> String {
    format!("r{k:04}")
}

/// An entity owner with M pulled rows, so M metadata entries.
pub fn owner<S: SyncStorage>(store: S, m: usize) -> CommandDatabase<S> {
    let mut db = run(CommandDatabase::open(
        store,
        schema(),
        vec![],
        [0x5d; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    db.configure_entities(
        EntityConfig {
            scope: "counters".into(),
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
    let request = db.begin_pull().unwrap().request;
    run(db.apply_pull(
        request,
        PullPage {
            checkpoint: "cp1".into(),
            complete: true,
            changes: (0..m).map(|k| change(id(k), "pulled", 10)).collect(),
        },
    ))
    .unwrap();
    assert_eq!(db.sync_progress().changes, m as i64);
    db
}

/// One entity patch (c = 1) through today's planner and full commit.
pub fn patch_work<S: SyncStorage>(store: S, m: usize) -> counters::WorkSnapshot {
    let mut db = owner(store, m);
    counters::reset();
    run(db.patch(
        "items",
        &id(0),
        BTreeMap::from([("body".into(), Value::String("patched".into()))]),
    ))
    .unwrap();
    counters::snapshot()
}

/// One progress read uses the owner's independently verified tombstone cache.
pub fn progress_work<S: SyncStorage>(store: S, m: usize) -> counters::WorkSnapshot {
    let db = owner(store, m);
    counters::reset();
    assert_eq!(db.sync_progress().tombstones, 0);
    counters::snapshot()
}

/// An explicit reset, which keeps its full plan (SI-Y06).
pub fn reset_work<S: SyncStorage>(store: S, m: usize) -> counters::WorkSnapshot {
    let mut db = owner(store, m);
    counters::reset();
    run(db.reset(ResetMode::DiscardRecovery)).unwrap();
    counters::snapshot()
}

/// SI-W03/R9 transition: the configured fixture has state, entity:state and a
/// command packet. A delta clones three expectations and one changed command;
/// Unproven retains both complete three-packet SyncPlan images.
pub fn receipt_packet_copies<S: SyncStorage>(store: S, full: bool) {
    let mut db = owner(store, 0);
    let request = db.request().unwrap();
    assert!(
        run(db.begin(
            request,
            CommandDefinition {
                name: "receipt-v1".into(),
                arguments: BTreeMap::new(),
                reference_paths: vec![],
            },
            Json::Object(BTreeMap::new()),
            Some(Json::Null),
            20,
        ))
        .unwrap()
    );
    let key = run(db.finish(request, Json::Null)).unwrap().key;
    assert_eq!(db.prepare_push(20, 0).unwrap().envelopes.len(), 1);
    counters::reset();
    db.receive(&key, None).unwrap();
    let work = counters::snapshot();
    assert_eq!(db.queue_status().commands[0].state, SyncState::Acknowledged);
    let minimum = if full {
        6
    } else {
        4
    };
    assert!(
        work.rows_cloned >= minimum,
        "configured receipt must count its exact full/delta images: {work:?}"
    );
}

/// Reset has two header packets before and after, and deliberately keeps its
/// full plan. Adapter-local copies may add work above this core-only minimum.
pub fn reset_packet_copies<S: SyncStorage>(store: S) {
    let work = reset_work(store, 0);
    assert!(
        work.rows_cloned >= 4,
        "reset's two complete two-packet images must be counted: {work:?}"
    );
}

/// An empty Exclusive page retains no row or packet inventory. Unproven keeps
/// the full application snapshot and M metadata packets plus two header packets.
pub fn merge_packet_copies<S: SyncStorage>(store: S, m: usize, full: bool) {
    let mut db = owner(store, m);
    let request = db.begin_pull().unwrap().request;
    counters::reset();
    let step = run(db.merge_prepare(
        request,
        PullPage {
            checkpoint: "cp2".into(),
            complete: true,
            changes: vec![],
        },
        BTreeSet::new(),
    ))
    .unwrap();
    let work = counters::snapshot();
    assert_eq!(
        work.rows_cloned,
        if full {
            2 * m as u64 + 2
        } else {
            0
        },
        "session packet and row images must count every copied Row: {work:?}"
    );
    let session = match step {
        MergeStep::Ready {
            session,
            ..
        } => session,
        _ => panic!("an empty page must be ready without a callback"),
    };
    db.merge_abort(session).unwrap();
}

/// SI-Y01: the observation seam must report real first images, not counts
/// inferred from an expected patch. Compare exact touched identities at two M.
pub fn patch_first_images<S: SyncStorage>(store: S, m: usize) {
    let mut db = owner(store, m);
    run(db.patch(
        "items",
        &id(0),
        BTreeMap::from([("body".into(), Value::String("patched".into()))]),
    ))
    .unwrap();
    let observed = db.ledger_work();
    let queued = db.queue_status();
    assert_eq!(queued.commands.len(), 1);
    assert_eq!(
        observed.record_keys,
        BTreeSet::from([queued.commands[0].key.clone()])
    );
    assert_eq!(
        observed.metadata_keys,
        BTreeSet::from([("items".into(), id(0))])
    );
}

fn deletion_work<S: SyncStorage>(store: S) -> (u64, counters::LedgerWorkState, i64) {
    let mut db = owner(store, 10);
    let before = db.ledger_work().epoch;
    assert!(run(db.remove("items", &id(0))).unwrap());
    let observed = db.ledger_work();
    // Outside the measured operation: this is intentionally an O(M) invariant.
    (before, observed, db.ledger_tombstone_count())
}

/// SI-Y01/SI-Y09: an actual committed deletion advances the owner epoch.
/// This behavioral RED is independent of the cache observation.
pub fn epoch_advances<S: SyncStorage>(store: S) {
    let (before, observed, _) = deletion_work(store);
    assert_eq!(observed.epoch, before + 1);
}

pub fn tombstone_cache<S: SyncStorage>(store: S) {
    let (_, observed, recomputed) = deletion_work(store);
    assert_eq!(recomputed, 1);
    assert_eq!(observed.cached_tombstones, Some(1));
    assert_eq!(observed.cached_tombstones, Some(recomputed));
}

pub fn assert_patch_clones_bounded(small: &counters::WorkSnapshot, large: &counters::WorkSnapshot) {
    assert_eq!(
        small.ledger_entries_cloned, large.ledger_entries_cloned,
        "SI-Y01 c=1 clones must be independent of M: M=10 {small:?}, M=1000 {large:?}"
    );
}

/// The named counters of the full-metadata patch commit are nonzero, and
/// grow by at least one entry per extra metadata entry (M = 10 versus 1000).
pub fn assert_unproven_patch_counters_grow(
    small: &counters::WorkSnapshot,
    large: &counters::WorkSnapshot,
) {
    let context = format!("M=10 {small:?}\nM=1000 {large:?}");
    eprintln!("SI-W03 patch {context}");
    for (name, at_10, at_1000) in [
        (
            "metadata_entries_visited",
            small.metadata_entries_visited,
            large.metadata_entries_visited,
        ),
        (
            "packets_encoded",
            small.packets_encoded,
            large.packets_encoded,
        ),
        ("packets_parsed", small.packets_parsed, large.packets_parsed),
    ] {
        assert!(at_10 > 0, "{name} is vacuous: {context}");
        assert!(
            at_1000 >= at_10 + 990,
            "{name} does not grow with M: {context}"
        );
    }
    assert!(
        small.rows_validated > 0,
        "rows_validated is vacuous: {context}"
    );
}

/// SI-Y07: one terminal entity confirmation or inverse reads only its row closure.
pub fn resolution_work<S: SyncStorage>(
    store: S,
    m: usize,
    rollback: bool,
) -> counters::WorkSnapshot {
    let mut db = owner(store, m);
    run(db.patch(
        "items",
        &id(0),
        BTreeMap::from([("body".into(), Value::String("patched".into()))]),
    ))
    .unwrap();
    let key = db.queue_status().commands[0].key.clone();
    db.prepare_push(20, 0).unwrap();
    db.receive(
        &key,
        Some(if rollback {
            RemoteOutcome::Rejected("no".into())
        } else {
            RemoteOutcome::Applied(Json::Null)
        }),
    )
    .unwrap();
    if rollback {
        run(db.reconcile(&key, ReconcilePlan::default())).unwrap();
    }
    counters::reset();
    if rollback {
        run(db.rollback(&key)).unwrap();
    } else {
        run(db.reconcile(&key, ReconcilePlan::default())).unwrap();
    }
    counters::snapshot()
}

pub fn assert_resolution_bounded(small: &counters::WorkSnapshot, large: &counters::WorkSnapshot) {
    eprintln!("SI-Y07 M10={small:?} M1000={large:?}");
    for (name, left, right) in [
        ("rows_cloned", small.rows_cloned, large.rows_cloned),
        (
            "ledger_entries_cloned",
            small.ledger_entries_cloned,
            large.ledger_entries_cloned,
        ),
        (
            "metadata_entries_visited",
            small.metadata_entries_visited,
            large.metadata_entries_visited,
        ),
        (
            "packets_encoded",
            small.packets_encoded,
            large.packets_encoded,
        ),
        ("packets_parsed", small.packets_parsed, large.packets_parsed),
        ("rows_validated", small.rows_validated, large.rows_validated),
        (
            "memory_rows_visited",
            small.memory_rows_visited,
            large.memory_rows_visited,
        ),
        (
            "sqlite_rows_read",
            small.sqlite_rows_read,
            large.sqlite_rows_read,
        ),
        (
            "sqlite_rows_written",
            small.sqlite_rows_written,
            large.sqlite_rows_written,
        ),
    ] {
        assert_eq!(left, right, "{name} must be bounded by the touched closure");
    }
    assert_eq!(small.sqlite_fullscan_steps, 0);
    assert_eq!(large.sqlite_fullscan_steps, 0);
}

/// A custom adapter that inherits the provided `ownership()`.
#[derive(Default)]
pub struct Unproven<S>(pub S);

impl<S: SyncStorage> Storage for Unproven<S> {
    fn create_table(
        &mut self,
        name: &str,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.0.create_table(name)
    }

    fn insert(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.0.insert(table, row)
    }

    fn get(
        &self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<Option<Row>, StorageError>> + Send {
        self.0.get(table, id)
    }

    fn replace(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.0.replace(table, row)
    }

    fn remove(
        &mut self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send {
        self.0.remove(table, id)
    }
}

impl<S: SyncStorage> TransactionalStorage for Unproven<S> {
    fn apply_batch(
        &mut self,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.0.apply_batch(mutations)
    }
}

impl<S: SyncStorage> TypedStorage for Unproven<S> {
    fn query(
        &self,
        schema: &Schema,
        query: &Query,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.0.query(schema, query)
    }
}

impl<S: SyncStorage> TypedTransactionalStorage for Unproven<S> {
    fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send {
        self.0.apply_typed_batch(schema, mutations)
    }
}

impl<S: SyncStorage> SchemaStorage for Unproven<S> {
    fn catalog(&self) -> impl Future<Output = Result<ModelCatalog, StorageError>> + Send {
        self.0.catalog()
    }

    fn scan(&self, table: &str) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.0.scan(table)
    }

    fn apply_schema_plan(
        &mut self,
        plan: SchemaPlan,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.0.apply_schema_plan(plan)
    }
}

impl<S: SyncStorage> ObservationStorage for Unproven<S> {
    fn project(
        &self,
        schema: &Schema,
        query: &Query,
        fields: &[String],
    ) -> impl Future<Output = Result<Vec<ProjectedRow>, StorageError>> + Send {
        self.0.project(schema, query, fields)
    }
}

impl<S: SyncStorage> SyncStorage for Unproven<S> {
    fn load_sync(&self) -> Result<Option<Vec<Row>>, StorageError> {
        self.0.load_sync()
    }

    fn apply_sync_plan(&mut self, plan: SyncPlan) -> Result<Vec<CommittedChange>, StorageError> {
        self.0.apply_sync_plan(plan)
    }
}

pub fn assert_patch_bounded(small: &counters::WorkSnapshot, large: &counters::WorkSnapshot) {
    assert_resolution_bounded(small, large);
    assert!(small.ledger_entries_cloned >= 1);
    assert!(
        small.packets_encoded >= 3,
        "header, touched record and metadata must encode"
    );
}

pub fn cache_lifecycle<S: SyncStorage>(store: S) {
    let mut db = owner(store, 10);
    let check = |db: &CommandDatabase<S>, expected| {
        assert_eq!(db.ledger_work().cached_tombstones, Some(expected));
        assert_eq!(db.ledger_tombstone_count(), expected);
    };
    run(db.remove("items", &id(0))).unwrap();
    check(&db, 1);
    let key = db.queue_status().commands[0].key.clone();
    db.prepare_push(20, 0).unwrap();
    db.receive(&key, Some(RemoteOutcome::Rejected("no".into())))
        .unwrap();
    run(db.reconcile(&key, ReconcilePlan::default())).unwrap();
    run(db.rollback(&key)).unwrap();
    check(&db, 0);
    let deletion = |id| EntityChange {
        table: "items".into(),
        record_id: id,
        op: EntityOperation::Delete,
        changes: BTreeMap::new(),
        removed_fields: BTreeSet::new(),
        timestamp_ms: 30,
        field_timestamps: BTreeMap::new(),
    };
    let request = db.begin_pull().unwrap().request;
    run(db.apply_pull(
        request,
        PullPage {
            checkpoint: "cp2".into(),
            complete: true,
            changes: vec![deletion(id(1))],
        },
    ))
    .unwrap();
    check(&db, 1);
    let epoch = db.ledger_work().epoch;
    let request = db.begin_pull().unwrap().request;
    let mut invalid = change(id(3), "bad", 40);
    invalid
        .changes
        .insert("unknown".into(), Value::String("bad".into()));
    invalid.field_timestamps.insert("unknown".into(), 40);
    assert!(
        run(db.apply_pull(
            request,
            PullPage {
                checkpoint: "bad".into(),
                complete: true,
                changes: vec![deletion(id(2)), invalid]
            }
        ))
        .is_err()
    );
    check(&db, 1);
    assert_eq!(db.ledger_work().epoch, epoch);
    run(db.reset(ResetMode::DiscardRecovery)).unwrap();
    check(&db, 0);
    assert_eq!(db.ledger_work().epoch, epoch + 1);
}
