//! API-0011: passive owned custom-merge offers over the core pull planner.
use super::*;
use crate::{
    model::{ROW_VERSION, SchemaDefinition},
    storage::{ModelState, Row, SyncStorage, Value, validate_row},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MergeSession(pub(super) u64);

#[derive(Clone, PartialEq, Eq)]
pub enum MergeStep {
    Offer {
        session: MergeSession,
        step: u64,
        table: String,
        record_id: String,
    },
    Ready {
        session: MergeSession,
        step: u64,
    },
}

#[derive(Clone, PartialEq, Eq)]
pub struct OwnedMergeInput {
    pub current: Option<Row>,
    pub metadata: RowMetadata,
    pub incoming: EntityChange,
    pub default: EntityResolution,
}

impl std::fmt::Debug for MergeStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MergeStep { .. }")
    }
}
impl std::fmt::Debug for OwnedMergeInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OwnedMergeInput { .. }")
    }
}

static NEXT_MERGE: AtomicU64 = AtomicU64::new(1);

pub(super) struct MergeState {
    id: MergeSession,
    request: PullRequest,
    model: ModelState,
    schema: SchemaDefinition,
    expected: MergeExpected,
    planner: super::pull::PullPlanner,
    /// SPEC-0037 R6.6: under a validated Exclusive owner, the page's touched
    /// rows, re-read exactly before commit. `None`: any other owner, whose store
    /// another writer may change, compares the full snapshot.
    touched: Option<Vec<(String, String)>>,
    tables: BTreeSet<String>,
    hooks: BTreeMap<String, MergeHook>,
    pending: Option<OwnedMergeInput>,
}

enum MergeExpected {
    Exclusive {
        epoch: u64,
    },
    Unproven {
        ledger: Box<records::SyncLedger>,
        persisted: Option<BTreeMap<String, Row>>,
    },
}

impl MergeState {
    fn step(&self) -> MergeStep {
        match &self.pending {
            Some(input) => MergeStep::Offer {
                session: self.id,
                step: self.planner.index as u64,
                table: input.incoming.table.clone(),
                record_id: input.incoming.record_id.clone(),
            },
            None => MergeStep::Ready {
                session: self.id,
                step: self.planner.index as u64,
            },
        }
    }
    fn advance(&mut self, ledger: &records::SyncLedger) -> Result<MergeStep, CommandError> {
        self.pending = self
            .planner
            .advance(&self.schema, &self.hooks, &self.tables, ledger)?;
        Ok(self.step())
    }
}

impl<S: SyncStorage> CommandDatabase<S> {
    pub async fn merge_prepare(
        &mut self,
        request: PullRequest,
        page: PullPage,
        tables: BTreeSet<String>,
    ) -> Result<MergeStep, CommandError> {
        self.ensure_pull_request(request)?;
        if self.merge.is_some() {
            return Err(CommandError::Busy);
        }
        let _permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        self.validate_pull_request(request)?;
        if tables.iter().any(|table| {
            !self.db.db.schema.tables.contains_key(table) || self.merge_hooks.contains_key(table)
        }) {
            return Err(CommandError::InvalidCommand);
        }
        let touched = self.db.db.validated_exclusive.then(|| {
            page.changes
                .iter()
                .map(|change| (change.table.clone(), change.record_id.clone()))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        });
        let before = self.merge_snapshot(touched.as_deref()).await?;
        let mut state = MergeState {
            id: MergeSession(
                NEXT_MERGE
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
                    .map_err(|_| CommandError::LimitExceeded)?,
            ),
            request,
            model: self.db.db.state.clone(),
            schema: self.db.db.schema.clone(),
            expected: if self.db.db.validated_exclusive {
                MergeExpected::Exclusive {
                    epoch: self.ledger_epoch,
                }
            } else {
                crate::storage::count_work!(
                    rows_cloned,
                    self.persisted_rows.as_ref().map_or(0, BTreeMap::len) as u64
                );
                MergeExpected::Unproven {
                    ledger: Box::new(self.ledger.clone()),
                    persisted: self.persisted_rows.clone(),
                }
            },
            planner: super::pull::PullPlanner::new(&self.ledger, before, page)?,
            touched,
            tables,
            hooks: self.merge_hooks.clone(),
            pending: None,
        };
        let step = state.advance(&self.ledger)?;
        self.merge = Some(state);
        Ok(step)
    }

