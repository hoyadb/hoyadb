//! API-0006 section 3: owned FIFO admission and private staged views.
use super::overlay::{Base, Delta, DeltaRef, Overlay};
use crate::{model::*, storage::*};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
};

#[derive(Clone)]
pub(crate) struct ExecutionCoordinator(Arc<Mutex<State>>);

// SPEC-0037 R3.2: identities are unique across every coordinator in the process.
static NEXT_OWNER: AtomicU64 = AtomicU64::new(0);
static NEXT_REQUEST: AtomicU64 = AtomicU64::new(0);

fn allocate(counter: &AtomicU64, exhausted: &str) -> Result<u64, StorageError> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .map(|previous| previous + 1)
        .map_err(|_| failure(exhausted))
}

#[derive(Default)]
struct State {
    owners: BTreeSet<u64>,
    queue: VecDeque<ExecutionRequest>,
    active: Option<ExecutionRequest>,
}

pub(crate) struct ExecutionOwner {
    coordinator: ExecutionCoordinator,
    id: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExecutionRequest {
    owner: u64,
    id: u64,
}

pub(crate) struct ExecutionPermit {
    coordinator: ExecutionCoordinator,
    request: ExecutionRequest,
}

/// SPEC-0037 R6: a command scope stages a delta over an immutable base. An
/// Exclusive validated owner stages over its own store and copies nothing at
/// begin; any other owner stages over the validated snapshot copied at begin.
pub(crate) struct StagedScope {
    snapshot: Option<SnapshotStorage>,
    delta: Delta,
    parts: Option<Parts>,
    validated: bool,
    permit: ExecutionPermit,
}

/// The staged database state carried between scope calls.
pub(crate) struct Parts {
    schema: SchemaDefinition,
    projection: Schema,
    state: ModelState,
    generator: Option<IdGenerator>,
    journal: Vec<CommittedChange>,
}

/// One scope call's writable staged database. Dropping it, including when the
/// call's future is dropped, returns the staged state to the scope.
pub(crate) struct Lease<'a, S> {
    db: Option<Database<Overlay<'a, S>>>,
    parts: &'a mut Option<Parts>,
}

impl<'a, S> std::ops::Deref for Lease<'a, S> {
    type Target = Database<Overlay<'a, S>>;

    fn deref(&self) -> &Self::Target {
        self.db.as_ref().expect("live lease")
    }
}

impl<S> std::ops::DerefMut for Lease<'_, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.db.as_mut().expect("live lease")
    }
}

impl<S> Drop for Lease<'_, S> {
    fn drop(&mut self) {
        if let Some(db) = self.db.take() {
            *self.parts = Some(Parts {
                schema: db.schema,
                projection: db.projection,
                state: db.state,
                generator: db.generator,
                journal: db.journal.unwrap_or_default(),
            });
        }
    }
}

pub(crate) struct PreparedScope {
    pub(crate) permit: ExecutionPermit,
    pub(crate) mutations: Vec<Mutation>,
    pub(crate) changes: Vec<CommittedChange>,
    pub(crate) state: ModelState,
}

fn failure(message: &str) -> StorageError {
    StorageError::BackendFailure {
        message: message.into(),
    }
}

impl ExecutionCoordinator {
    pub(crate) fn global() -> Self {
        static GLOBAL: OnceLock<Arc<Mutex<State>>> = OnceLock::new();
        Self(
            GLOBAL
                .get_or_init(|| Arc::new(Mutex::new(State::default())))
                .clone(),
        )
    }

    /// SPEC-0037 R3.1: per-database admission only where exclusivity is proven.
    pub(crate) fn for_database(ownership: StoreOwnership, target_is_wasm: bool) -> Self {
        if ownership == StoreOwnership::Exclusive && !target_is_wasm {
            Self(Arc::new(Mutex::new(State::default())))
        } else {
            Self::global()
        }
    }

