//! SPEC-0039 / TC-0043 GW14: an Unproven adapter whose guarded row changes between the
//! planner snapshot and the atomic apply. No-op expectations (an equal-value Put, an absent
//! Remove) are storage guards, so the stale expectation is a RollbackConflict and nothing commits.
#[allow(dead_code)]
#[path = "command_guard_api.rs"]
mod api;
use api::*;
use hoyadb_core::{command::*, model::ROW_VERSION, storage::*};
use std::{
    future::Future,
    sync::{Arc, Mutex},
};

// TC-0043 GW18: these wrappers intentionally use the global Unproven coordinator.
// Isolate independent test cases; the deliberate two-handle race within a case is unchanged.
static OWNERS: Mutex<()> = Mutex::new(());

/// A concurrent writer's change, applied to the inner store just before the next sync commit,
/// or (GW17) an injected storage failure returned instead of committing.
pub enum Race {
    Replace(&'static str, Row),
    Insert(&'static str, Row),
    Fail(&'static str),
}

pub type Armed = Arc<Mutex<Option<Race>>>;
pub type Seen = Arc<Mutex<Vec<SyncPlan>>>;

pub struct Racing<S> {
    inner: S,
    race: Armed,
    seen: Seen,
}

impl<S: SyncStorage> Racing<S> {
    pub fn new(inner: S) -> (Self, Armed) {
        let (store, race, _) = Self::recording(inner);
        (store, race)
    }

    /// Also records every SyncPlan the core submits.
    pub fn recording(inner: S) -> (Self, Armed, Seen) {
        let race = Arc::new(Mutex::new(None));
        let seen = Arc::new(Mutex::new(vec![]));
        (
            Self {
                inner,
                race: race.clone(),
                seen: seen.clone(),
            },
            race,
            seen,
        )
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
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.apply_batch(mutations)
    }
}

impl<S: SyncStorage> TypedStorage for Racing<S> {
    fn query(
        &self,
        schema: &Schema,
        query: &Query,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.inner.query(schema, query)
    }
}

impl<S: SyncStorage> TypedTransactionalStorage for Racing<S> {
    fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send {
        self.inner.apply_typed_batch(schema, mutations)
    }
}

impl<S: SyncStorage> SchemaStorage for Racing<S> {
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

impl<S: SyncStorage> ObservationStorage for Racing<S> {
    fn project(
        &self,
        schema: &Schema,
        query: &Query,
        fields: &[String],
    ) -> impl Future<Output = Result<Vec<ProjectedRow>, StorageError>> + Send {
        self.inner.project(schema, query, fields)
    }
}

impl<S: SyncStorage> SyncStorage for Racing<S> {
    fn load_sync(&self) -> Result<Option<Vec<Row>>, StorageError> {
        self.inner.load_sync()
    }
    fn apply_sync_plan(&mut self, plan: SyncPlan) -> Result<Vec<CommittedChange>, StorageError> {
        self.seen.lock().unwrap().push(plan.clone());
        if let Some(race) = self.race.lock().unwrap().take() {
            match race {
                Race::Replace(table, row) => run(self.inner.replace(table, row))?,
                Race::Insert(table, row) => run(self.inner.insert(table, row))?,
                Race::Fail(message) => {
                    return Err(StorageError::SchemaConflict {
                        message: message.into(),
                    });
                }
            }
        }
        self.inner.apply_sync_plan(plan)
    }
    // Inherits StoreOwnership::Unproven: another writer may reach these rows.
}

fn raw(id: &str, value: &str) -> Row {
    let mut fields = body(value);
    fields.insert(ROW_VERSION.into(), Value::Integer(1));
    Row {
        id: id.into(),
        fields,
    }
}

fn notes<S: SyncStorage>(db: &CommandDatabase<S>) -> Vec<Row> {
    run(db.query("notes").fetch(db)).unwrap()
}

/// GW14 on one adapter: `fresh` yields an empty inner store per scenario.
pub fn gw14<S: SyncStorage>(fresh: impl Fn() -> S) {
    let _owners = OWNERS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_eq!(Racing::new(fresh()).0.ownership(), StoreOwnership::Unproven);
    for (name, plan, guard, race) in [
        (
            "equal-value Put",
            writes(vec![put("other-note", "other")]),
            expect("other-note", Some("other")),
            Race::Replace("notes", raw("other-note", "raced")),
        ),
        (
            "absent no-op Remove",
            writes(vec![remove("never-existed")]),
            expect("never-existed", None),
            Race::Insert("notes", raw("never-existed", "raced")),
        ),
    ] {
        let (store, armed) = Racing::new(fresh());
        let (mut db, a) = corrected(store);
        let pending = db.pending_reconciliations();
        let status = db.queue_status();
        let ledger = {
            let storage = db.into_storage();
            let rows = storage.load_sync().unwrap();
            db = open(storage);
            rows
        };
        *armed.lock().unwrap() = Some(race);
        assert_eq!(
            run(db.reconcile_guarded(&a, plan.clone(), vec![guard.clone()])),
            Err(CommandError::RollbackConflict),
            "{name}"
        );
        assert!(armed.lock().unwrap().is_none(), "{name}: race happened");
        assert!(
            db.pending_reconciliations() == pending,
            "{name}: still pending"
        );
        assert!(db.queue_status() == status, "{name}: status unchanged");
        let raced = notes(&db);
        let storage = db.into_storage();
        assert_eq!(
            storage.load_sync().unwrap(),
            ledger,
            "{name}: ledger unchanged"
        );
        let db = open(storage);
        assert_eq!(
            notes(&db),
            raced,
            "{name}: only the racing write is visible"
        );
        // Control: without a race the same plan and expectation commit.
        let (store, _) = Racing::new(fresh());
        let (mut clean, a2) = corrected(store);
        assert_eq!(
            run(clean.reconcile_guarded(&a2, plan, vec![guard])),
            Ok(()),
            "{name}: control"
        );
        clean.close();
        db.close();
    }
}

/// GW05 on both adapters: an identical guarded Put is committed as a storage expectation only:
/// the submitted plan guards the row with its exact raw image, carries no mutation for it, and
/// publishes no data notification.
pub fn gw05_identical_put_is_expectation_only<S: SyncStorage>(fresh: impl Fn() -> S) {
    let _owners = OWNERS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (store, _, seen) = Racing::recording(fresh());
    let (mut db, a) = corrected(store);
    let query = db.query("notes");
    let data = run(db.observe(&query)).unwrap();
    db.drain_events(0);
    let before = notes(&db);
    seen.lock().unwrap().clear();
    assert_eq!(
        run(db.reconcile_guarded(
            &a,
            writes(vec![put("other-note", "other")]),
            vec![expect("other-note", Some("other"))],
        )),
        Ok(())
    );
    let plans = seen.lock().unwrap();
    assert_eq!(plans.len(), 1, "one atomic commit");
    let plan = &plans[0];
    assert!(
        plan.guards.contains(&RowGuard {
            table: "notes".into(),
            id: "other-note".into(),
            expected: Some(raw("other-note", "other")),
        }),
        "the expectation is a storage guard"
    );
    assert!(
        !plan.mutations.iter().any(|m| matches!(m,
            Mutation::Insert { row, .. } | Mutation::Replace { row, .. } if row.id == "other-note")
            || matches!(m, Mutation::Remove { id, .. } if id == "other-note")),
        "no application mutation for the identical row"
    );
    drop(plans);
    assert_eq!(notes(&db), before);
    assert!(
        !db.drain_events(0)
            .iter()
            .any(|e| matches!(e, CommandEvent::Data(n) if n.subscription == data)),
        "no data notification"
    );
    db.close();
}

/// GW17: only the exact storage guard conflict becomes RollbackConflict; any other storage
/// SchemaConflict at atomic apply stays a Storage error, for guarded and ordinary reconcile,
/// with rows, ledger, metadata, status, pending outcome and notifications unchanged.
pub fn gw17<S: SyncStorage>(fresh: impl Fn() -> S) {
    let _owners = OWNERS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    const OTHER: &str = "command model changed before commit";
    let guarded = || {
        (
            writes(vec![put("other-note", "backend")]),
            vec![expect("other-note", Some("other"))],
        )
    };
    for (message, ordinary) in [
        (OTHER, false),
        (OTHER, true),
        (GUARD_CONFLICT, false),
        (GUARD_CONFLICT, true),
    ] {
        let (store, armed) = Racing::new(fresh());
        let (db, a) = corrected(store);
        let (mut db, before) = capture(db);
        let query = db.query("notes");
        let data = run(db.observe(&query)).unwrap();
        let outcome = db.observe_outcome(&a).unwrap();
        db.drain_events(0);
        *armed.lock().unwrap() = Some(Race::Fail(message));
        let result = if ordinary {
            run(db.reconcile(&a, ReconcilePlan::default()))
        } else {
            let (plan, guards) = guarded();
            run(db.reconcile_guarded(&a, plan, guards))
        };
        let expected = if message == GUARD_CONFLICT {
            Err(CommandError::RollbackConflict)
        } else {
            Err(CommandError::Storage(StorageError::SchemaConflict {
                message: message.into(),
            }))
        };
        assert_eq!(result, expected, "{message} ordinary={ordinary}");
        assert!(armed.lock().unwrap().is_none(), "the failure was injected");
        let events = db.drain_events(0);
        assert!(
            !events.iter().any(|e| matches!(e, CommandEvent::Data(n) if n.subscription == data)
                || matches!(e, CommandEvent::Outcome { subscription, .. } if *subscription == outcome)),
            "{message}: no notification"
        );
        let db = assert_unchanged(db, &before);
        db.close();
    }
}