    pub fn merge_input(
        &self,
        session: MergeSession,
        step: u64,
    ) -> Result<OwnedMergeInput, CommandError> {
        let state = self
            .merge
            .as_ref()
            .filter(|state| state.id == session && state.planner.index as u64 == step)
            .ok_or(CommandError::InvalidTicket)?;
        state.pending.clone().ok_or(CommandError::InvalidTicket)
    }

    async fn validate_merge_state(&mut self, session: MergeSession) -> Result<(), CommandError> {
        let state = self
            .merge
            .as_ref()
            .filter(|state| state.id == session)
            .ok_or(CommandError::InvalidTicket)?;
        let changed = self.db.db.state != state.model
            || self.db.db.schema != state.schema
            || match &state.expected {
                MergeExpected::Exclusive {
                    epoch,
                } => self.ledger_epoch != *epoch,
                MergeExpected::Unproven {
                    ledger,
                    persisted,
                } => self.ledger != **ledger || self.persisted_rows != *persisted,
            };
        if changed {
            self.retire_merge();
            return Err(CommandError::StalePull);
        }
        let (before, touched) = (
            super::reconcile::clone_snapshot(&state.planner.before),
            state.touched.clone(),
        );
        if self.merge_snapshot(touched.as_deref()).await? != before {
            self.retire_merge();
            return Err(CommandError::StalePull);
        }
        Ok(())
    }

    /// The full snapshot, or under a validated Exclusive owner only the touched
    /// rows: the owner epoch and model, compared separately, then cover every
    /// other committed change, since no other writer exists.
    async fn merge_snapshot(
        &self,
        touched: Option<&[(String, String)]>,
    ) -> Result<super::reconcile::Snapshot, CommandError> {
        match touched {
            Some(touched) => self.touched_snapshot(touched).await,
            None => self.snapshot().await,
        }
    }

    /// SPEC-0037 R6.6: every declared table, holding only the given rows as
    /// validated public images (absent rows stay absent).
    pub(super) async fn touched_snapshot(
        &self,
        touched: &[(String, String)],
    ) -> Result<super::reconcile::Snapshot, CommandError> {
        let mut snapshot: super::reconcile::Snapshot = self
            .db
            .db
            .schema
            .tables
            .keys()
            .map(|table| (table.clone(), BTreeMap::new()))
            .collect();
        for (table, id) in touched {
            let Some(rows) = snapshot.get_mut(table) else {
                continue;
            };
            if let Some(mut row) = self.db.db.storage.get(table, id).await? {
                // Count the owned row image acquired for the planner snapshot.
                crate::storage::count_work!(rows_cloned, 1);
                validate_row(&self.db.db.projection, table, &row)?;
                if row.fields.remove(ROW_VERSION)
                    != Some(Value::Integer(i64::from(self.db.db.state.version)))
                {
                    return Err(CommandError::Protocol);
                }
                rows.insert(row.id.clone(), row);
            }
        }
        Ok(snapshot)
    }

    fn retire_merge(&mut self) {
        if let Some(state) = self.merge.take() {
            // Only this session's pull is retired; an unrelated token cannot cancel work.
            if self.ensure_pull_request(state.request).is_ok() {
                self.pull = None;
            }
        }
    }