    #[cfg(test)]
    fn isolated() -> Self {
        Self(Arc::new(Mutex::new(State::default())))
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, State>, StorageError> {
        self.0
            .lock()
            .map_err(|_| failure("execution coordinator is unavailable"))
    }

    pub(crate) fn owner(&self) -> Result<ExecutionOwner, StorageError> {
        let mut state = self.lock()?;
        let id = allocate(&NEXT_OWNER, "execution owner IDs exhausted")?;
        state.owners.insert(id);
        Ok(ExecutionOwner {
            coordinator: self.clone(),
            id,
        })
    }
}

impl ExecutionOwner {
    pub(crate) fn validate_request(&self, request: ExecutionRequest) -> Result<(), StorageError> {
        let state = self.coordinator.lock()?;
        if request.owner != self.id
            || !state.owners.contains(&self.id)
            || !state.queue.contains(&request)
        {
            return Err(failure("invalid execution request"));
        }
        Ok(())
    }

    pub(crate) fn request(&self) -> Result<ExecutionRequest, StorageError> {
        let mut state = self.coordinator.lock()?;
        if !state.owners.contains(&self.id) {
            return Err(failure("execution owner is closed"));
        }
        let id = allocate(&NEXT_REQUEST, "execution request IDs exhausted")?;
        let request = ExecutionRequest {
            owner: self.id,
            id,
        };
        state.queue.push_back(request);
        Ok(request)
    }

    pub(crate) fn admit(
        &self,
        request: ExecutionRequest,
    ) -> Result<Option<ExecutionPermit>, StorageError> {
        let mut state = self.coordinator.lock()?;
        if request.owner != self.id
            || !state.owners.contains(&self.id)
            || !state.queue.contains(&request)
        {
            return Err(failure("invalid execution request"));
        }
        if state.active.is_some() || state.queue.front() != Some(&request) {
            return Ok(None);
        }
        state.queue.pop_front();
        state.active = Some(request);
        Ok(Some(ExecutionPermit {
            coordinator: self.coordinator.clone(),
            request,
        }))
    }

    pub(crate) fn cancel(&self, request: ExecutionRequest) -> Result<(), StorageError> {
        let mut state = self.coordinator.lock()?;
        if request.owner != self.id
            || !state.owners.contains(&self.id)
            || !state.queue.contains(&request)
        {
            return Err(failure("invalid execution request"));
        }
        state.queue.retain(|queued| queued != &request);
        Ok(())
    }

    pub(crate) fn idle_permit(&self) -> Result<ExecutionPermit, StorageError> {
        let mut state = self.coordinator.lock()?;
        if !state.owners.contains(&self.id) {
            return Err(failure("execution owner is closed"));
        }
        if state.active.is_some() || !state.queue.is_empty() {
            return Err(failure("execution coordinator is busy"));
        }
        let id = allocate(&NEXT_REQUEST, "execution request IDs exhausted")?;
        let request = ExecutionRequest {
            owner: self.id,
            id,
        };
        state.active = Some(request);
        Ok(ExecutionPermit {
            coordinator: self.coordinator.clone(),
            request,
        })
    }
}

impl Drop for ExecutionOwner {
    fn drop(&mut self) {
        let mut state = self
            .coordinator
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.owners.remove(&self.id);
        state.queue.retain(|request| request.owner != self.id);
        if state.active.is_some_and(|request| request.owner == self.id) {
            state.active = None;
        }
    }
}

impl ExecutionPermit {
    pub(crate) fn validate(&self) -> Result<(), StorageError> {
        let state = self.coordinator.lock()?;
        if state.active != Some(self.request) || !state.owners.contains(&self.request.owner) {
            return Err(failure("execution ticket is no longer active"));
        }
        Ok(())
    }
}

impl Drop for ExecutionPermit {
    fn drop(&mut self) {
        let mut state = self
            .coordinator
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.active == Some(self.request) {
            state.active = None;
        }
    }
}

impl StagedScope {
    pub(crate) async fn begin<S: SyncStorage>(
        db: &Database<S>,
        permit: ExecutionPermit,
    ) -> Result<Self, StorageError> {
        permit.validate()?;
        // R5 validated an Exclusive owner's whole inventory at open; if its
        // reads also complete on first poll, its scope stages against the live
        // store and copies and validates nothing. Every other owner, including
        // a yielding Exclusive adapter, awaits today's per-begin snapshot and
        // inventory validation and stages against that private copy.
        let live = db.validated_exclusive && db.storage.reads_ready();
        let snapshot = if live {
            None
        } else {
            let mut snapshot = SnapshotStorage::default();
            let mut inventory = BTreeMap::new();
            for table in db.schema.tables.keys() {
                let rows = db.storage.scan(table).await?;
                snapshot.create_table(table).await?;
                for row in &rows {
                    count_work!(rows_cloned, 1);
                    snapshot.insert(table, row.clone()).await?;
                }
                inventory.insert(table.clone(), rows);
            }
            validate_model_inventory(&db.projection, db.state.version, &inventory)?;
            snapshot.create_table(MODEL_TABLE).await?;
            snapshot
                .insert(MODEL_TABLE, model_state_row(&db.state)?)
                .await?;
            Some(snapshot)
        };
        permit.validate()?;
        Ok(Self {
            snapshot,
            delta: Delta::default(),
            parts: Some(Parts {
                schema: db.schema.clone(),
                projection: db.projection.clone(),
                state: db.state.clone(),
                generator: db.generator.clone(),
                journal: vec![],
            }),
            // A private snapshot base never receives a validated-query proof.
            validated: live,
            permit,
        })
    }

