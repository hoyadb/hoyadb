//! API-0006 section 4: checked local command owner; no unqueued commit escape.
use super::{Json, JsonError, execution::*, ledger_txn::LedgerTxn, records::*};
use crate::{model::*, storage::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Deref,
};

#[derive(Clone, PartialEq, Eq)]
pub enum CommandError {
    Storage(StorageError),
    InvalidCommand,
    InvalidTicket,
    Busy,
    Capacity,
    InvalidPayload(JsonError),
    Protocol,
    UnknownLocalReference,
    RollbackConflict,
    Closed,
    LimitExceeded,
    AuthorityRequired,
    StalePull,
    PullConflict,
}

impl From<StorageError> for CommandError {
    fn from(value: StorageError) -> Self {
        Self::Storage(value)
    }
}

impl From<JsonError> for CommandError {
    fn from(value: JsonError) -> Self {
        Self::InvalidPayload(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgumentType {
    Any,
    String,
    Integer,
    Float,
    Boolean,
    Array,
    Object,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Argument {
    pub kind: ArgumentType,
    pub optional: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandDefinition {
    pub name: String,
    pub arguments: BTreeMap<String, Argument>,
    pub reference_paths: Vec<Vec<PathPart>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandRequest(ExecutionRequest);

#[derive(Clone, PartialEq, Eq)]
pub struct CapturedContext {
    identity: Json,
    now_ms: i64,
}

impl CapturedContext {
    pub fn identity(&self) -> &Json {
        &self.identity
    }

    pub fn now(&self) -> i64 {
        self.now_ms
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub key: String,
    pub result: Json,
}

pub(super) struct ActiveCommand {
    request: CommandRequest,
    pub(super) scope: StagedScope,
    definition: CommandDefinition,
    args: Json,
    context: CapturedContext,
    dependencies: BTreeSet<String>,
}

pub struct CommandDatabase<S> {
    pub(super) db: ReactiveDatabase<S>,
    pub(super) execution_owner: ExecutionOwner,
    pub(super) ledger: SyncLedger,
    pub(super) aliases: super::durable::AliasState,
    pub(super) persisted_rows: Option<BTreeMap<String, Row>>,
    pub(super) ledger_epoch: u64,
    pub(super) tombstones: usize,
    pub(super) metadata_ordinals: BTreeMap<(String, String), u64>,
    #[cfg(feature = "work-counters")]
    pub(super) ledger_work_keys: counters::LedgerWorkState,
    pub(super) active: Option<ActiveCommand>,
    pub(super) delivery: super::sync::Delivery,
    pub(super) entity_provider:
        Option<Box<dyn FnMut() -> Result<super::entity::EntityContext, CommandError> + Send>>,
    pub(super) merge_hooks: BTreeMap<String, super::entity::MergeHook>,
    pub(super) pull: Option<super::pull::PullTicketState>,
    pub(super) merge: Option<super::merge::MergeState>,
    pub(super) conflict_policy: super::ConflictPolicy,
}

impl<S> Deref for CommandDatabase<S> {
    type Target = Database<S>;

    fn deref(&self) -> &Self::Target {
        &self.db
    }
}

impl<S: SyncStorage> CommandDatabase<S> {
    /// TC-0041 SI-Y01/SI-Y09 inspection only; absent from production builds.
    #[cfg(feature = "work-counters")]
    #[doc(hidden)]
    pub fn ledger_work(&self) -> counters::LedgerWorkState {
        counters::LedgerWorkState {
            epoch: self.ledger_epoch,
            cached_tombstones: i64::try_from(self.tombstones).ok(),
            ..self.ledger_work_keys.clone()
        }
    }

    /// Independently recompute the cache invariant after taking the measured
    /// snapshot. This deliberately visits every entry and is test-only work.
    #[cfg(feature = "work-counters")]
    #[doc(hidden)]
    pub fn ledger_tombstone_count(&self) -> i64 {
        self.ledger.entity.as_ref().map_or(0, |entity| {
            count_work!(metadata_entries_visited, entity.metadata.len() as u64);
            entity
                .metadata
                .values()
                .filter(|value| value.tombstone.is_some())
                .count() as i64
        })
    }

    pub async fn open(
        storage: S,
        schema: SchemaDefinition,
        migrations: Vec<Migration>,
        new_namespace: [u8; 16],
        options: SyncOptions,
    ) -> Result<Self, CommandError> {
        let execution_owner =
            ExecutionCoordinator::for_database(storage.ownership(), cfg!(target_arch = "wasm32"))
                .owner()?;
        let _permit = execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        let persisted_rows = storage.load_sync()?;
        let (ledger, aliases) = if let Some(rows) = &persisted_rows {
            let stored = super::durable::StoredLedger::from_rows(rows)?;
            let ledger = stored.ledger;
            if ledger.header.options != options {
                return Err(CommandError::InvalidCommand);
            }
            (ledger, stored.aliases)
        } else {
            (
                SyncLedger {
                    entity: None,
                    header: SyncHeader {
                        namespace: new_namespace
                            .iter()
                            .map(|byte| format!("{byte:02x}"))
                            .collect(),
                        sequence: 1,
                        last_ms: 0,
                        options,
                    },
                    records: BTreeMap::new(),
                },
                super::durable::AliasState::default(),
            )
        };
        ledger.validate()?;
        let exclusive = storage.ownership() == StoreOwnership::Exclusive;
        let mut db = Database::open(storage, schema, migrations).await?;
        if aliases.enabled
            && let Some(rows) = &persisted_rows
        {
            validate_sync_inventory(rows, &db.state)?;
        }
        // SPEC-0037 R5.2: command-capable open validates every stored row once,
        // for every adapter, so untouched corruption fails here, not per command.
        let mut inventory = BTreeMap::new();
        for table in db.schema.tables.keys() {
            inventory.insert(table.clone(), db.storage.scan(table).await?);
        }
        validate_model_inventory(&db.projection, db.state.version, &inventory)?;
        db.validated_exclusive = exclusive;
        let db = db.into_reactive();
        let (metadata_ordinals, tombstones) = metadata_inventory(&ledger);
        let persisted_rows =
            persisted_rows.map(|rows| rows.into_iter().map(|row| (row.id.clone(), row)).collect());
        Ok(Self {
            db,
            execution_owner,
            ledger,
            aliases,
            persisted_rows,
            ledger_epoch: 0,
            tombstones,
            metadata_ordinals,
            #[cfg(feature = "work-counters")]
            ledger_work_keys: counters::LedgerWorkState::default(),
            active: None,
            delivery: super::sync::Delivery::default(),
            entity_provider: None,
            merge_hooks: BTreeMap::new(),
            conflict_policy: super::ConflictPolicy::default(),
            pull: None,
            merge: None,
        })
    }

    pub fn request(&self) -> Result<CommandRequest, CommandError> {
        Ok(CommandRequest(self.execution_owner.request()?))
    }

    pub fn cancel_request(&self, request: CommandRequest) -> Result<(), CommandError> {
        self.execution_owner
            .cancel(request.0)
            .map_err(|_| CommandError::InvalidTicket)
    }

    pub fn set_id_generator(
        &mut self,
        generator: impl FnMut() -> String + Send + 'static,
    ) -> Result<(), CommandError> {
        let _permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        self.db.db.generator = Some(std::sync::Arc::new(std::sync::Mutex::new(Box::new(
            generator,
        ))));
        Ok(())
    }

    pub async fn begin(
        &mut self,
        request: CommandRequest,
        definition: CommandDefinition,
        args: Json,
        identity: Option<Json>,
        now_ms: i64,
    ) -> Result<bool, CommandError> {
        self.execution_owner
            .validate_request(request.0)
            .map_err(|_| CommandError::InvalidTicket)?;
        let validated = (|| {
            validate_invocation(&definition, &args, identity.as_ref(), now_ms)?;
            if self.ledger.live_keys().len() >= self.ledger.header.options.capacity {
                return Err(CommandError::Capacity);
            }
            let mut args = args;
            let dependencies = normalize_references_with_aliases(
                &mut args,
                &definition.reference_paths,
                &self.ledger,
                Some(&self.aliases),
            )?;
            // The stored record and envelope nest these values deeper than their
            // standalone encodings. Apply finish's own bound to the record this
            // invocation enqueues with no staged change, so a value that can never
            // be stored fails here, before its handler runs. Staged images only add.
            let probe = queued_command(
                &self.ledger.header,
                self.db.db.state.version,
                &definition,
                args,
                identity.clone().ok_or(CommandError::InvalidCommand)?,
                now_ms,
                dependencies,
            );
            probe
                .check_send_size()
                .map_err(|_| CommandError::LimitExceeded)?;
            Ok((probe.args, probe.dependencies))
        })();
        let (args, dependencies) = match validated {
            Ok(value) => value,
            Err(error) => {
                self.execution_owner
                    .cancel(request.0)
                    .map_err(|_| CommandError::InvalidTicket)?;
                return Err(error);
            }
        };
        let Some(permit) = self
            .execution_owner
            .admit(request.0)
            .map_err(|_| CommandError::InvalidTicket)?
        else {
            return Ok(false);
        };
        let scope = StagedScope::begin(&self.db.db, permit).await?;
        self.active = Some(ActiveCommand {
            request,
            scope,
            definition,
            args,
            context: CapturedContext {
                identity: identity.ok_or(CommandError::InvalidCommand)?,
                now_ms,
            },
            dependencies,
        });
        Ok(true)
    }

    fn active(&self, request: CommandRequest) -> Result<&ActiveCommand, CommandError> {
        let active = self
            .active
            .as_ref()
            .filter(|active| active.request == request)
            .ok_or(CommandError::InvalidTicket)?;
        active
            .scope
            .validate()
            .map_err(|_| CommandError::InvalidTicket)?;
        Ok(active)
    }

    /// SPEC-0037 R6: the active scope's writable staged database over this
    /// owner's store, for one call.
    pub(super) fn staged(
        &mut self,
        request: CommandRequest,
    ) -> Result<super::execution::Lease<'_, S>, CommandError> {
        let base = &self.db.db.storage;
        let active = self
            .active
            .as_mut()
            .filter(|active| active.request == request)
            .ok_or(CommandError::InvalidTicket)?;
        active
            .scope
            .enter(base)
            .map_err(|_| CommandError::InvalidTicket)
    }

    fn staged_view(
        &self,
        request: CommandRequest,
    ) -> Result<Database<super::overlay::Overlay<'_, S>>, CommandError> {
        self.active(request)?
            .scope
            .reader(&self.db.db.storage)
            .map_err(|_| CommandError::InvalidTicket)
    }

    pub fn scope_args(&self, request: CommandRequest) -> Result<&Json, CommandError> {
        Ok(&self.active(request)?.args)
    }

    pub fn scope_context(&self, request: CommandRequest) -> Result<&CapturedContext, CommandError> {
        Ok(&self.active(request)?.context)
    }

    pub async fn scope_fetch(
        &self,
        request: CommandRequest,
        query: &QueryBuilder,
    ) -> Result<Vec<Row>, CommandError> {
        Ok(query.fetch(&self.staged_view(request)?).await?)
    }

    pub async fn scope_apply_batch(
        &mut self,
        request: CommandRequest,
        writes: Vec<Write>,
    ) -> Result<Vec<CommittedChange>, CommandError> {
        Ok(self.staged(request)?.apply_batch(writes).await?)
    }

    pub async fn scope_insert(
        &mut self,
        request: CommandRequest,
        table: &str,
        fields: BTreeMap<String, Value>,
    ) -> Result<Row, CommandError> {
        Ok(self.staged(request)?.insert(table, fields).await?)
    }

    pub async fn scope_replace(
        &mut self,
        request: CommandRequest,
        table: &str,
        id: &str,
        fields: BTreeMap<String, Value>,
    ) -> Result<Row, CommandError> {
        Ok(self.staged(request)?.replace(table, id, fields).await?)
    }

    pub async fn scope_remove(
        &mut self,
        request: CommandRequest,
        table: &str,
        id: &str,
    ) -> Result<bool, CommandError> {
        Ok(self.staged(request)?.remove(table, id).await?)
    }

    pub async fn scope_link(
        &mut self,
        request: CommandRequest,
        source: &str,
        id: &str,
        relation: &str,
        target: &str,
        fields: BTreeMap<String, Value>,
    ) -> Result<Row, CommandError> {
        Ok(self
            .staged(request)?
            .link(source, id, relation, target, fields)
            .await?)
    }

    pub async fn scope_unlink(
        &mut self,
        request: CommandRequest,
        source: &str,
        id: &str,
        relation: &str,
        target: &str,
    ) -> Result<usize, CommandError> {
        Ok(self
            .staged(request)?
            .unlink(source, id, relation, target)
            .await?)
    }

    pub async fn finish(
        &mut self,
        request: CommandRequest,
        result: Json,
    ) -> Result<CommandResult, CommandError> {
        self.active(request)?;
        let active = self.active.take().ok_or(CommandError::InvalidTicket)?;
        result.canonical()?;
        if self.ledger.live_keys().len() >= self.ledger.header.options.capacity {
            return Err(CommandError::Capacity);
        }
        let prepared = active.scope.prepare()?;
        let version = self.db.db.state.version;
        let (key, changes) = self.transact(prepared.state.clone(), &prepared.permit, |next| {
            // The same constructor begin preflighted, now from the committing header.
            let mut record = queued_command(
                &next.header,
                prepared.state.version,
                &active.definition,
                active.args,
                active.context.identity,
                active.context.now_ms,
                active.dependencies,
            );
            let order = record.order;
            next.header_mut().sequence = order.checked_add(1).ok_or(CommandError::LimitExceeded)?;
            next.header_mut().last_ms = next.header.last_ms.max(active.context.now_ms);
            let key = record.key.clone();
            if next.records.contains_key(&key) {
                return Err(CommandError::InvalidCommand);
            }
            let mut bounds = BTreeMap::new();
            for change in &prepared.changes {
                let entry = bounds
                    .entry((change.table.clone(), change.id.clone()))
                    .or_insert((change.before.is_none(), change.after.as_ref()));
                entry.1 = change.after.as_ref();
            }
            let mut created = BTreeMap::new();
            for ((table, _), (was_new, after)) in bounds {
                if was_new && let Some(row) = after {
                    if !row.id.starts_with("local:")
                        || row.id.len() == 6
                        || next
                            .records
                            .values()
                            .any(|record| record.created.contains_key(&row.id))
                    {
                        return Err(CommandError::InvalidCommand);
                    }
                    if let Some(previous_table) = created.insert(row.id.clone(), table.clone())
                        && previous_table != table
                    {
                        return Err(CommandError::InvalidCommand);
                    }
                }
            }
            record.created = created;
            record.undo = prepared.changes;
            record
                .check_send_size()
                .map_err(|_| CommandError::LimitExceeded)?;
            let guards = initial_guards(&record.undo, version);
            next.insert_record(key.clone(), record);
            Ok((key, guards, prepared.mutations))
        })?;
        self.db.publish_committed(changes).await;
        Ok(CommandResult {
            key,
            result,
        })
    }

    pub fn abort(&mut self, request: CommandRequest) -> Result<(), CommandError> {
        self.active(request)?;
        self.active
            .take()
            .ok_or(CommandError::InvalidTicket)?
            .scope
            .abort();
        Ok(())
    }

    pub fn close(self) {}

    /// Close this owner and recover only its committed storage.
    pub fn into_storage(self) -> S {
        let Self {
            db,
            active,
            execution_owner,
            ..
        } = self;
        drop(active);
        drop(execution_owner);
        db.into_storage()
    }

    pub(super) fn transact<T>(
        &mut self,
        next_model: ModelState,
        permit: &ExecutionPermit,
        plan: impl FnOnce(&mut LedgerTxn<'_>) -> Result<(T, Vec<RowGuard>, Vec<Mutation>), CommandError>,
    ) -> Result<(T, Vec<CommittedChange>), CommandError> {
        #[cfg(feature = "work-counters")]
        {
            self.ledger_work_keys = counters::LedgerWorkState::default();
        }
        let mut txn = LedgerTxn::with_aliases(
            &mut self.ledger,
            &mut self.aliases,
            self.db.db.validated_exclusive,
        );
        let planned = plan(&mut txn);
        #[cfg(feature = "work-counters")]
        {
            self.ledger_work_keys.record_keys = txn.record_keys();
            self.ledger_work_keys.metadata_keys = txn.metadata_keys();
        }
        let (value, guards, mutations) = planned?;
        if !txn.changed() && mutations.is_empty() && next_model == self.db.db.state {
            return Ok((value, vec![]));
        }
        let mut commit = LedgerCommit {
            db: &mut self.db,
            persisted_rows: &mut self.persisted_rows,
            ledger_epoch: &mut self.ledger_epoch,
            tombstones: &mut self.tombstones,
            metadata_ordinals: &mut self.metadata_ordinals,
            delivery: &mut self.delivery,
        };
        let changes = commit.persist(txn, next_model, guards, mutations, permit)?;
        Ok((value, changes))
    }

    pub async fn insert(
        &mut self,
        table: &str,
        fields: BTreeMap<String, Value>,
    ) -> Result<Row, CommandError> {
        if self.ledger.entity.is_some() {
            return self
                .entity_apply_batch(vec![Write::Insert {
                    table: table.into(),
                    fields,
                }])
                .await?
                .into_iter()
                .next()
                .and_then(|c| c.after)
                .ok_or(CommandError::Protocol);
        }
        let _permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        Ok(self.db.insert(table, fields).await?)
    }

    pub async fn apply_batch(
        &mut self,
        writes: Vec<Write>,
    ) -> Result<Vec<CommittedChange>, CommandError> {
        if self.ledger.entity.is_some() {
            return self.entity_apply_batch(writes).await;
        }
        let _permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        Ok(self.db.apply_batch(writes).await?)
    }

    pub async fn replace(
        &mut self,
        table: &str,
        id: &str,
        fields: BTreeMap<String, Value>,
    ) -> Result<Row, CommandError> {
        if self.ledger.entity.is_some() {
            return self.entity_replace(table, id, fields).await;
        }
        let _permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        Ok(self.db.replace(table, id, fields).await?)
    }

    pub async fn remove(&mut self, table: &str, id: &str) -> Result<bool, CommandError> {
        if self.ledger.entity.is_some() {
            return Ok(!self
                .entity_apply_batch(vec![Write::Remove {
                    table: table.into(),
                    id: id.into(),
                }])
                .await?
                .is_empty());
        }
        let _permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        Ok(self.db.remove(table, id).await?)
    }

    pub async fn link(
        &mut self,
        source: &str,
        id: &str,
        relation: &str,
        target: &str,
        fields: BTreeMap<String, Value>,
    ) -> Result<Row, CommandError> {
        if self.ledger.entity.is_some() {
            return self.entity_link(source, id, relation, target, fields).await;
        }
        let _permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        Ok(self.db.link(source, id, relation, target, fields).await?)
    }

    pub async fn unlink(
        &mut self,
        source: &str,
        id: &str,
        relation: &str,
        target: &str,
    ) -> Result<usize, CommandError> {
        if self.ledger.entity.is_some() {
            return self.entity_unlink(source, id, relation, target).await;
        }
        let _permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        Ok(self.db.unlink(source, id, relation, target).await?)
    }

    pub async fn observe(&mut self, query: &QueryBuilder) -> Result<Subscription, StorageError> {
        query.observe(&mut self.db).await
    }

    pub async fn observe_count(
        &mut self,
        query: &QueryBuilder,
    ) -> Result<Subscription, StorageError> {
        query.observe_count(&mut self.db).await
    }

    pub async fn observe_with_columns(
        &mut self,
        query: &QueryBuilder,
        columns: Vec<String>,
    ) -> Result<Subscription, StorageError> {
        query.observe_with_columns(&mut self.db, columns).await
    }

    pub fn unsubscribe(&mut self, subscription: Subscription) -> bool {
        self.db.unsubscribe(subscription)
    }

    pub fn drain_notifications(&mut self, now_ms: u64) -> Vec<Notification> {
        self.db.drain_notifications(now_ms)
    }

    pub fn observation_stats(&self) -> ObservationStats {
        self.db.observation_stats()
    }

    pub async fn refresh_notifications(&mut self) {
        self.db.refresh_notifications().await
    }
}

/// The ledger borrow is separate from the synchronous storage commit fields.
struct LedgerCommit<'a, S> {
    db: &'a mut ReactiveDatabase<S>,
    persisted_rows: &'a mut Option<BTreeMap<String, Row>>,
    ledger_epoch: &'a mut u64,
    tombstones: &'a mut usize,
    metadata_ordinals: &'a mut BTreeMap<(String, String), u64>,
    delivery: &'a mut super::sync::Delivery,
}

impl<S: SyncStorage> LedgerCommit<'_, S> {
    fn persist(
        &mut self,
        mut txn: LedgerTxn<'_>,
        next_model: ModelState,
        guards: Vec<RowGuard>,
        mutations: Vec<Mutation>,
        permit: &ExecutionPermit,
    ) -> Result<Vec<CommittedChange>, CommandError> {
        let changed_metadata = txn.changed_metadata();
        if let Some(previous) = txn.prior_entity_header()
            && (!mutations.is_empty()
                || !changed_metadata.is_empty()
                || !txn.changed_aliases().is_empty())
        {
            let revision = previous
                .revision
                .checked_add(1)
                .ok_or(CommandError::LimitExceeded)?;
            txn.edit_entity_header(|header| {
                header.revision = revision;
                Ok(())
            })?;
        }
        let mut next_tombstones = *self.tombstones;
        for identity in &changed_metadata {
            let before = txn
                .prior_metadata(identity)
                .is_some_and(|value| value.tombstone.is_some());
            let after = txn
                .entity
                .as_ref()
                .and_then(|entity| entity.metadata.get(identity))
                .is_some_and(|value| value.tombstone.is_some());
            next_tombstones = next_tombstones
                .checked_sub(usize::from(before))
                .and_then(|value| value.checked_add(usize::from(after)))
                .ok_or(CommandError::LimitExceeded)?;
        }
        let next_epoch = self
            .ledger_epoch
            .checked_add(1)
            .ok_or(CommandError::LimitExceeded)?;
        // Reconfiguration additionally proves application-wide emptiness, which
        // remains the full-plan adapter's responsibility.
        let reconfigure = txn
            .prior_entity_header()
            .zip(txn.entity_header())
            .is_some_and(|(old, new)| old.config != new.config);
        let mut changes = if self.db.db.validated_exclusive {
            if reconfigure {
                self.full_plan(&txn, &next_model, guards, mutations, permit)?
            } else {
                txn.validate()?;
                validate_sync_model_bounded(&txn, &next_model)?;
                validate_ledger_transition(
                    &txn,
                    &self.db.db.state,
                    &next_model,
                    &guards,
                    &mutations,
                    &changed_metadata,
                )?;
                let (packet_changes, rank_changes) = packet_changes(
                    &txn,
                    self.persisted_rows.as_ref(),
                    self.metadata_ordinals,
                    &changed_metadata,
                )?;
                let expected_ids = packet_changes
                    .keys()
                    .cloned()
                    .chain(["state".into(), "entity:state".into()])
                    .collect::<BTreeSet<_>>();
                let expected_packets = expected_ids
                    .into_iter()
                    .map(|id| {
                        let before = self.persisted_rows.as_ref().and_then(|rows| rows.get(&id));
                        count_work!(rows_cloned, u64::from(before.is_some()));
                        (id, before.cloned())
                    })
                    .collect();
                let mut upserts = Vec::new();
                let mut deletes = Vec::new();
                for (id, row) in &packet_changes {
                    if let Some(row) = row {
                        count_work!(rows_cloned, 1);
                        upserts.push(row.clone());
                    } else {
                        deletes.push(id.clone());
                    }
                }
                permit.validate().map_err(|_| CommandError::InvalidTicket)?;
                let delta = SyncDelta::validated_exclusive(SyncDeltaParts {
                    expected_model: self.db.db.state.clone(),
                    next_model: next_model.clone(),
                    expected_packets,
                    upserts,
                    deletes,
                    guards,
                    mutations,
                })?;
                let changes = self.db.db.storage.apply_sync_delta(delta)?;
                let packets = self.persisted_rows.get_or_insert_with(BTreeMap::new);
                for (id, row) in packet_changes {
                    if let Some(row) = row {
                        packets.insert(id, row);
                    } else {
                        packets.remove(&id);
                    }
                }
                for (identity, rank) in rank_changes {
                    if let Some(rank) = rank {
                        self.metadata_ordinals.insert(identity, rank);
                    } else {
                        self.metadata_ordinals.remove(&identity);
                    }
                }
                changes
            }
        } else {
            self.full_plan(&txn, &next_model, guards, mutations, permit)?
        };
        self.db.db.state = next_model;
        *self.ledger_epoch = next_epoch;
        *self.tombstones = next_tombstones;
        let ledger = txn.commit();
        self.delivery.changed(ledger, next_tombstones);
        for change in &mut changes {
            for row in change.before.iter_mut().chain(change.after.iter_mut()) {
                row.fields.remove(ROW_VERSION);
            }
        }
        Ok(changes)
    }
    fn full_plan(
        &mut self,
        txn: &LedgerTxn<'_>,
        next_model: &ModelState,
        guards: Vec<RowGuard>,
        mutations: Vec<Mutation>,
        permit: &ExecutionPermit,
    ) -> Result<Vec<CommittedChange>, CommandError> {
        let rows = txn.stored_rows()?;
        permit.validate().map_err(|_| CommandError::InvalidTicket)?;
        let changes = self.db.db.storage.apply_sync_plan(SyncPlan {
            transition: SyncTransition::Normal,
            expected_model: self.db.db.state.clone(),
            next_model: next_model.clone(),
            expected_sync: self.persisted_rows.as_ref().map(clone_packet_inventory),
            next_sync: clone_sync_rows(&rows),
            guards,
            mutations,
        })?;
        *self.persisted_rows = Some(rows.into_iter().map(|row| (row.id.clone(), row)).collect());
        *self.metadata_ordinals = metadata_inventory(txn).0;
        Ok(changes)
    }
}

type RankChanges = Vec<((String, String), Option<u64>)>;
type PacketChanges = BTreeMap<String, Option<Row>>;

fn packet_changes(
    txn: &LedgerTxn<'_>,
    persisted: Option<&BTreeMap<String, Row>>,
    ranks: &BTreeMap<(String, String), u64>,
    changed_metadata: &BTreeSet<(String, String)>,
) -> Result<(PacketChanges, RankChanges), CommandError> {
    let mut packets = BTreeMap::from([("state".into(), Some(sync_header_row(&txn.header)?))]);
    packets.insert(
        "entity:state".into(),
        txn.entity
            .as_ref()
            .map(|entity| {
                super::durable::stored_entity_header(
                    entity,
                    txn.aliases().is_some_and(|aliases| aliases.enabled),
                )
            })
            .transpose()?,
    );
    if let Some(aliases) = txn.aliases() {
        for (table, local) in txn.changed_aliases() {
            let id = super::durable::alias_packet_id(&table, &local)?;
            let packet = aliases
                .mappings
                .get(&(table.clone(), local.clone()))
                .map(|target| super::durable::alias_packet(&table, &local, target))
                .transpose()?;
            packets.insert(id, packet);
        }
    }
    for key in txn.changed_records() {
        packets.insert(format!("command:{key}"), None);
        packets.insert(format!("entity:record:{key}"), None);
        if let Some(record) = txn.records.get(&key) {
            for row in record_rows(record)? {
                packets.insert(row.id.clone(), Some(row));
            }
        }
    }
    let mut rank_changes = Vec::new();
    if let Some(entity) = &txn.entity {
        if entity.ledger_format == 3 {
            for identity in changed_metadata {
                count_work!(metadata_entries_visited, 1);
                let id = identity_metadata_id(&identity.0, &identity.1)?;
                let row = entity
                    .metadata
                    .get(identity)
                    .map(|metadata| identity_metadata_row(&identity.0, &identity.1, metadata))
                    .transpose()?;
                packets.insert(id, row);
            }
            packets.retain(|id, row| persisted.and_then(|prior| prior.get(id)) != row.as_ref());
            return Ok((packets, rank_changes));
        }
        let membership = changed_metadata
            .iter()
            .filter(|identity| {
                txn.prior_metadata(identity).is_some() != entity.metadata.contains_key(*identity)
            })
            .cloned()
            .collect::<BTreeSet<_>>();
        let first_shift = membership.first();
        for identity in changed_metadata {
            if first_shift.is_some_and(|first| identity >= first) {
                continue;
            }
            if let Some(metadata) = entity.metadata.get(identity) {
                let rank = ranks.get(identity).ok_or(CommandError::Protocol)?;
                count_work!(metadata_entries_visited, 1);
                let row = metadata_row(*rank, &identity.0, &identity.1, metadata)?;
                packets.insert(row.id.clone(), Some(row));
            }
        }
        if let Some(first) = first_shift {
            let start = ranks
                .range(first.clone()..)
                .next()
                .map_or(ranks.len() as u64, |(_, rank)| *rank);
            for (offset, (identity, metadata)) in entity.metadata.range(first.clone()..).enumerate()
            {
                count_work!(metadata_entries_visited, 1);
                let ordinal = start
                    .checked_add(offset as u64)
                    .ok_or(CommandError::LimitExceeded)?;
                let row = metadata_row(ordinal, &identity.0, &identity.1, metadata)?;
                packets.insert(row.id.clone(), Some(row));
                rank_changes.push((identity.clone(), Some(ordinal)));
            }
            for identity in membership {
                if !entity.metadata.contains_key(&identity) {
                    rank_changes.push((identity, None));
                }
            }
            for ordinal in entity.metadata.len() as u64..ranks.len() as u64 {
                packets.insert(format!("entity:metadata:{ordinal:016x}"), None);
            }
        }
    }
    packets.retain(|id, row| persisted.and_then(|prior| prior.get(id)) != row.as_ref());
    Ok((packets, rank_changes))
}

fn validate_ledger_transition(
    txn: &LedgerTxn<'_>,
    expected_model: &ModelState,
    next_model: &ModelState,
    guards: &[RowGuard],
    mutations: &[Mutation],
    changed_metadata: &BTreeSet<(String, String)>,
) -> Result<(), CommandError> {
    let old = txn.prior_header();
    if old.namespace != txn.header.namespace
        || txn.header.sequence < old.sequence
        || txn.header.last_ms < old.last_ms
    {
        return Err(schema_conflict("command identity counters cannot regress").into());
    }
    for key in txn.changed_records() {
        if let (Some(old), Some(new)) = (txn.prior_record(&key), txn.records.get(&key))
            && old.frozen.is_some()
            && (old.frozen != new.frozen || old.envelope() != new.envelope())
        {
            return Err(schema_conflict("frozen command envelope cannot change").into());
        }
    }
    let definition = model_definition(next_model)?.storage_schema()?;
    if let Some(entity) = &txn.entity {
        for identity in changed_metadata {
            if let Some(metadata) = entity.metadata.get(identity) {
                count_work!(metadata_entries_visited, 1);
                let invalid_metadata = || StorageError::BackendFailure {
                    message: "invalid entity comparison bookkeeping".into(),
                };
                let table = definition
                    .tables
                    .get(&identity.0)
                    .ok_or_else(invalid_metadata)?;
                if metadata
                    .fields
                    .keys()
                    .any(|field| field == ROW_VERSION || !table.columns.contains_key(field))
                {
                    return Err(invalid_metadata().into());
                }
            }
        }
    }
    let pristine = |header: &super::ledger_txn::EntityHeader, empty: bool| {
        header.checkpoint.is_none()
            && !header.ready
            && header.pages == 0
            && header.changes == 0
            && empty
    };
    match (txn.prior_entity_header(), txn.entity_header()) {
        (None, None) => {}
        (None, Some(new)) => {
            if txn.prior_records().iter().any(|record| record.is_live())
                || !pristine(
                    &new,
                    txn.entity
                        .as_ref()
                        .is_none_or(|entity| entity.metadata.is_empty()),
                )
                || new.generation != 1
                || new.revision != 0
            {
                return Err(schema_conflict("invalid entity adoption").into());
            }
        }
        (Some(_), None) => {
            return Err(schema_conflict("entity comparison state cannot disappear").into());
        }
        (Some(old), Some(new)) => {
            if old.ledger_format != new.ledger_format
                || txn.prior_alias_enabled() != txn.aliases().is_some_and(|state| state.enabled)
            {
                return Err(
                    schema_conflict("entity ledger format cannot change implicitly").into(),
                );
            }
            let config_changed = old.config != new.config;
            if config_changed
                && (old.generation <= 1
                    || !txn.prior_records().is_empty()
                    || !txn.records.is_empty()
                    || expected_model != next_model
                    || old_header_changed(txn)
                    || !guards.is_empty()
                    || !mutations.is_empty())
            {
                return Err(schema_conflict(
                    "entity reconfiguration requires an unchanged empty post-reset shell",
                )
                .into());
            }
            if new.pages < old.pages
                || new.changes < old.changes
                || (old.ready && !new.ready)
                || (old.checkpoint.is_some() && new.checkpoint.is_none())
            {
                return Err(schema_conflict(
                    "bootstrap progress requires explicit reset to regress",
                )
                .into());
            }
            let changed = !mutations.is_empty()
                || !changed_metadata.is_empty()
                || !txn.changed_aliases().is_empty();
            let revision = old
                .revision
                .checked_add(i64::from(changed))
                .ok_or_else(|| schema_conflict("entity revision exhausted"))?;
            if old.generation != new.generation
                || new.revision != revision
                || (config_changed
                    && !(pristine(old, txn.prior_metadata_len() == 0)
                        && pristine(
                            &new,
                            txn.entity
                                .as_ref()
                                .is_none_or(|entity| entity.metadata.is_empty()),
                        )))
            {
                return Err(schema_conflict(
                    "invalid entity generation/configuration/revision transition",
                )
                .into());
            }
        }
    }
    Ok(())
}
fn old_header_changed(txn: &LedgerTxn<'_>) -> bool {
    txn.prior_header() != &txn.header
}

pub(super) fn clone_packet_inventory(rows: &BTreeMap<String, Row>) -> Vec<Row> {
    count_work!(rows_cloned, rows.len() as u64);
    rows.values().cloned().collect()
}

pub(super) fn metadata_inventory(ledger: &SyncLedger) -> (BTreeMap<(String, String), u64>, usize) {
    let mut ranks = BTreeMap::new();
    let mut tombstones = 0;
    if let Some(entity) = &ledger.entity {
        for (ordinal, (identity, metadata)) in entity.metadata.iter().enumerate() {
            count_work!(metadata_entries_visited, 1);
            if entity.ledger_format == 2 {
                ranks.insert(identity.clone(), ordinal as u64);
            }
            tombstones += usize::from(metadata.tombstone.is_some());
        }
    }
    (ranks, tombstones)
}

pub(super) fn clone_sync_rows(rows: &[Row]) -> Vec<Row> {
    count_work!(rows_cloned, rows.len() as u64);
    rows.to_vec()
}

pub(super) fn validate_command_name(name: &str) -> Result<(), CommandError> {
    let Some((prefix, version)) = name.rsplit_once("-v") else {
        return Err(CommandError::InvalidCommand);
    };
    if prefix.is_empty()
        || name.len() > 128
        || version.is_empty()
        || !version.bytes().all(|byte| byte.is_ascii_digit())
        || version.bytes().all(|byte| byte == b'0')
    {
        return Err(CommandError::InvalidCommand);
    }
    Ok(())
}

fn validate_invocation(
    definition: &CommandDefinition,
    args: &Json,
    identity: Option<&Json>,
    now_ms: i64,
) -> Result<(), CommandError> {
    validate_command_name(&definition.name)?;
    if !(0..=253_402_300_799_999).contains(&now_ms) {
        return Err(CommandError::InvalidCommand);
    }
    args.canonical()?;
    identity.ok_or(CommandError::InvalidCommand)?.canonical()?;
    let Json::Object(values) = args else {
        return Err(CommandError::InvalidCommand);
    };
    if values
        .keys()
        .any(|key| !definition.arguments.contains_key(key))
    {
        return Err(CommandError::InvalidCommand);
    }
    for (name, arg) in &definition.arguments {
        match values.get(name) {
            None if arg.optional => {}
            Some(Json::Null) if arg.optional => {}
            Some(value)
                if match arg.kind {
                    ArgumentType::Any => true,
                    ArgumentType::String => matches!(value, Json::String(_)),
                    ArgumentType::Integer => matches!(value, Json::Integer(_)),
                    ArgumentType::Float => matches!(value, Json::Float(_)),
                    ArgumentType::Boolean => matches!(value, Json::Bool(_)),
                    ArgumentType::Array => matches!(value, Json::Array(_)),
                    ArgumentType::Object => matches!(value, Json::Object(_)),
                } => {}
            _ => return Err(CommandError::InvalidCommand),
        }
    }
    let mut paths = BTreeSet::new();
    for path in &definition.reference_paths {
        if path.is_empty()
            || !paths.insert(path)
            || !matches!(path_value(args,path),Some(Json::String(value)) if !value.is_empty())
        {
            return Err(CommandError::InvalidCommand);
        }
    }
    Ok(())
}

/// The exact queued record a command invocation enqueues before its staged
/// creations and undo images are known: the next key and order of `header`.
fn queued_command(
    header: &SyncHeader,
    schema_version: u32,
    definition: &CommandDefinition,
    args: Json,
    identity: Json,
    now_ms: i64,
    dependencies: BTreeSet<String>,
) -> SyncRecord {
    SyncRecord {
        entity: None,
        metadata_undo: vec![],
        key: format!(
            "{:016x}-{}-{:016x}",
            header.last_ms.max(now_ms),
            header.namespace,
            header.sequence
        ),
        command: definition.name.clone(),
        args,
        context: Json::Object(BTreeMap::from([
            ("identity".into(), identity),
            ("capturedAt".into(), Json::String(utc_time(now_ms))),
        ])),
        schema_version,
        order: header.sequence,
        reference_paths: definition.reference_paths.clone(),
        dependencies,
        created: BTreeMap::new(),
        mappings: BTreeMap::new(),
        undo: vec![],
        state: SyncState::Queued,
        frozen: None,
        attempts: 0,
        due_ms: 0,
        outcome: None,
        rolled_back: false,
        reconciled: false,
    }
}

fn path_mut<'a>(mut value: &'a mut Json, path: &[PathPart]) -> Option<&'a mut Json> {
    for part in path {
        value = match (part, value) {
            (PathPart::Key(key), Json::Object(values)) => values.get_mut(key)?,
            (PathPart::Index(index), Json::Array(values)) => values.get_mut(*index)?,
            _ => return None,
        };
    }
    Some(value)
}

/// Rewrite only an unpublished candidate; discard it if a later path errors.
pub(super) fn normalize_references(
    args: &mut Json,
    paths: &[Vec<PathPart>],
    ledger: &SyncLedger,
) -> Result<BTreeSet<String>, CommandError> {
    normalize_references_with_aliases(args, paths, ledger, None)
}

fn normalize_references_with_aliases(
    args: &mut Json,
    paths: &[Vec<PathPart>],
    ledger: &SyncLedger,
    aliases: Option<&super::durable::AliasState>,
) -> Result<BTreeSet<String>, CommandError> {
    let mut encoded_bytes = args.canonical()?.len();
    let mut dependencies = BTreeSet::new();
    for path in paths {
        let Some(Json::String(value)) = path_mut(args, path) else {
            return Err(CommandError::InvalidCommand);
        };
        if let Some(aliases) = aliases.filter(|aliases| aliases.enabled) {
            let resolved = aliases.resolve(None, value)?;
            let before = Json::String(value.clone()).canonical()?.len();
            let after = Json::String(resolved.clone()).canonical()?.len();
            encoded_bytes = encoded_bytes
                .checked_sub(before)
                .and_then(|length| length.checked_add(after))
                .filter(|length| *length <= super::json::MAX_BYTES)
                .ok_or(CommandError::InvalidPayload(JsonError::LimitExceeded))?;
            *value = resolved;
        }
        let known = ledger
            .records
            .values()
            .find(|record| record.created.contains_key(value));
        if value.starts_with("local:") || known.is_some() {
            let producer = known.ok_or(CommandError::UnknownLocalReference)?;
            if matches!(producer.state, SyncState::Cancelled | SyncState::Discarded)
                || producer.rolled_back
            {
                return Err(CommandError::UnknownLocalReference);
            }
            if let Some(mapped) = producer.mappings.get(value) {
                // Container shape and node count do not change. Account for the
                // codec's exact escaped string lengths before retaining a replacement.
                let before = Json::String(value.clone()).canonical()?.len();
                let after = Json::String(mapped.clone()).canonical()?.len();
                encoded_bytes = encoded_bytes
                    .checked_sub(before)
                    .and_then(|length| length.checked_add(after))
                    .filter(|length| *length <= super::json::MAX_BYTES)
                    .ok_or(CommandError::InvalidPayload(JsonError::LimitExceeded))?;
                *value = mapped.clone();
            } else {
                dependencies.insert(value.clone());
            }
        }
    }
    Ok(dependencies)
}

pub(super) fn initial_guards(changes: &[CommittedChange], version: u32) -> Vec<RowGuard> {
    let mut guards = BTreeMap::new();
    for change in changes {
        guards
            .entry((change.table.clone(), change.id.clone()))
            .or_insert_with(|| {
                count_work!(rows_cloned, u64::from(change.before.is_some()));
                RowGuard {
                    table: change.table.clone(),
                    id: change.id.clone(),
                    expected: change.before.clone().map(|mut row| {
                        row.fields
                            .insert(ROW_VERSION.into(), Value::Integer(i64::from(version)));
                        row
                    }),
                }
            });
    }
    guards.into_values().collect()
}

pub(super) fn utc_time(now_ms: i64) -> String {
    let seconds = now_ms / 1000;
    let days = seconds / 86400;
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp
        + if mp < 10 {
            3
        } else {
            -9
        };
    year += i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        seconds % 86400 / 3600,
        seconds % 3600 / 60,
        seconds % 60,
        now_ms % 1000
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{EntityConfig, EntityContext};
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };

    fn run<F: Future>(future: F) -> F::Output {
        match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("snapshot unexpectedly suspended"),
        }
    }

