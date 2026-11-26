//! API-0006 section 5: durable transitions and owned sync notifications.
use super::{records::SyncLedger, *};
use crate::{model::ROW_VERSION, storage::*};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Default)]
pub(super) struct Delivery {
    watchers: BTreeMap<SyncSubscription, Watch>,
    pending: Vec<CommandEvent>,
}

enum Watch {
    Progress(SyncProgress),
    Status(QueueStatus),
    Outcome {
        key: String,
        value: CommandOutcome,
    },
}

static NEXT_SUBSCRIPTION: AtomicU64 = AtomicU64::new(1);

fn token() -> Result<SyncSubscription, CommandError> {
    NEXT_SUBSCRIPTION
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
        .map(SyncSubscription)
        .map_err(|_| CommandError::LimitExceeded)
}

impl Delivery {
    fn terminal(&mut self, key: &str, value: CommandOutcome) {
        for (subscription, watch) in &mut self.watchers {
            if let Watch::Outcome {
                key: watched,
                value: previous,
            } = watch
                && watched == key
                && *previous != value
            {
                *previous = value.clone();
                self.pending.push(CommandEvent::Outcome {
                    subscription: *subscription,
                    value: value.clone(),
                });
            }
        }
    }

    pub(super) fn changed(&mut self, ledger: &SyncLedger, tombstones: usize) {
        for (subscription, watch) in &mut self.watchers {
            match watch {
                Watch::Progress(previous) => {
                    let value = super::pull::progress(ledger, tombstones);
                    if value != *previous {
                        *previous = value.clone();
                        self.pending.push(CommandEvent::Progress {
                            subscription: *subscription,
                            value,
                        });
                    }
                }
                Watch::Status(previous) => {
                    let value = queue::status(ledger);
                    if value != *previous {
                        *previous = value.clone();
                        self.pending.push(CommandEvent::Status {
                            subscription: *subscription,
                            value,
                        });
                    }
                }
                Watch::Outcome {
                    key,
                    value: previous,
                } => {
                    if let Some(value) = ledger.records.get(key).and_then(queue::outcome)
                        && value != *previous
                    {
                        *previous = value.clone();
                        self.pending.push(CommandEvent::Outcome {
                            subscription: *subscription,
                            value,
                        });
                    }
                }
            }
        }
    }
}

impl<S: SyncStorage> CommandDatabase<S> {
    pub fn prepare_push(
        &mut self,
        now_ms: i64,
        jitter_basis_points: u16,
    ) -> Result<PushBatch, CommandError> {
        let permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        // A queue-free, unpersisted owner only observes this wake. Dropping the
        // guard restores its identity floor and leaves recovery storage absent.
        if self.persisted_rows.is_none() {
            let mut txn =
                super::ledger_txn::LedgerTxn::new(&mut self.ledger, self.db.db.validated_exclusive);
            return queue::push_in(&mut txn, now_ms, jitter_basis_points);
        }
        let (batch, _) = self.transact(self.db.db.state.clone(), &permit, |next| {
            let batch = queue::push_in(next, now_ms, jitter_basis_points)?;
            Ok((batch, vec![], vec![]))
        })?;
        Ok(batch)
    }

    pub fn receive(
        &mut self,
        key: &str,
        outcome: Option<RemoteOutcome>,
    ) -> Result<(), CommandError> {
        let permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        self.transact(self.db.db.state.clone(), &permit, |next| {
            queue::receive_in(next, key, outcome)?;
            Ok(((), vec![], vec![]))
        })?;
        Ok(())
    }

    /// SPEC-0038 C3: explicitly rearm an Acknowledged record whose final outcome was lost.
    /// It moves back to Sent, due at the validated raw sample, with unchanged frozen bytes,
    /// attempts and key; an already Sent record is a no-op. It never transmits.
    pub fn request_redelivery(&mut self, key: &str, now_ms: i64) -> Result<(), CommandError> {
        let permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        self.transact(self.db.db.state.clone(), &permit, |next| {
            queue::redeliver_in(next, key, now_ms)?;
            Ok(((), vec![], vec![]))
        })?;
        Ok(())
    }