    pub(crate) fn validate(&self) -> Result<(), StorageError> {
        self.permit.validate()
    }

    fn parts(&self) -> Result<&Parts, StorageError> {
        self.permit.validate()?;
        self.parts
            .as_ref()
            .ok_or_else(|| failure("command scope is already in use"))
    }

    pub(crate) fn state(&self) -> Result<&ModelState, StorageError> {
        Ok(&self.parts()?.state)
    }

    pub(crate) fn journal(&self) -> Result<&[CommittedChange], StorageError> {
        Ok(&self.parts()?.journal)
    }

    /// A writable staged database over `base` (the owner's store) for one call.
    pub(crate) fn enter<'a, S: SyncStorage>(
        &'a mut self,
        base: &'a S,
    ) -> Result<Lease<'a, S>, StorageError> {
        self.permit.validate()?;
        let parts = self
            .parts
            .take()
            .ok_or_else(|| failure("command scope is already in use"))?;
        let base = match &self.snapshot {
            Some(snapshot) => Base::Snapshot(snapshot),
            None => Base::Store(base),
        };
        Ok(Lease {
            db: Some(Database {
                storage: Overlay {
                    base,
                    delta: DeltaRef::Write(&mut self.delta),
                    validated: self.validated,
                },
                schema: parts.schema,
                projection: parts.projection,
                state: parts.state,
                generator: parts.generator,
                journal: Some(parts.journal),
                validated_exclusive: false,
            }),
            parts: &mut self.parts,
        })
    }

    /// A read-only staged database over `base` for one call.
    pub(crate) fn reader<'a, S: SyncStorage>(
        &'a self,
        base: &'a S,
    ) -> Result<Database<Overlay<'a, S>>, StorageError> {
        let parts = self.parts()?;
        let base = match &self.snapshot {
            Some(snapshot) => Base::Snapshot(snapshot),
            None => Base::Store(base),
        };
        Ok(Database {
            storage: Overlay {
                base,
                delta: DeltaRef::Read(&self.delta),
                validated: self.validated,
            },
            schema: parts.schema.clone(),
            projection: parts.projection.clone(),
            state: parts.state.clone(),
            generator: parts.generator.clone(),
            journal: None,
            validated_exclusive: false,
        })
    }

    pub(crate) fn abort(self) {}

    pub(crate) fn prepare(self) -> Result<PreparedScope, StorageError> {
        self.permit.validate()?;
        let parts = self
            .parts
            .ok_or_else(|| failure("command scope is already in use"))?;
        let changes = parts.journal;
        let version = i64::from(parts.schema.version);
        let versioned = |mut row: Row| {
            row.fields
                .insert(ROW_VERSION.into(), Value::Integer(version));
            row
        };
        let mutations = changes
            .iter()
            .map(|change| match (&change.before, &change.after) {
                (None, Some(row)) => Mutation::Insert {
                    table: change.table.clone(),
                    row: versioned(row.clone()),
                },
                (Some(_), Some(row)) => Mutation::Replace {
                    table: change.table.clone(),
                    row: versioned(row.clone()),
                },
                (_, None) => Mutation::Remove {
                    table: change.table.clone(),
                    id: change.id.clone(),
                },
            })
            .collect();
        Ok(PreparedScope {
            permit: self.permit,
            mutations,
            changes,
            state: parts.state,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };

    fn run<F: Future>(future: F) -> F::Output {
        match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("test unexpectedly suspended"),
        }
    }

    fn schema() -> SchemaDefinition {
        SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([
                (
                    "parents".into(),
                    TableDefinition {
                        columns: BTreeMap::from([(
                            "name".into(),
                            Column {
                                kind: ColumnType::String,
                                optional: false,
                            },
                        )]),
                        relations: BTreeMap::from([(
                            "children".into(),
                            RelationDefinition::HasMany {
                                table: "children".into(),
                                foreign_key: "parent_id".into(),
                            },
                        )]),
                    },
                ),
                (
                    "children".into(),
                    TableDefinition {
                        columns: BTreeMap::from([(
                            "rank".into(),
                            Column {
                                kind: ColumnType::Integer,
                                optional: false,
                            },
                        )]),
                        relations: BTreeMap::from([(
                            "parent".into(),
                            RelationDefinition::ToOne {
                                table: "parents".into(),
                                foreign_key: "parent_id".into(),
                                optional: false,
                            },
                        )]),
                    },
                ),
            ]),
        }
    }

    fn fields(name: &str) -> BTreeMap<String, Value> {
        BTreeMap::from([("name".into(), Value::String(name.into()))])
    }

    fn db() -> Database<SnapshotStorage> {
        run(Database::open(SnapshotStorage::default(), schema(), vec![])).unwrap()
    }

    fn permit(owner: &ExecutionOwner) -> ExecutionPermit {
        owner.admit(owner.request().unwrap()).unwrap().unwrap()
    }

    #[test]
    fn c02_fifo_owners_cancellation_and_idle_reservation() {
        let c = ExecutionCoordinator::isolated();
        let a = c.owner().unwrap();
        let b = c.owner().unwrap();
        let d = c.owner().unwrap();
        let ar = a.request().unwrap();
        let br = b.request().unwrap();
        let dr = d.request().unwrap();
        assert!(b.admit(br).unwrap().is_none());
        let active = a.admit(ar).unwrap().unwrap();
        assert!(d.admit(dr).unwrap().is_none());
        assert!(a.idle_permit().is_err());
        b.cancel(br).unwrap();
        assert!(b.admit(br).is_err());
        drop(active);
        let active = d.admit(dr).unwrap().unwrap();
        assert!(a.idle_permit().is_err());
        drop(active);
        let idle = a.idle_permit().unwrap();
        let br = b.request().unwrap();
        assert!(b.admit(br).unwrap().is_none());
        drop(idle);
        assert!(b.admit(br).unwrap().is_some());
    }

    #[test]
    fn c03_c04_stale_forged_owner_drop_and_global_identity() {
        let c = ExecutionCoordinator::isolated();
        let a = c.owner().unwrap();
        let b = c.owner().unwrap();
        let ar = a.request().unwrap();
        assert!(b.admit(ar).is_err());
        assert!(
            a.admit(ExecutionRequest {
                owner: ar.owner,
                id: u64::MAX
            })
            .is_err()
        );
        let active = a.admit(ar).unwrap().unwrap();
        let br = b.request().unwrap();
        drop(a);
        assert!(active.validate().is_err());
        let current = b.admit(br).unwrap().unwrap();
        drop(active);
        current.validate().unwrap();
        drop(current);
        assert!(b.admit(br).is_err());
        let g1 = ExecutionCoordinator::global();
        let g2 = ExecutionCoordinator::global();
        assert!(Arc::ptr_eq(&g1.0, &g2.0));
    }

    fn rejected(result: Result<impl Sized, StorageError>) {
        assert!(matches!(
            result,
            Err(StorageError::BackendFailure { message }) if message == "invalid execution request"
        ));
    }

    #[test]
    fn si_a04u_isolated_coordinators_reject_each_others_tickets() {
        let first = ExecutionCoordinator::isolated();
        let second = ExecutionCoordinator::isolated();
        let a = first.owner().unwrap();
        let b = second.owner().unwrap();
        let ar = a.request().unwrap();
        let br = b.request().unwrap();
        rejected(b.validate_request(ar));
        rejected(b.admit(ar));
        rejected(b.cancel(ar));
        assert_ne!(a.id, b.id);
        assert_ne!(ar.id, br.id);
        assert_eq!(second.lock().unwrap().queue, VecDeque::from([br]));
        assert_eq!(first.lock().unwrap().queue, VecDeque::from([ar]));
    }

    #[test]
    fn si_a05u_new_coordinator_never_reissues_ids() {
        let earlier = ExecutionCoordinator::isolated();
        let owner = earlier.owner().unwrap();
        let mut owners = BTreeSet::from([owner.id]);
        let mut requests = BTreeSet::from([owner.idle_permit().unwrap().request.id]);
        assert!(requests.insert(owner.request().unwrap().id));
        drop(owner);
        drop(earlier);
        let later = ExecutionCoordinator::isolated();
        let owner = later.owner().unwrap();
        assert!(owners.insert(owner.id));
        assert!(requests.insert(owner.idle_permit().unwrap().request.id));
        assert!(requests.insert(owner.request().unwrap().id));
    }

    #[test]
    fn si_a06_coordinator_selection() {
        let global = ExecutionCoordinator::global();
        let exclusive = ExecutionCoordinator::for_database(StoreOwnership::Exclusive, false);
        assert!(!Arc::ptr_eq(&exclusive.0, &global.0));
        assert!(!Arc::ptr_eq(
            &exclusive.0,
            &ExecutionCoordinator::for_database(StoreOwnership::Exclusive, false).0
        ));
        for (ownership, wasm) in [
            (StoreOwnership::Unproven, false),
            (StoreOwnership::Exclusive, true),
            (StoreOwnership::Unproven, true),
        ] {
            assert!(Arc::ptr_eq(
                &ExecutionCoordinator::for_database(ownership, wasm).0,
                &global.0
            ));
        }
    }

    /// TC-0041 SI-W01: not feature-gated, so a build without the dev feature
    /// fails to compile instead of skipping the check.
    #[test]
    fn si_w01_counters_measure_staged_scope_entries() {
        use crate::storage::counters;
        for n in [10, 1000] {
            let mut committed = db();
            for index in 0..n {
                run(committed.insert("parents", fields(&format!("p{index}")))).unwrap();
            }
            let owner = ExecutionCoordinator::isolated().owner().unwrap();
            let active = permit(&owner);
            counters::reset();
            let scope = run(StagedScope::begin(&committed, active)).unwrap();
            assert_eq!(
                counters::snapshot(),
                // Snapshot staging (no validated Exclusive owner here) scans,
                // copies and validates every row.
                counters::WorkSnapshot {
                    rows_cloned: n,
                    rows_validated: n,
                    inventory_rows_validated: n,
                    memory_rows_visited: n,
                    ..Default::default()
                }
            );
            drop(scope);
        }
    }

    #[test]
    fn c01_staged_relations_ordered_log_and_exact_counter_stay_private() {
        let mut committed = db();
        let prior = run(committed.insert("parents", fields("prior"))).unwrap();
        let initial_state = committed.state.clone();
        let c = ExecutionCoordinator::isolated();
        let owner = c.owner().unwrap();
        let mut scope = run(StagedScope::begin(&committed, permit(&owner))).unwrap();
        let mut staged = scope.enter(&committed.storage).unwrap();
        let parent = run(staged.insert("parents", fields("new"))).unwrap();
        let child = run(staged.insert(
            "children",
            BTreeMap::from([
                ("rank".into(), Value::Integer(7)),
                ("parent_id".into(), Value::Reference(parent.id.clone())),
            ]),
        ))
        .unwrap();
        run(staged.replace("parents", &parent.id, fields("edited"))).unwrap();
        assert!(run(staged.remove("parents", &prior.id)).unwrap());
        assert_eq!(
            run(staged
                .related("parents", &parent.id, "children")
                .order_by("rank", Direction::Desc)
                .limit(1)
                .fetch(&*staged))
            .unwrap(),
            vec![child.clone()]
        );
        assert_eq!(
            run(staged
                .related("children", &child.id, "parent")
                .fetch(&*staged))
            .unwrap()[0]
                .fields,
            fields("edited")
        );
        assert_eq!(
            run(committed.query("parents").fetch(&committed)).unwrap(),
            vec![prior]
        );
        assert_eq!(committed.state, initial_state);
        drop(staged);
        let prepared = scope.prepare().unwrap();
        assert_eq!(prepared.mutations.len(), 4);
        assert_eq!(prepared.changes.len(), 4);
        assert_eq!(prepared.state.next_id, initial_state.next_id + 2);
        let Mutation::Insert {
            row,
            ..
        } = &prepared.mutations[0]
        else {
            panic!("ordered insert lost")
        };
        assert_eq!(row.id, parent.id);
        assert_eq!(row.fields.get(ROW_VERSION), Some(&Value::Integer(1)));
        assert!(
            matches!(&prepared.mutations[2], Mutation::Replace { row, .. } if row.id == parent.id)
        );
        assert!(matches!(&prepared.mutations[3], Mutation::Remove { id, .. } if id == "local:1"));
        assert_eq!(committed.state, initial_state);
    }

    #[test]
    fn c03_abort_and_initialized_drop_preserve_generator_custody() {
        let mut sequence = 0;
        let mut committed = db().with_id_generator(move || {
            sequence += 1;
            format!("custom:{sequence}")
        });
        let c = ExecutionCoordinator::isolated();
        let owner = c.owner().unwrap();
        let mut scope = run(StagedScope::begin(&committed, permit(&owner))).unwrap();
        assert_eq!(
            run(scope
                .enter(&committed.storage)
                .unwrap()
                .insert("parents", fields("discard")))
            .unwrap()
            .id,
            "custom:1"
        );
        drop(scope);
        assert_eq!(
            run(committed.insert("parents", fields("keep"))).unwrap().id,
            "custom:2"
        );
        let initial = committed.state.clone();
        let mut scope = run(StagedScope::begin(&committed, permit(&owner))).unwrap();
        run(scope
            .enter(&committed.storage)
            .unwrap()
            .insert("parents", fields("abort")))
        .unwrap();
        scope.abort();
        assert_eq!(
            run(committed.query("parents").fetch(&committed))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(committed.state, initial);
        assert_eq!(
            run(committed.insert("parents", fields("next"))).unwrap().id,
            "custom:4"
        );
    }

    #[test]
    fn c03_preparation_retains_permit_through_commit_boundary() {
        let committed = db();
        let c = ExecutionCoordinator::isolated();
        let a = c.owner().unwrap();
        let b = c.owner().unwrap();
        let scope = run(StagedScope::begin(&committed, permit(&a))).unwrap();
        let br = b.request().unwrap();
        let prepared = scope.prepare().unwrap();
        prepared.permit.validate().unwrap();
        assert!(b.admit(br).unwrap().is_none());
        assert!(prepared.mutations.is_empty());
        drop(prepared);
        assert!(b.admit(br).unwrap().is_some());
    }

    #[test]
    fn c03_dropped_initialization_future_releases_admission() {
        let committed = db();
        let c = ExecutionCoordinator::isolated();
        let a = c.owner().unwrap();
        let b = c.owner().unwrap();
        let begin = StagedScope::begin(&committed, permit(&a));
        let br = b.request().unwrap();
        assert!(b.admit(br).unwrap().is_none());
        drop(begin);
        assert!(b.admit(br).unwrap().is_some());
    }

    #[test]
    fn c04_revoked_scope_rejects_reads_writes_and_preparation() {
        let committed = db();
        let c = ExecutionCoordinator::isolated();
        let owner = c.owner().unwrap();
        let mut scope = run(StagedScope::begin(&committed, permit(&owner))).unwrap();
        drop(owner);
        assert!(scope.reader(&committed.storage).is_err());
        assert!(scope.enter(&committed.storage).is_err());
        assert!(scope.prepare().is_err());
        assert!(
            run(committed.query("parents").fetch(&committed))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn c03_failed_snapshot_releases_admission_without_changing_source() {
        let mut committed = db();
        let malformed = Row {
            id: "broken".into(),
            fields: fields("missing managed version"),
        };
        run(committed.storage.insert("parents", malformed.clone())).unwrap();
        let c = ExecutionCoordinator::isolated();
        let a = c.owner().unwrap();
        let b = c.owner().unwrap();
        let active = permit(&a);
        let br = b.request().unwrap();
        assert!(run(StagedScope::begin(&committed, active)).is_err());
        assert!(b.admit(br).unwrap().is_some());
        assert_eq!(
            run(committed.storage.get("parents", "broken")).unwrap(),
            Some(malformed)
        );
        assert_eq!(committed.state.next_id, 1);
    }
}