    #[test]
    fn g07_owner_proposals_preserve_command_prefix_and_strict_entity_allocator() {
        let _fixture = crate::command::TEST_OWNERS.lock().unwrap();
        let schema = SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([
                ("items".into(), TableDefinition::default()),
                ("other".into(), TableDefinition::default()),
            ]),
        };
        let mut db = run(CommandDatabase::open(
            SnapshotStorage::default(),
            schema,
            vec![],
            [18; 16],
            SyncOptions::default(),
        ))
        .unwrap();
        db.configure_entities(
            EntityConfig {
                scope: "g07".into(),
                tables: BTreeSet::from(["items".into()]),
            },
            || {
                Ok(EntityContext {
                    identity: Json::Null,
                    now_ms: 0,
                })
            },
        )
        .unwrap();
        assert_eq!(
            run(db.insert_proposed("other", BTreeMap::new(), Some("test-fixed-id-001".into())))
                .unwrap()
                .id,
            "test-fixed-id-001"
        );
        assert_eq!(
            run(db.insert_proposed("items", BTreeMap::new(), Some("entity:forged".into()))),
            Err(CommandError::InvalidCommand)
        );
        let entity = run(db.insert_proposed("items", BTreeMap::new(), None)).unwrap();
        assert!(
            entity
                .id
                .starts_with("entity:12121212121212121212121212121212:")
        );
        let request = db.request().unwrap();
        let definition = CommandDefinition {
            name: "create-v1".into(),
            arguments: BTreeMap::new(),
            reference_paths: vec![],
        };
        assert!(
            run(db.begin(
                request,
                definition,
                Json::Object(BTreeMap::new()),
                Some(Json::Null),
                0
            ))
            .unwrap()
        );
        let row = run(db.scope_insert_proposed(
            request,
            "other",
            BTreeMap::new(),
            Some("local:test-fixed-id-001".into()),
        ))
        .unwrap();
        assert_eq!(row.id, "local:test-fixed-id-001");
        run(db.finish(request, Json::Null)).unwrap();
        assert_eq!(
            db.queue_status().pending,
            2,
            "one entity plus one command, no duplicate entity enqueue"
        );
        let request = db.request().unwrap();
        assert!(
            run(db.begin(
                request,
                CommandDefinition {
                    name: "invalid-v1".into(),
                    arguments: BTreeMap::new(),
                    reference_paths: vec![]
                },
                Json::Object(BTreeMap::new()),
                Some(Json::Null),
                0
            ))
            .unwrap()
        );
        run(db.scope_insert_proposed(
            request,
            "other",
            BTreeMap::new(),
            Some("not-a-command-local".into()),
        ))
        .unwrap();
        assert_eq!(
            run(db.finish(request, Json::Null)),
            Err(CommandError::InvalidCommand)
        );
        assert_eq!(db.queue_status().pending, 2);
        assert!(
            !run(db.query("other").fetch(&db))
                .unwrap()
                .iter()
                .any(|row| row.id == "not-a-command-local")
        );
    }

    #[test]
    fn g07_proposed_ids_are_exact_atomic_and_preserve_generator_custody() {
        let _fixture = crate::command::TEST_OWNERS.lock().unwrap();
        let schema = SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([("items".into(), TableDefinition::default())]),
        };
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = calls.clone();
        let mut db = run(Database::open(SnapshotStorage::default(), schema, vec![]))
            .unwrap()
            .with_id_generator(move || {
                format!(
                    "generated:{}",
                    observed.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                )
            });
        let insert = || Write::Insert {
            table: "items".into(),
            fields: BTreeMap::new(),
        };
        let changes =
            run(db.apply_batch_proposed(vec![(insert(), Some("test-fixed-id-001".into()))]))
                .unwrap();
        assert_eq!(changes[0].id, "test-fixed-id-001");
        assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 0);
        let before = run(db.query("items").fetch(&db)).unwrap();
        for writes in [
            vec![(insert(), Some(String::new()))],
            vec![
                (insert(), Some("duplicate".into())),
                (insert(), Some("duplicate".into())),
            ],
            vec![(
                Write::Remove {
                    table: "items".into(),
                    id: "test-fixed-id-001".into(),
                },
                Some("not-an-insertion".into()),
            )],
        ] {
            assert!(run(db.apply_batch_proposed(writes)).is_err());
            assert_eq!(run(db.query("items").fetch(&db)).unwrap(), before);
        }
        assert_eq!(
            run(db.insert("items", BTreeMap::new())).unwrap().id,
            "generated:0"
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[test]
    fn g22_consuming_storage_aborts_staging_and_releases_active_and_queued_tickets() {
        let _fixture = crate::command::TEST_OWNERS.lock().unwrap();
        let schema = SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([("items".into(), TableDefinition::default())]),
        };
        let open = |storage| {
            run(CommandDatabase::open(
                storage,
                schema.clone(),
                vec![],
                [17; 16],
                SyncOptions::default(),
            ))
            .unwrap()
        };
        let definition = CommandDefinition {
            name: "create-v1".into(),
            arguments: BTreeMap::new(),
            reference_paths: vec![],
        };
        let mut a = open(SnapshotStorage::default());
        let mut b = open(SnapshotStorage::default());
        // SPEC-0037 R3: owned stores admit per database; share one coordinator so
        // consuming A's storage must release the ticket B is queued behind.
        let shared = ExecutionCoordinator::global();
        a.execution_owner = shared.owner().unwrap();
        b.execution_owner = shared.owner().unwrap();
        let committed = run(a.insert("items", BTreeMap::new())).unwrap();
        let finished = a.request().unwrap();
        assert!(
            run(a.begin(
                finished,
                definition.clone(),
                Json::Object(BTreeMap::new()),
                Some(Json::Null),
                0
            ))
            .unwrap()
        );
        let command_row = run(a.scope_insert(finished, "items", BTreeMap::new())).unwrap();
        run(a.finish(finished, Json::Null)).unwrap();
        let active = a.request().unwrap();
        assert!(
            run(a.begin(
                active,
                definition.clone(),
                Json::Object(BTreeMap::new()),
                Some(Json::Null),
                0
            ))
            .unwrap()
        );
        let discarded = run(a.scope_insert(active, "items", BTreeMap::new())).unwrap();
        let abandoned = a.request().unwrap();
        let next = b.request().unwrap();
        assert!(
            !run(b.begin(
                next,
                definition.clone(),
                Json::Object(BTreeMap::new()),
                Some(Json::Null),
                0
            ))
            .unwrap()
        );
        let storage = a.into_storage();
        assert!(
            run(b.begin(
                next,
                definition.clone(),
                Json::Object(BTreeMap::new()),
                Some(Json::Null),
                0
            ))
            .unwrap()
        );
        b.abort(next).unwrap();
        let mut reopened = open(storage);
        let rows = run(reopened.query("items").fetch(&reopened)).unwrap();
        assert_eq!(rows, vec![committed, command_row]);
        assert_eq!(
            reopened.queue_status().pending,
            1,
            "committed recovery survives"
        );
        assert_eq!(
            run(reopened.insert("items", BTreeMap::new())).unwrap().id,
            discarded.id
        );
        assert_eq!(
            run(reopened.begin(
                abandoned,
                definition,
                Json::Object(BTreeMap::new()),
                Some(Json::Null),
                0
            )),
            Err(CommandError::InvalidTicket)
        );
    }

    #[test]
    fn c06_net_removed_creation_keeps_undo_and_counter_without_mapping_owner() {
        let _fixture = crate::command::TEST_OWNERS.lock().unwrap();
        let schema = SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([("items".into(), TableDefinition::default())]),
        };
        let mut db = run(CommandDatabase::open(
            SnapshotStorage::default(),
            schema,
            vec![],
            [7; 16],
            SyncOptions::default(),
        ))
        .unwrap();
        let request = db.request().unwrap();
        let definition = CommandDefinition {
            name: "noop-v1".into(),
            arguments: BTreeMap::new(),
            reference_paths: vec![],
        };
        run(db.begin(
            request,
            definition,
            Json::Object(BTreeMap::new()),
            Some(Json::Null),
            0,
        ))
        .unwrap();
        let inserted = run(db.scope_insert(request, "items", BTreeMap::new())).unwrap();
        assert!(run(db.scope_remove(request, "items", &inserted.id)).unwrap());
        let result = run(db.finish(request, Json::Null)).unwrap();
        let record = &db.ledger.records[&result.key];
        assert!(
            record.created.is_empty(),
            "net-removed rows must not require authoritative ID mapping"
        );
        assert_eq!(record.undo.len(), 2);
        assert_eq!(record.undo[0].after.as_ref().unwrap().id, inserted.id);
        assert_eq!(record.undo[1].before.as_ref().unwrap().id, inserted.id);
        assert_eq!(db.db.db.state.next_id, 2);
        assert!(run(db.query("items").fetch(&db)).unwrap().is_empty());
    }

    #[test]
    fn c05_argument_kinds_optional_values_and_utc_limits() {
        assert_eq!(utc_time(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(utc_time(951_782_400_123), "2000-02-29T00:00:00.123Z");
        assert_eq!(utc_time(253_402_300_799_999), "9999-12-31T23:59:59.999Z");
        let identity = Some(Json::Null);
        for (kind, value) in [
            (ArgumentType::Any, Json::Null),
            (ArgumentType::String, Json::String("é".into())),
            (ArgumentType::Integer, Json::Integer(i64::MAX)),
            (
                ArgumentType::Float,
                Json::Float(FiniteFloat::new(42.0).unwrap()),
            ),
            (ArgumentType::Boolean, Json::Bool(true)),
            (ArgumentType::Array, Json::Array(vec![Json::Null])),
            (ArgumentType::Object, Json::Object(BTreeMap::new())),
        ] {
            let mut definition = CommandDefinition {
                name: "typed-v1".into(),
                arguments: BTreeMap::from([(
                    "value".into(),
                    Argument {
                        kind,
                        optional: false,
                    },
                )]),
                reference_paths: vec![],
            };
            let args = Json::Object(BTreeMap::from([("value".into(), value)]));
            validate_invocation(&definition, &args, identity.as_ref(), 0).unwrap();
            assert_eq!(
                validate_invocation(&definition, &args, None, 0),
                Err(CommandError::InvalidCommand)
            );
            assert_eq!(
                validate_invocation(&definition, &args, identity.as_ref(), -1),
                Err(CommandError::InvalidCommand)
            );
            assert_eq!(
                validate_invocation(&definition, &args, identity.as_ref(), 253_402_300_800_000),
                Err(CommandError::InvalidCommand)
            );
            definition.arguments.get_mut("value").unwrap().optional = true;
            validate_invocation(
                &definition,
                &Json::Object(BTreeMap::new()),
                identity.as_ref(),
                0,
            )
            .unwrap();
            validate_invocation(
                &definition,
                &Json::Object(BTreeMap::from([("value".into(), Json::Null)])),
                identity.as_ref(),
                0,
            )
            .unwrap();
        }
    }

    #[test]
    fn c05_cached_mapping_expansion_remains_bounded_before_scope_admission() {
        let namespace = "08".repeat(16);
        let key = format!("0000000000000000-{namespace}-0000000000000001");
        let mut record = SyncRecord {
            entity: None,
            metadata_undo: vec![],
            key: key.clone(),
            command: "seed-v1".into(),
            args: Json::Object(BTreeMap::new()),
            context: Json::Object(BTreeMap::from([
                ("identity".into(), Json::Null),
                (
                    "capturedAt".into(),
                    Json::String("1970-01-01T00:00:00.000Z".into()),
                ),
            ])),
            schema_version: 1,
            order: 1,
            reference_paths: vec![],
            dependencies: BTreeSet::new(),
            created: BTreeMap::from([("local:x".into(), "items".into())]),
            mappings: BTreeMap::from([("local:x".into(), "r".repeat(600_000))]),
            undo: vec![],
            state: SyncState::Applied,
            frozen: None,
            attempts: 1,
            due_ms: 0,
            outcome: Some(RemoteOutcome::Applied(Json::Null)),
            rolled_back: false,
            reconciled: true,
        };
        record.frozen = Some(record.envelope().canonical().unwrap());
        let ledger = SyncLedger {
            entity: None,
            header: SyncHeader {
                namespace,
                sequence: 2,
                last_ms: 0,
                options: SyncOptions::default(),
            },
            records: BTreeMap::from([(key, record)]),
        };
        ledger.to_rows().unwrap();
        let mut args = Json::Object(BTreeMap::from([
            ("first".into(), Json::String("local:x".into())),
            ("second".into(), Json::String("local:x".into())),
        ]));
        let paths = vec![
            vec![PathPart::Key("first".into())],
            vec![PathPart::Key("second".into())],
        ];
        assert!(
            matches!(
                normalize_references(&mut args, &paths, &ledger),
                Err(CommandError::InvalidPayload(JsonError::LimitExceeded))
            ),
            "cached mapping expansion must reject before an oversized scope is retained"
        );
    }

    fn nested(containers: usize) -> Json {
        (0..containers).fold(Json::Null, |value, _| Json::Array(vec![value]))
    }

    #[test]
    fn c05_begin_applies_the_record_depth_bound_finish_enforces() {
        let _fixture = crate::command::TEST_OWNERS.lock().unwrap();
        let schema = SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([("items".into(), TableDefinition::default())]),
        };
        let mut db = run(CommandDatabase::open(
            SnapshotStorage::default(),
            schema,
            vec![],
            [5; 16],
            SyncOptions::default(),
        ))
        .unwrap();
        let definition = CommandDefinition {
            name: "deep-v1".into(),
            arguments: BTreeMap::from([(
                "value".into(),
                Argument {
                    kind: ArgumentType::Any,
                    optional: false,
                },
            )]),
            reference_paths: vec![],
        };
        let args =
            |containers| Json::Object(BTreeMap::from([("value".into(), nested(containers))]));
        // Standalone values at the codec's own depth limit; the stored record and
        // envelope nest args one level and identity two levels deeper.
        args(63).canonical().unwrap();
        nested(63).canonical().unwrap();
        for (args, identity) in [(args(63), Json::Null), (args(0), nested(63))] {
            let request = db.request().unwrap();
            assert_eq!(
                run(db.begin(request, definition.clone(), args, Some(identity), 0)),
                Err(CommandError::LimitExceeded),
                "an unstorable invocation must fail before its handler runs"
            );
            assert_eq!(db.cancel_request(request), Err(CommandError::InvalidTicket));
        }
        // The deepest storable shapes still run and enqueue.
        let request = db.request().unwrap();
        assert!(run(db.begin(request, definition, args(62), Some(nested(62)), 0)).unwrap());
        run(db.finish(request, Json::Null)).unwrap();
        assert_eq!(db.queue_status().pending, 1);
    }

    #[test]
    fn adoption_refuses_default_generated_local_ids_the_entity_path_rejects() {
        let _fixture = crate::command::TEST_OWNERS.lock().unwrap();
        let schema = SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([
                ("items".into(), TableDefinition::default()),
                ("other".into(), TableDefinition::default()),
            ]),
        };
        let open = |storage| {
            run(CommandDatabase::open(
                storage,
                schema.clone(),
                vec![],
                [6; 16],
                SyncOptions::default(),
            ))
            .unwrap()
        };
        let config = |tables: &[&str]| EntityConfig {
            scope: "seeded".into(),
            tables: tables.iter().map(|table| (*table).into()).collect(),
        };
        let provider = || {
            Ok(EntityContext {
                identity: Json::Null,
                now_ms: 0,
            })
        };
        let mut db = open(SnapshotStorage::default());
        // No generator and no proposal: the standalone default allocates `local:N`.
        let seeded = run(db.insert("items", BTreeMap::new())).unwrap();
        assert_eq!(seeded.id, "local:1");
        // API-0009: the entity path rejects an unknown `local:` ID, so adopting
        // this table would leave the row unwritable. Refuse before any mutation.
        for durable in [false, true] {
            let refused = if durable {
                db.configure_entities_durable(config(&["items"]), provider)
            } else {
                db.configure_entities(config(&["items"]), provider)
            };
            assert_eq!(refused, Err(CommandError::Protocol));
            assert!(db.ledger.entity.is_none() && !db.aliases.enabled);
            assert!(db.entity_provider.is_none() && db.persisted_rows.is_none());
        }
        // The refused store stays an ordinary writable store.
        run(db.replace("items", &seeded.id, BTreeMap::new())).unwrap();
        assert_eq!(
            run(db.query("items").fetch(&db)).unwrap(),
            vec![seeded.clone()]
        );
        // Only tables being configured are examined.
        db.configure_entities(config(&["other"]), provider).unwrap();
        // Seeded rows with ordinary proposed IDs remain adoptable and writable.
        let mut db = open(SnapshotStorage::default());
        let legacy =
            run(db.insert_proposed("items", BTreeMap::new(), Some("legacy-1".into()))).unwrap();
        run(db.insert("other", BTreeMap::new())).unwrap();
        db.configure_entities(config(&["items"]), provider).unwrap();
        run(db.replace("items", &legacy.id, BTreeMap::new())).unwrap();
        assert_eq!(db.queue_status().pending, 1);
        // Removing the reserved row makes the same store adoptable.
        let mut db = open(SnapshotStorage::default());
        let seeded = run(db.insert("items", BTreeMap::new())).unwrap();
        assert!(run(db.remove("items", &seeded.id)).unwrap());
        db.configure_entities(config(&["items"]), provider).unwrap();
    }
}