    pub fn pending_reconciliations(&self) -> Vec<(String, RemoteOutcome)> {
        let mut records = self
            .ledger
            .records
            .values()
            .filter(|r| !r.reconciled && r.state != SyncState::Cancelled)
            .collect::<Vec<_>>();
        records.sort_by_key(|r| r.order);
        records
            .into_iter()
            .filter_map(|r| r.outcome.clone().map(|outcome| (r.key.clone(), outcome)))
            .collect()
    }

    /// API-0012: persisted identity facts for a retained pending outcome, without admission.
    pub fn reconciliation_context(
        &self,
        key: &str,
    ) -> Option<(QueuedWriteKind, BTreeMap<String, String>)> {
        let record = self.ledger.records.get(key)?;
        if record.reconciled || record.state == SyncState::Cancelled || record.outcome.is_none() {
            return None;
        }
        let kind = if record.entity.is_some() {
            QueuedWriteKind::Entity
        } else {
            QueuedWriteKind::Command
        };
        Some((kind, record.created.clone()))
    }

    pub async fn reconcile(&mut self, key: &str, plan: ReconcilePlan) -> Result<(), CommandError> {
        self.reconcile_guarded(key, plan, vec![]).await
    }

    /// SPEC-0039: reconcile with explicit expectations for rows written outside the command
    /// closure. Every expectation is also a storage guard in the same atomic commit, so a row
    /// changed after the snapshot (an Unproven adapter race) is a RollbackConflict too.
    pub async fn reconcile_guarded(
        &mut self,
        key: &str,
        plan: ReconcilePlan,
        guards: Vec<CorrectionGuard>,
    ) -> Result<(), CommandError> {
        self.reconcile_resolving(key, plan, guards)
            .await
            .map(|_| ())
    }

    pub fn set_conflict_policy(
        &mut self,
        policy: super::ConflictPolicy,
    ) -> Result<(), CommandError> {
        let _permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        policy.validate()?;
        self.conflict_policy = policy;
        Ok(())
    }

    pub async fn reconcile_resolving(
        &mut self,
        key: &str,
        plan: ReconcilePlan,
        guards: Vec<CorrectionGuard>,
    ) -> Result<super::Resolution, CommandError> {
        let permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        let before = self.resolution_snapshot(key, Some(&plan), &guards).await?;
        let schema = self.db.db.schema.clone();
        let hooks = self.merge_hooks.clone();
        let policy = self.conflict_policy.clone();
        let result = self
            .apply_transaction(super::reconcile::clone_snapshot(&before), &permit, |txn| {
                super::reconcile::reconcile_in(
                    txn, &before, &schema, key, plan, guards, &hooks, &policy,
                )
            })
            .await;
        match result {
            Err(CommandError::Storage(StorageError::SchemaConflict {
                message,
            })) if message == GUARD_CONFLICT => Err(CommandError::RollbackConflict),
            result => result,
        }
    }

    pub async fn rollback(&mut self, key: &str) -> Result<(), CommandError> {
        self.rollback_resolving(key).await.map(|_| ())
    }

    pub async fn rollback_resolving(
        &mut self,
        key: &str,
    ) -> Result<super::Resolution, CommandError> {
        self.inverse(key, false).await
    }

    pub async fn cancel(&mut self, key: &str) -> Result<(), CommandError> {
        self.inverse(key, true).await.map(|_| ())
    }

    async fn inverse(
        &mut self,
        key: &str,
        cancel: bool,
    ) -> Result<super::Resolution, CommandError> {
        let permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        let before = self.resolution_snapshot(key, None, &[]).await?;
        let schema = self.db.db.schema.clone();
        let hooks = self.merge_hooks.clone();
        let policy = self.conflict_policy.clone();
        self.apply_transaction(super::reconcile::clone_snapshot(&before), &permit, |txn| {
            super::reconcile::inverse_in(txn, &before, &schema, key, cancel, &hooks, &policy)
                .map(|planned| (planned, vec![]))
        })
        .await
    }