    pub async fn merge_reply(
        &mut self,
        session: MergeSession,
        step: u64,
        row: Option<Row>,
    ) -> Result<MergeStep, CommandError> {
        let input = self.merge_input(session, step)?;
        // This internal copy is retained for candidate validation; the public
        // merge_input result boundary itself is outside staging accounting.
        crate::storage::count_work!(
            rows_cloned,
            u64::from(input.current.is_some()) + u64::from(input.default.row.is_some())
        );
        let _permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        self.validate_merge_state(session).await?;
        let result = (|| {
            let state = self.merge.as_mut().ok_or(CommandError::InvalidTicket)?;
            let custom = entity::validate_custom_resolution(
                &MergeInput {
                    schema: &state.schema,
                    current: input.current.as_ref(),
                    metadata: &input.metadata,
                    incoming: &input.incoming,
                    default: &input.default,
                },
                EntityResolution {
                    row,
                    metadata: input.default.metadata.clone(),
                },
            )?;
            state.planner.accept(custom)?;
            state.advance(&self.ledger)
        })();
        if result.is_err() {
            self.retire_merge();
        }
        result
    }

    pub async fn merge_commit(&mut self, session: MergeSession) -> Result<(), CommandError> {
        let state = self
            .merge
            .as_ref()
            .filter(|state| state.id == session)
            .ok_or(CommandError::InvalidTicket)?;
        if state.pending.is_some() {
            return Err(CommandError::InvalidTicket);
        }
        let permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        self.validate_merge_state(session).await?;
        let state = self.merge.as_ref().ok_or(CommandError::InvalidTicket)?;
        let planned = state.planner.finish()?;
        let before = super::reconcile::clone_snapshot(&state.planner.before);
        self.apply_transaction_then(
            before,
            &permit,
            |txn| Ok((planned.stage(txn)?, vec![])),
            Self::retire_merge,
        )
        .await?;
        Ok(())
    }

    pub fn merge_abort(&mut self, session: MergeSession) -> Result<(), CommandError> {
        if self.merge.as_ref().is_none_or(|state| state.id != session) {
            return Err(CommandError::InvalidTicket);
        }
        self.retire_merge();
        Ok(())
    }
}

#[cfg(test)]
mod snapshot_counter_tests {
    use super::*;
    use crate::{
        model::TableDefinition,
        storage::{SnapshotStorage, counters},
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };

    fn run<F: Future>(future: F) -> F::Output {
        match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("memory fixture suspended"),
        }
    }

    #[test]
    fn si_c04_touched_snapshot_counts_only_returned_row_images() {
        let _serial = TEST_OWNERS
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut db = run(CommandDatabase::open(
            SnapshotStorage::default(),
            SchemaDefinition {
                version: 1,
                tables: BTreeMap::from([("items".into(), TableDefinition::default())]),
            },
            vec![],
            [0x3c; 16],
            SyncOptions::default(),
        ))
        .unwrap();
        db.configure_entities(
            EntityConfig {
                scope: "counter".into(),
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
                checkpoint: "seed".into(),
                complete: true,
                changes: vec![EntityChange {
                    table: "items".into(),
                    record_id: "r".into(),
                    op: EntityOperation::Create,
                    changes: BTreeMap::new(),
                    removed_fields: BTreeSet::new(),
                    timestamp_ms: 1,
                    field_timestamps: BTreeMap::new(),
                }],
            },
        ))
        .unwrap();
        counters::reset();
        let rows = run(db.touched_snapshot(&[
            ("items".into(), "r".into()),
            ("items".into(), "absent".into()),
        ]))
        .unwrap();
        assert_eq!(rows["items"].len(), 1);
        assert_eq!(
            counters::snapshot().rows_cloned,
            1,
            "exactly one owned staging row was returned"
        );
        counters::reset();
        assert!(
            run(db.touched_snapshot(&[("items".into(), "absent".into())])).unwrap()["items"]
                .is_empty()
        );
        assert_eq!(counters::snapshot().rows_cloned, 0);
    }
}