#[cfg(test)]
mod proof_boundary_tests {
    use super::*;
    use crate::command::{EntityConfig, EntityContext, entity::*, phase3_cases::run};

    fn open(schema: SchemaDefinition) -> CommandDatabase<SnapshotStorage> {
        run(CommandDatabase::open(
            SnapshotStorage::default(),
            schema,
            vec![],
            [0x70; 16],
            SyncOptions::default(),
        ))
        .unwrap()
    }
    fn config() -> EntityConfig {
        EntityConfig {
            scope: "proof-boundary".into(),
            tables: BTreeSet::from(["items".into()]),
        }
    }
    fn assert_owner_matches_store(
        db: &CommandDatabase<SnapshotStorage>,
        before: &SyncLedger,
        packets: &Option<Vec<Row>>,
        epoch: u64,
    ) {
        assert_eq!(&db.ledger, before);
        assert_eq!(&db.db.db.storage.load_sync().unwrap(), packets);
        assert_eq!(db.ledger_epoch, epoch);
    }
    #[test]
    fn si_y01_late_planner_error_restores_headers_records_metadata_and_caches() {
        let (mut db, key) = crate::command::phase3_cases::owner();
        let before = db.ledger.clone();
        let packets = db.db.db.storage.load_sync().unwrap();
        let epoch = db.ledger_epoch;
        let ranks = db.metadata_ordinals.clone();
        let cache = db.tombstones;
        let model = db.db.db.state.clone();
        db.observe_progress().unwrap();
        db.drain_events(0);
        let permit = db.execution_owner.idle_permit().unwrap();
        let result = db.transact::<()>(model.clone(), &permit, |txn| {
            txn.header_mut().last_ms += 1;
            txn.record_mut(&key).unwrap().due_ms += 1;
            txn.edit_entity_header(|header| {
                header.pages += 1;
                Ok(())
            })?;
            txn.put_metadata(
                ("items".into(), "late".into()),
                RowMetadata {
                    fields: BTreeMap::new(),
                    tombstone: Some(FieldStamp {
                        timestamp_ms: 1,
                        authoritative: false,
                    }),
                },
            )?;
            Err(CommandError::InvalidCommand)
        });
        assert_eq!(result.err(), Some(CommandError::InvalidCommand));
        assert_owner_matches_store(&db, &before, &packets, epoch);
        assert_eq!(db.metadata_ordinals, ranks);
        assert_eq!(db.tombstones, cache);
        assert_eq!(db.db.db.state, model);
        assert!(db.drain_events(0).is_empty());
    }