    pub async fn discard(&mut self, key: &str) -> Result<super::Resolution, CommandError> {
        let permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        let before = self.resolution_snapshot(key, None, &[]).await?;
        let schema = self.db.db.schema.clone();
        self.apply_transaction(super::reconcile::clone_snapshot(&before), &permit, |txn| {
            super::reconcile::discard_in(txn, &before, &schema, key)
                .map(|planned| (planned, vec![]))
        })
        .await
    }

    /// Complete synchronous planning and persistence before notification delivery may yield.
    pub(super) async fn apply_transaction(
        &mut self,
        before: super::reconcile::Snapshot,
        permit: &super::execution::ExecutionPermit,
        plan: impl FnOnce(
            &mut super::ledger_txn::LedgerTxn<'_>,
        ) -> Result<(super::reconcile::RowPlan, Vec<RowGuard>), CommandError>,
    ) -> Result<super::Resolution, CommandError> {
        self.apply_transaction_then(before, permit, plan, |_| {})
            .await
    }

    /// `retire` runs synchronously after the durable commit and before publication, whose
    /// storage reads may suspend on an adapter without ready reads: a future abandoned
    /// there must not leave a consumed pull ticket or merge session outstanding.
    pub(super) async fn apply_transaction_then(
        &mut self,
        before: super::reconcile::Snapshot,
        permit: &super::execution::ExecutionPermit,
        plan: impl FnOnce(
            &mut super::ledger_txn::LedgerTxn<'_>,
        ) -> Result<(super::reconcile::RowPlan, Vec<RowGuard>), CommandError>,
        retire: impl FnOnce(&mut Self),
    ) -> Result<super::Resolution, CommandError> {
        let version = self.db.db.state.version;
        let epoch = self.ledger_epoch;
        let (planned, changes) = self.transact(self.db.db.state.clone(), permit, |txn| {
            let (planned, expectations) = plan(txn)?;
            planned.resolution.preflight()?;
            queue::trim_in(txn);
            let (guards, mutations) = row_plan(&before, &planned.rows, expectations, version)?;
            Ok((planned, guards, mutations))
        })?;
        retire(self);
        if self.ledger_epoch != epoch {
            for (key, value) in planned.outcomes {
                self.delivery.terminal(&key, value);
            }
            self.db.publish_committed(changes).await;
        }
        if planned.conflict {
            Err(CommandError::RollbackConflict)
        } else {
            Ok(planned.resolution)
        }
    }

    /// On a validated Exclusive owner, fetch only closure/write/mapping identities
    /// and rows whose declared references a mapping can rewrite. Unproven stores
    /// retain the original full snapshot and validation order.
    async fn resolution_snapshot(
        &self,
        key: &str,
        plan: Option<&ReconcilePlan>,
        guards: &[CorrectionGuard],
    ) -> Result<super::reconcile::Snapshot, CommandError> {
        if !self.db.db.validated_exclusive {
            return self.snapshot().await;
        }
        let mut identities = BTreeMap::<String, std::collections::BTreeSet<String>>::new();
        // A malformed key is still diagnosed by the planner after ledger validation.
        if let Ok(keys) = super::reconcile::closure(&self.ledger, key) {
            for key in keys {
                let record = &self.ledger.records[&key];
                for change in &record.undo {
                    identities
                        .entry(change.table.clone())
                        .or_default()
                        .insert(change.id.clone());
                }
                for change in &record.metadata_undo {
                    identities
                        .entry(change.table.clone())
                        .or_default()
                        .insert(change.record_id.clone());
                }
            }
        }
        let mut mapped = std::collections::BTreeSet::new();
        if let Some(plan) = plan {
            for write in &plan.writes {
                let (table, id) = match write {
                    ReconcileWrite::Entity {
                        change,
                    } => (&change.table, &change.record_id),
                    ReconcileWrite::Put {
                        table,
                        id,
                        ..
                    }
                    | ReconcileWrite::Remove {
                        table,
                        id,
                    } => (table, id),
                };
                identities
                    .entry(table.clone())
                    .or_default()
                    .insert(id.clone());
            }
            if let Some(record) = self.ledger.records.get(key) {
                for (local, remote) in &plan.mappings {
                    if let Some(table) = record.created.get(local) {
                        identities
                            .entry(table.clone())
                            .or_default()
                            .extend([local.clone(), remote.clone()]);
                        if local != remote {
                            mapped.insert(local.clone());
                        }
                    }
                }
            }
        }
        for guard in guards {
            identities
                .entry(guard.table.clone())
                .or_default()
                .insert(guard.id.clone());
        }
        let mut result = BTreeMap::new();
        for (table, declaration) in &self.db.db.projection.tables {
            let mut rows = BTreeMap::new();
            if let Some(ids) = identities.get(table) {
                let query = self
                    .db
                    .query(table)
                    .where_in("id", ids.iter().cloned().map(Value::String).collect());
                for row in self.db.fetch(&query).await? {
                    rows.insert(row.id.clone(), row);
                }
            }
            if !mapped.is_empty() {
                for (field, column) in &declaration.columns {
                    if column.kind == ColumnType::Reference {
                        let query = self.db.query(table).where_in(
                            field,
                            mapped.iter().cloned().map(Value::Reference).collect(),
                        );
                        for row in self.db.fetch(&query).await? {
                            rows.insert(row.id.clone(), row);
                        }
                    }
                }
            }
            result.insert(table.clone(), rows);
        }
        Ok(result)
    }

    pub(super) async fn snapshot(&self) -> Result<super::reconcile::Snapshot, CommandError> {
        let mut snapshot = BTreeMap::new();
        for table in self.db.db.schema.tables.keys() {
            let rows = self.db.db.storage.scan(table).await?;
            let mut public = BTreeMap::new();
            for mut row in rows {
                validate_row(&self.db.db.projection, table, &row)?;
                if row.fields.remove(ROW_VERSION)
                    != Some(Value::Integer(i64::from(self.db.db.state.version)))
                {
                    return Err(CommandError::Protocol);
                }
                if public.insert(row.id.clone(), row).is_some() {
                    return Err(CommandError::Protocol);
                }
            }
            snapshot.insert(table.clone(), public);
        }
        Ok(snapshot)
    }

    pub fn forget(&mut self, key: &str) -> Result<(), CommandError> {
        let permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        let record = self.ledger.records.get(key).ok_or(CommandError::Protocol)?;
        let eligible = if record.state == SyncState::Cancelled {
            record.rolled_back && record.reconciled
        } else {
            !record.is_live()
        };
        if !eligible {
            return Err(CommandError::Protocol);
        }
        let live = self.ledger.live_keys();
        for dependent in self
            .ledger
            .records
            .values()
            .filter(|r| r.key != key && live.contains(&r.key))
        {
            let mut references = dependent.dependencies.clone();
            for path in &dependent.reference_paths {
                if let Some(Json::String(id)) = super::records::path_value(&dependent.args, path) {
                    references.insert(id.clone());
                }
            }
            for change in &dependent.undo {
                for row in change.before.iter().chain(change.after.iter()) {
                    for value in row.fields.values() {
                        if let Value::Reference(id) = value {
                            references.insert(id.clone());
                        }
                    }
                }
            }
            if record.created.keys().any(|id| references.contains(id)) {
                return Err(CommandError::Protocol);
            }
        }
        // Removing a producer is never allowed to strand retained references.
        self.transact(self.db.db.state.clone(), &permit, |next| {
            next.remove_record(key);
            queue::trim_in(next);
            next.validate().map_err(|_| CommandError::Protocol)?;
            Ok(((), vec![], vec![]))
        })?;
        Ok(())
    }

    pub fn observe_progress(&mut self) -> Result<SyncSubscription, CommandError> {
        let value = self.sync_progress();
        let subscription = token()?;
        self.delivery
            .watchers
            .insert(subscription, Watch::Progress(value.clone()));
        self.delivery.pending.push(CommandEvent::Progress {
            subscription,
            value,
        });
        Ok(subscription)
    }

    pub fn queue_status(&self) -> QueueStatus {
        queue::status(&self.ledger)
    }