    #[test]
    fn si_y05_format2_rank_changes_write_exact_suffix_and_restore_on_drop() {
        for m in [10usize, 1000] {
            for position in [0, m / 2, m] {
                let mut entity = header_from_json(&Json::parse(r#"{"config":{"scope":"proof-boundary","tables":["items"]},"generation":1,"revision":0,"checkpoint":null,"ready":false,"pages":0,"changes":0}"#).unwrap()).unwrap();
                for ordinal in 0..m {
                    entity.metadata.insert(
                        ("items".into(), format!("r{:06}", 2 * ordinal)),
                        RowMetadata::default(),
                    );
                }
                let original = SyncLedger {
                    header: SyncHeader {
                        namespace: "7".repeat(32),
                        sequence: 1,
                        last_ms: 0,
                        options: SyncOptions::default(),
                    },
                    entity: Some(entity),
                    records: BTreeMap::new(),
                };
                let packets = original
                    .to_rows()
                    .unwrap()
                    .into_iter()
                    .map(|row| (row.id.clone(), row))
                    .collect::<BTreeMap<_, _>>();
                let ranks = metadata_inventory(&original).0;
                let id = if position == 0 {
                    "q".into()
                } else {
                    format!("r{:06}", 2 * position - 1)
                };
                let mut candidate = original.clone();
                let mut txn = LedgerTxn::new(&mut candidate, true);
                txn.put_metadata(("items".into(), id), RowMetadata::default())
                    .unwrap();
                let (changes, rank_changes) =
                    packet_changes(&txn, Some(&packets), &ranks, &txn.changed_metadata()).unwrap();
                assert_eq!(
                    changes.keys().cloned().collect::<BTreeSet<_>>(),
                    (position..=m)
                        .map(|rank| format!("entity:metadata:{rank:016x}"))
                        .collect()
                );
                let mut merged = packets.clone();
                for (id, value) in changes {
                    if let Some(value) = value {
                        merged.insert(id, value);
                    } else {
                        merged.remove(&id);
                    }
                }
                assert_eq!(
                    SyncLedger::from_rows(&merged.into_values().collect::<Vec<_>>()).unwrap(),
                    *txn
                );
                let mut next_ranks = ranks.clone();
                for (identity, rank) in rank_changes {
                    if let Some(rank) = rank {
                        next_ranks.insert(identity, rank);
                    } else {
                        next_ranks.remove(&identity);
                    }
                }
                assert_eq!(next_ranks, metadata_inventory(&txn).0);
                drop(txn);
                assert_eq!(candidate, original);
                assert_eq!(ranks, metadata_inventory(&candidate).0);
            }
        }
    }

    #[test]
    fn si_w03_initial_guard_counts_only_the_first_row_image() {
        let row = Row {
            id: "x".into(),
            fields: BTreeMap::new(),
        };
        let change = CommittedChange {
            table: "items".into(),
            id: "x".into(),
            before: Some(row),
            after: None,
        };
        let changes = vec![change.clone(), change];
        counters::reset();
        assert_eq!(initial_guards(&changes, 1).len(), 1);
        assert_eq!(counters::snapshot().rows_cloned, 1);
    }

    #[test]
    fn si_y03_adoption_refuses_live_recovery_before_delta_and_preserves_owner() {
        let mut db = open(SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([("items".into(), TableDefinition::default())]),
        });
        let ticket = db.request().unwrap();
        assert!(
            run(db.begin(
                ticket,
                CommandDefinition {
                    name: "live-v1".into(),
                    arguments: BTreeMap::new(),
                    reference_paths: vec![]
                },
                Json::Object(BTreeMap::new()),
                Some(Json::Null),
                1
            ))
            .unwrap()
        );
        run(db.finish(ticket, Json::Null)).unwrap();
        let before = db.ledger.clone();
        let packets = db.db.db.storage.load_sync().unwrap();
        let epoch = db.ledger_epoch;
        let permit = db.execution_owner.idle_permit().unwrap();
        let result = db.transact(db.db.db.state.clone(), &permit, |txn| {
            txn.adopt_entity(EntityLedger::new(config()))?;
            Ok(((), vec![], vec![]))
        });
        assert!(matches!(
            result,
            Err(CommandError::Storage(StorageError::SchemaConflict { .. }))
        ));
        assert_owner_matches_store(&db, &before, &packets, epoch);
        assert!(db.ledger.entity.is_none());
    }
    #[test]
    fn si_y03_retained_frozen_envelope_cannot_be_replaced_by_another_valid_envelope() {
        let (mut db, key) = crate::command::phase3_cases::owner();
        let before = db.ledger.clone();
        let packets = db.db.db.storage.load_sync().unwrap();
        let epoch = db.ledger_epoch;
        let permit = db.execution_owner.idle_permit().unwrap();
        let result = db.transact(db.db.db.state.clone(), &permit, |txn| {
            let record = txn.record_mut(&key).unwrap();
            record.command = "changed-v1".into();
            record.frozen = Some(record.envelope().canonical()?);
            txn.validate()?; // The new record is internally valid; the prior envelope is the proof.
            Ok(((), vec![], vec![]))
        });
        assert!(matches!(
            result,
            Err(CommandError::Storage(StorageError::SchemaConflict { .. }))
        ));
        assert_owner_matches_store(&db, &before, &packets, epoch);
    }
    #[test]
    fn si_y03_touched_metadata_accepts_normalized_relation_columns() {
        let mut db = open(SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([
                ("parents".into(), TableDefinition::default()),
                (
                    "items".into(),
                    TableDefinition {
                        relations: BTreeMap::from([(
                            "parent".into(),
                            RelationDefinition::ToOne {
                                table: "parents".into(),
                                foreign_key: "parent_id".into(),
                                optional: false,
                            },
                        )]),
                        ..Default::default()
                    },
                ),
            ]),
        });
        db.configure_entities(config(), || {
            Ok(EntityContext {
                identity: Json::Null,
                now_ms: 1,
            })
        })
        .unwrap();
        let parent =
            run(db.insert_proposed("parents", BTreeMap::new(), Some("server:parent".into())))
                .unwrap();
        let result = run(db.insert(
            "items",
            BTreeMap::from([("parent_id".into(), Value::Reference(parent.id))]),
        ));
        assert!(
            result.is_ok(),
            "generated relation columns are valid comparison metadata: {result:?}"
        );
    }
    #[test]
    fn si_y03_touched_metadata_rejects_undeclared_columns_with_full_validator_error() {
        let mut db = open(SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([("items".into(), TableDefinition::default())]),
        });
        db.configure_entities(config(), || {
            Ok(EntityContext {
                identity: Json::Null,
                now_ms: 1,
            })
        })
        .unwrap();
        let value = RowMetadata {
            fields: BTreeMap::from([(
                "missing".into(),
                FieldStamp {
                    timestamp_ms: 1,
                    authoritative: false,
                },
            )]),
            tombstone: None,
        };
        let mut oracle = db.ledger.entity.clone().unwrap();
        oracle
            .metadata
            .insert(("items".into(), "x".into()), value.clone());
        let expected = CommandError::Storage(oracle.validate(&db.db.db.schema).unwrap_err());
        let before = db.ledger.clone();
        let packets = db.db.db.storage.load_sync().unwrap();
        let epoch = db.ledger_epoch;
        let permit = db.execution_owner.idle_permit().unwrap();
        let result = db.transact(db.db.db.state.clone(), &permit, |txn| {
            txn.put_metadata(("items".into(), "x".into()), value)?;
            Ok(((), vec![], vec![]))
        });
        assert_eq!(result.err(), Some(expected));
        assert_owner_matches_store(&db, &before, &packets, epoch);
    }
    #[test]
    fn si_f02_owner_proof_boundary_retains_entity_format() {
        let (db, _) = crate::command::phase3_cases::owner();
        for (from, to) in [(2, 3), (3, 2)] {
            let mut ledger = db.ledger.clone();
            ledger.entity.as_mut().unwrap().ledger_format = from;
            let mut txn = LedgerTxn::new(&mut ledger, true);
            assert!(
                validate_ledger_transition(
                    &txn,
                    &db.db.db.state,
                    &db.db.db.state,
                    &[],
                    &[],
                    &BTreeSet::new()
                )
                .is_ok()
            );
            txn.edit_entity_header(|header| {
                header.ledger_format = to;
                Ok(())
            })
            .unwrap();
            assert!(
                validate_ledger_transition(
                    &txn,
                    &db.db.db.state,
                    &db.db.db.state,
                    &[],
                    &[],
                    &BTreeSet::new()
                )
                .is_err(),
                "owner proof admitted implicit format {from}->{to}"
            );
        }
    }
}