    pub fn observe_queue_status(&mut self) -> Result<SyncSubscription, CommandError> {
        let value = self.queue_status();
        let subscription = token()?;
        self.delivery
            .watchers
            .insert(subscription, Watch::Status(value.clone()));
        self.delivery.pending.push(CommandEvent::Status {
            subscription,
            value,
        });
        Ok(subscription)
    }

    pub fn observe_outcome(&mut self, key: &str) -> Result<SyncSubscription, CommandError> {
        let value = self
            .ledger
            .records
            .get(key)
            .and_then(queue::outcome)
            .ok_or(CommandError::Protocol)?;
        let subscription = token()?;
        self.delivery.watchers.insert(
            subscription,
            Watch::Outcome {
                key: key.into(),
                value: value.clone(),
            },
        );
        self.delivery.pending.push(CommandEvent::Outcome {
            subscription,
            value,
        });
        Ok(subscription)
    }

    pub fn unsubscribe_sync(&mut self, subscription: SyncSubscription) -> bool {
        let removed = self.delivery.watchers.remove(&subscription).is_some();
        self.delivery.pending.retain(|e| match e {
            CommandEvent::Data(_) => true,
            CommandEvent::Status {
                subscription: s,
                ..
            }
            | CommandEvent::Progress {
                subscription: s,
                ..
            }
            | CommandEvent::Outcome {
                subscription: s,
                ..
            } => *s != subscription,
        });
        removed
    }

    pub fn drain_events(&mut self, now_ms: u64) -> Vec<CommandEvent> {
        let mut events = self
            .db
            .drain_notifications(now_ms)
            .into_iter()
            .map(CommandEvent::Data)
            .collect::<Vec<_>>();
        events.append(&mut self.delivery.pending);
        events
    }
}

#[cfg(test)]
mod retention_tests {
    use super::*;
    use crate::command::records::{SyncHeader, SyncRecord};
    use crate::model::{Database, SchemaDefinition, TableDefinition};
    use std::{
        collections::BTreeSet,
        future::Future,
        task::{Context, Poll, Waker},
    };

    fn run<F: Future>(future: F) -> F::Output {
        match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("snapshot operation unexpectedly suspended"),
        }
    }

    #[test]
    fn c19_forget_preserves_producer_needed_by_live_undo() {
        let _fixture = crate::command::TEST_OWNERS.lock().unwrap();
        let schema = SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([
                ("parents".into(), TableDefinition::default()),
                (
                    "children".into(),
                    TableDefinition {
                        columns: BTreeMap::from([(
                            "parent".into(),
                            Column {
                                kind: ColumnType::Reference,
                                optional: false,
                            },
                        )]),
                        relations: BTreeMap::new(),
                    },
                ),
            ]),
        };
        let mut storage = run(Database::open(
            SnapshotStorage::default(),
            schema.clone(),
            vec![],
        ))
        .unwrap()
        .into_storage();
        let model = run(storage.catalog()).unwrap().state.unwrap();
        let namespace = "2a".repeat(16);
        let producer_key = format!("0000000000000064-{namespace}-0000000000000001");
        let consumer_key = format!("0000000000000064-{namespace}-0000000000000002");
        let context = Json::Object(BTreeMap::from([
            ("identity".into(), Json::Null),
            (
                "capturedAt".into(),
                Json::String("1970-01-01T00:00:00.100Z".into()),
            ),
        ]));
        let child = Row {
            id: "local:b".into(),
            fields: BTreeMap::from([("parent".into(), Value::Reference("local:a".into()))]),
        };
        let mut producer = SyncRecord {
            entity: None,
            metadata_undo: vec![],
            key: producer_key.clone(),
            command: "parent-v1".into(),
            args: Json::Object(BTreeMap::new()),
            context: context.clone(),
            schema_version: 1,
            order: 1,
            reference_paths: vec![],
            dependencies: BTreeSet::new(),
            created: BTreeMap::from([("local:a".into(), "parents".into())]),
            mappings: BTreeMap::from([("local:a".into(), "server:a".into())]),
            undo: vec![],
            state: SyncState::Applied,
            frozen: None,
            attempts: 1,
            due_ms: 1100,
            outcome: Some(RemoteOutcome::Applied(Json::Null)),
            rolled_back: false,
            reconciled: true,
        };
        producer.frozen = Some(producer.envelope().canonical().unwrap());
        let consumer = SyncRecord {
            entity: None,
            metadata_undo: vec![],
            key: consumer_key.clone(),
            command: "child-v1".into(),
            args: Json::Object(BTreeMap::new()),
            context,
            schema_version: 1,
            order: 2,
            reference_paths: vec![],
            dependencies: BTreeSet::new(),
            created: BTreeMap::from([("local:b".into(), "children".into())]),
            mappings: BTreeMap::new(),
            undo: vec![CommittedChange {
                table: "children".into(),
                id: child.id.clone(),
                before: None,
                after: Some(child.clone()),
            }],
            state: SyncState::Queued,
            frozen: None,
            attempts: 0,
            due_ms: 0,
            outcome: None,
            rolled_back: false,
            reconciled: false,
        };
        let ledger = SyncLedger {
            entity: None,
            header: SyncHeader {
                namespace,
                sequence: 3,
                last_ms: 100,
                options: SyncOptions::default(),
            },
            records: BTreeMap::from([(producer_key.clone(), producer), (consumer_key, consumer)]),
        };
        assert!(ledger.live_keys().contains(&producer_key));
        assert!(!ledger.records[&producer_key].is_live());
        let mut parent = Row {
            id: "server:a".into(),
            fields: BTreeMap::new(),
        };
        let mut raw_child = child;
        for row in [&mut parent, &mut raw_child] {
            row.fields.insert(ROW_VERSION.into(), Value::Integer(1));
        }
        storage
            .apply_sync_plan(SyncPlan {
                transition: SyncTransition::Normal,
                expected_model: model.clone(),
                next_model: model.clone(),
                expected_sync: None,
                next_sync: ledger.to_rows().unwrap(),
                guards: vec![
                    RowGuard {
                        table: "parents".into(),
                        id: parent.id.clone(),
                        expected: None,
                    },
                    RowGuard {
                        table: "children".into(),
                        id: raw_child.id.clone(),
                        expected: None,
                    },
                ],
                mutations: vec![
                    Mutation::Insert {
                        table: "parents".into(),
                        row: parent,
                    },
                    Mutation::Insert {
                        table: "children".into(),
                        row: raw_child,
                    },
                ],
            })
            .unwrap();
        let mut db = run(CommandDatabase::open(
            storage,
            schema,
            vec![],
            [0; 16],
            SyncOptions::default(),
        ))
        .unwrap();
        let before = db.queue_status();
        let before_rows = db.db.db.storage.load_sync().unwrap();
        assert_eq!(before.live, 2);

        assert_eq!(db.forget(&producer_key), Err(CommandError::Protocol));

        assert_eq!(db.queue_status(), before);
        assert_eq!(db.db.db.storage.load_sync().unwrap(), before_rows);
        assert_eq!(db.db.db.state, model);
        assert_eq!(run(db.query("parents").fetch(&db)).unwrap().len(), 1);
        assert_eq!(run(db.query("children").fetch(&db)).unwrap().len(), 1);
    }
}

/// SPEC-0039: the planner's derived guards cover exactly the rows it mutates; the only surplus
/// guards are the caller's explicit expectations, merged once per identity. Anything else is
/// an internal inconsistency (Protocol), never a weaker commit.
fn row_plan(
    before: &super::reconcile::Snapshot,
    after: &super::reconcile::Snapshot,
    expectations: Vec<RowGuard>,
    version: u32,
) -> Result<(Vec<RowGuard>, Vec<Mutation>), CommandError> {
    let mut guards = vec![];
    let mut mutations = vec![];
    let raw = |mut row: Row| {
        row.fields
            .insert(ROW_VERSION.into(), Value::Integer(i64::from(version)));
        row
    };
    for (table, old) in before {
        let new = after.get(table).ok_or(CommandError::Protocol)?;
        let ids = old
            .keys()
            .chain(new.keys())
            .collect::<std::collections::BTreeSet<_>>();
        for id in ids {
            let prior = old.get(id);
            let after = new.get(id);
            if prior == after {
                continue;
            }
            crate::storage::count_work!(
                rows_cloned,
                u64::from(prior.is_some()) + u64::from(after.is_some())
            );
            guards.push(RowGuard {
                table: table.clone(),
                id: id.clone(),
                expected: prior.cloned().map(&raw),
            });
            mutations.push(match after {
                Some(row) if prior.is_some() => Mutation::Replace {
                    table: table.clone(),
                    row: raw(row.clone()),
                },
                Some(row) => Mutation::Insert {
                    table: table.clone(),
                    row: raw(row.clone()),
                },
                None => Mutation::Remove {
                    table: table.clone(),
                    id: id.clone(),
                },
            });
        }
    }
    if after.len() != before.len() {
        return Err(CommandError::Protocol);
    }
    let expectations = expectations
        .into_iter()
        .map(|guard| RowGuard {
            expected: guard.expected.map(&raw),
            ..guard
        })
        .collect();
    Ok((commit_guards(guards, &mutations, expectations)?, mutations))
}

fn commit_guards(
    derived: Vec<RowGuard>,
    mutations: &[Mutation],
    explicit: Vec<RowGuard>,
) -> Result<Vec<RowGuard>, CommandError> {
    let identity = |guard: &RowGuard| (guard.table.clone(), guard.id.clone());
    let mutated = mutations
        .iter()
        .map(|mutation| match mutation {
            Mutation::Insert {
                table,
                row,
            }
            | Mutation::Replace {
                table,
                row,
            } => (table.clone(), row.id.clone()),
            Mutation::Remove {
                table,
                id,
            } => (table.clone(), id.clone()),
        })
        .collect::<std::collections::BTreeSet<_>>();
    let mut merged = BTreeMap::new();
    for guard in derived {
        if merged.insert(identity(&guard), guard).is_some() {
            return Err(CommandError::Protocol);
        }
    }
    if !merged.keys().cloned().eq(mutated) {
        return Err(CommandError::Protocol);
    }
    for guard in explicit {
        match merged.get(&identity(&guard)) {
            Some(derived) if derived.expected != guard.expected => {
                return Err(CommandError::Protocol);
            }
            Some(_) => {}
            None => {
                merged.insert(identity(&guard), guard);
            }
        }
    }
    Ok(merged.into_values().collect())
}

#[cfg(test)]
mod guard_merge_tests {
    use super::*;

    fn guard(id: &str, value: Option<i64>) -> RowGuard {
        RowGuard {
            table: "items".into(),
            id: id.into(),
            expected: value.map(|value| Row {
                id: id.into(),
                fields: BTreeMap::from([("value".into(), Value::Integer(value))]),
            }),
        }
    }

    fn replace(id: &str) -> Mutation {
        Mutation::Replace {
            table: "items".into(),
            row: Row {
                id: id.into(),
                fields: BTreeMap::new(),
            },
        }
    }

    #[test]
    fn gw14_explicit_expectations_are_the_only_surplus_guards() {
        let merged = commit_guards(
            vec![guard("a", Some(1))],
            &[replace("a")],
            vec![guard("s", None)],
        )
        .unwrap();
        assert_eq!(merged, vec![guard("a", Some(1)), guard("s", None)]);
        // An identical explicit expectation for a mutated row is merged once.
        let merged = commit_guards(
            vec![guard("a", Some(1))],
            &[replace("a")],
            vec![guard("a", Some(1))],
        )
        .unwrap();
        assert_eq!(merged, vec![guard("a", Some(1))]);
    }

    #[test]
    fn gw14_a_derived_guard_without_its_mutation_is_protocol() {
        for (derived, mutations, explicit) in [
            (vec![guard("a", Some(1))], vec![], vec![]),
            (vec![], vec![replace("a")], vec![]),
            (
                vec![guard("a", Some(1)), guard("a", Some(1))],
                vec![replace("a")],
                vec![],
            ),
            (
                vec![guard("a", Some(1))],
                vec![replace("a")],
                vec![guard("a", Some(2))],
            ),
        ] {
            assert!(matches!(
                commit_guards(derived, &mutations, explicit),
                Err(CommandError::Protocol)
            ));
        }
    }
}
