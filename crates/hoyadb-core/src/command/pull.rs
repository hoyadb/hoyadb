//! API-0009: atomic pull/bootstrap and explicit generation reset.
use super::{entity::*, *};
use crate::storage::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_PULL: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PullRequest(u64);

#[derive(Clone, PartialEq, Eq)]
pub struct PullBatch {
    pub request: PullRequest,
    pub since: Option<String>,
    pub scope: String,
    pub generation: i64,
}

#[derive(Clone, PartialEq, Eq)]
pub struct PullPage {
    pub checkpoint: String,
    pub complete: bool,
    pub changes: Vec<EntityChange>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetMode {
    PreserveRecovery,
    DiscardRecovery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgressPhase {
    NotStarted,
    Bootstrapping,
    Ready,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncProgress {
    pub phase: ProgressPhase,
    pub ready: bool,
    pub pages: i64,
    pub changes: i64,
    pub tombstones: usize,
}

pub(super) struct PullTicketState {
    pub(super) request: PullRequest,
    pub(super) checkpoint: Option<String>,
    pub(super) generation: i64,
    pub(super) revision: i64,
}

impl PullPage {
    pub fn to_json(&self) -> Result<Json, CommandError> {
        if self.changes.len() > 1000 {
            return Err(CommandError::LimitExceeded);
        }
        let changes = self
            .changes
            .iter()
            .map(|c| {
                object([
                    ("table", text(&c.table)),
                    ("record_id", text(&c.record_id)),
                    ("op", text(entity::op_name(c.op))),
                    (
                        "changes",
                        Json::Object(
                            c.changes
                                .iter()
                                .map(|(k, v)| (k.clone(), cell_json(v)))
                                .collect(),
                        ),
                    ),
                    (
                        "removed_fields",
                        Json::Array(c.removed_fields.iter().map(|s| text(s)).collect()),
                    ),
                    ("timestamp_ms", text(&c.timestamp_ms.to_string())),
                    (
                        "field_timestamps",
                        Json::Object(
                            c.field_timestamps
                                .iter()
                                .map(|(k, v)| (k.clone(), text(&v.to_string())))
                                .collect(),
                        ),
                    ),
                ])
            })
            .collect();
        let json = object([
            ("checkpoint", text(&self.checkpoint)),
            ("complete", Json::Bool(self.complete)),
            ("changes", Json::Array(changes)),
        ]);
        json.canonical()?;
        Ok(json)
    }

    pub fn from_json(value: &Json) -> Result<Self, CommandError> {
        value.canonical()?;
        let m = exact(value, &["checkpoint", "complete", "changes"])?;
        let Json::Bool(complete) = m["complete"] else {
            return Err(CommandError::InvalidCommand);
        };
        let Json::Array(changes) = &m["changes"] else {
            return Err(CommandError::InvalidCommand);
        };
        if changes.len() > 1000 {
            return Err(CommandError::LimitExceeded);
        }
        let changes = changes
            .iter()
            .map(|c| {
                let c = exact(
                    c,
                    &[
                        "table",
                        "record_id",
                        "op",
                        "changes",
                        "removed_fields",
                        "timestamp_ms",
                        "field_timestamps",
                    ],
                )?;
                let op = match as_text(&c["op"])? {
                    "create" => EntityOperation::Create,
                    "update" => EntityOperation::Update,
                    "delete" => EntityOperation::Delete,
                    _ => return Err(CommandError::InvalidCommand),
                };
                let Json::Object(values) = &c["changes"] else {
                    return Err(CommandError::InvalidCommand);
                };
                let Json::Object(times) = &c["field_timestamps"] else {
                    return Err(CommandError::InvalidCommand);
                };
                let Json::Array(removed) = &c["removed_fields"] else {
                    return Err(CommandError::InvalidCommand);
                };
                let mut removed_fields = BTreeSet::new();
                for value in removed {
                    if !removed_fields.insert(as_text(value)?.to_owned()) {
                        return Err(CommandError::InvalidCommand);
                    }
                }
                Ok(EntityChange {
                    table: as_text(&c["table"])?.into(),
                    record_id: as_text(&c["record_id"])?.into(),
                    op,
                    changes: values
                        .iter()
                        .map(|(k, v)| Ok((k.clone(), cell(v)?)))
                        .collect::<Result<_, CommandError>>()?,
                    removed_fields,
                    timestamp_ms: decimal(&c["timestamp_ms"])?,
                    field_timestamps: times
                        .iter()
                        .map(|(k, v)| Ok((k.clone(), decimal(v)?)))
                        .collect::<Result<_, CommandError>>()?,
                })
            })
            .collect::<Result<_, CommandError>>()?;
        Ok(Self {
            checkpoint: as_text(&m["checkpoint"])?.into(),
            complete,
            changes,
        })
    }
}

fn object<const N: usize>(entries: [(&str, Json); N]) -> Json {
    Json::Object(entries.into_iter().map(|(k, v)| (k.into(), v)).collect())
}

fn text(s: &str) -> Json {
    Json::String(s.into())
}

fn exact<'a>(value: &'a Json, keys: &[&str]) -> Result<&'a BTreeMap<String, Json>, CommandError> {
    let Json::Object(m) = value else {
        return Err(CommandError::InvalidCommand);
    };
    if m.len() != keys.len() || keys.iter().any(|k| !m.contains_key(*k)) {
        return Err(CommandError::InvalidCommand);
    }
    Ok(m)
}

fn as_text(value: &Json) -> Result<&str, CommandError> {
    if let Json::String(s) = value {
        Ok(s)
    } else {
        Err(CommandError::InvalidCommand)
    }
}

fn decimal(value: &Json) -> Result<i64, CommandError> {
    let text = as_text(value)?;
    let number = text
        .parse::<i64>()
        .map_err(|_| CommandError::InvalidCommand)?;
    if number.to_string() != text {
        return Err(CommandError::InvalidCommand);
    }
    Ok(number)
}

fn cell_json(value: &Value) -> Json {
    let (kind, payload) = match value {
        Value::Null => return object([("kind", text("null"))]),
        Value::String(v) => ("string", text(v)),
        Value::Reference(v) => ("reference", text(v)),
        Value::Integer(v) => ("integer", text(&v.to_string())),
        Value::Timestamp(v) => ("timestamp", text(&v.to_string())),
        Value::Boolean(v) => ("boolean", Json::Bool(*v)),
        Value::Float(v) => ("float", text(&v.get().to_string())),
    };
    object([("kind", text(kind)), ("value", payload)])
}

fn cell(value: &Json) -> Result<Value, CommandError> {
    let Json::Object(m) = value else {
        return Err(CommandError::InvalidCommand);
    };
    let kind = as_text(m.get("kind").ok_or(CommandError::InvalidCommand)?)?;
    if kind == "null" {
        exact(value, &["kind"])?;
        return Ok(Value::Null);
    }
    let m = exact(value, &["kind", "value"])?;
    let value = &m["value"];
    Ok(match kind {
        "string" => Value::String(as_text(value)?.into()),
        "reference" => Value::Reference(as_text(value)?.into()),
        "integer" => Value::Integer(decimal(value)?),
        "timestamp" => Value::Timestamp(decimal(value)?),
        "boolean" => {
            let Json::Bool(v) = value else {
                return Err(CommandError::InvalidCommand);
            };
            Value::Boolean(*v)
        }
        "float" => {
            let s = as_text(value)?;
            let number = if s.contains(['.', 'e', 'E']) {
                s.to_owned()
            } else {
                format!("{s}.0")
            };
            let Json::Float(v) = Json::parse(&number)? else {
                return Err(CommandError::InvalidCommand);
            };
            Value::Float(v)
        }
        _ => return Err(CommandError::InvalidCommand),
    })
}
impl<S: SyncStorage> CommandDatabase<S> {
    pub fn begin_pull(&mut self) -> Result<PullBatch, CommandError> {
        let _permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        if self.pull.is_some() {
            return Err(CommandError::Busy);
        }
        let e = self
            .ledger
            .entity
            .as_ref()
            .ok_or(CommandError::InvalidCommand)?;
        let request = PullRequest(
            NEXT_PULL
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
                .map_err(|_| CommandError::LimitExceeded)?,
        );
        let batch = PullBatch {
            request,
            since: e.checkpoint.clone(),
            scope: e.config.scope.clone(),
            generation: e.generation,
        };
        self.pull = Some(PullTicketState {
            request,
            checkpoint: e.checkpoint.clone(),
            generation: e.generation,
            revision: e.revision,
        });
        Ok(batch)
    }

    pub fn abort_pull(&mut self, request: PullRequest) -> Result<(), CommandError> {
        if self.pull.as_ref().is_none_or(|t| t.request != request) {
            return Err(CommandError::InvalidTicket);
        }
        self.pull = None;
        self.merge = None;
        Ok(())
    }

    pub(super) fn ensure_pull_request(&self, request: PullRequest) -> Result<(), CommandError> {
        if self
            .pull
            .as_ref()
            .is_none_or(|ticket| ticket.request != request)
        {
            Err(CommandError::InvalidTicket)
        } else {
            Ok(())
        }
    }

    pub(super) fn validate_pull_request(
        &mut self,
        request: PullRequest,
    ) -> Result<(), CommandError> {
        self.ensure_pull_request(request)?;
        let ticket = self.pull.as_ref().expect("validated pull request");
        let e = self
            .ledger
            .entity
            .as_ref()
            .ok_or(CommandError::InvalidCommand)?;
        if ticket.generation != e.generation
            || ticket.revision != e.revision
            || ticket.checkpoint != e.checkpoint
        {
            self.pull = None;
            self.merge = None;
            return Err(CommandError::StalePull);
        }
        Ok(())
    }

    pub async fn apply_pull(
        &mut self,
        request: PullRequest,
        page: PullPage,
    ) -> Result<(), CommandError> {
        self.ensure_pull_request(request)?;
        let permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        self.validate_pull_request(request)?;
        if self.merge.is_some() {
            return Err(CommandError::Busy);
        }
        let before = if self.db.db.validated_exclusive {
            let touched = page
                .changes
                .iter()
                .map(|change| (change.table.clone(), change.record_id.clone()))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            self.touched_snapshot(&touched).await?
        } else {
            self.snapshot().await?
        };
        let mut planner = PullPlanner::new(&self.ledger, before, page)?;
        planner.advance(
            &self.db.db.schema,
            &self.merge_hooks,
            &BTreeSet::new(),
            &self.ledger,
        )?;
        let planned = planner.finish()?;
        self.apply_transaction_then(
            planner.before,
            &permit,
            |txn| Ok((planned.stage(txn)?, vec![])),
            |owner| owner.pull = None,
        )
        .await?;
        Ok(())
    }

    pub async fn reset(&mut self, mode: ResetMode) -> Result<(), CommandError> {
        let permit = self
            .execution_owner
            .idle_permit()
            .map_err(|_| CommandError::Busy)?;
        let discard_recovery = mode == ResetMode::DiscardRecovery;
        if !discard_recovery && self.ledger.has_live_recovery() {
            return Err(CommandError::Protocol);
        }
        if self.persisted_rows.is_none() {
            return Err(CommandError::InvalidCommand);
        }
        let mut next = self.ledger.clone();
        let entity = next.entity.as_mut().ok_or(CommandError::InvalidCommand)?;
        entity.generation = entity
            .generation
            .checked_add(1)
            .ok_or(CommandError::LimitExceeded)?;
        entity.revision = entity
            .revision
            .checked_add(1)
            .ok_or(CommandError::LimitExceeded)?;
        entity.checkpoint = None;
        entity.ready = false;
        entity.pages = 0;
        entity.changes = 0;
        entity.metadata.clear();
        next.records.clear();
        let before = self.snapshot().await?;
        let mut guards = vec![];
        let mut mutations = vec![];
        for (table, rows) in before {
            for (id, mut row) in rows {
                row.fields.insert(
                    crate::model::ROW_VERSION.into(),
                    Value::Integer(i64::from(self.db.db.state.version)),
                );
                guards.push(RowGuard {
                    table: table.clone(),
                    id: id.clone(),
                    expected: Some(row),
                });
                mutations.push(Mutation::Remove {
                    table: table.clone(),
                    id,
                });
            }
        }
        let mut next_aliases = self.aliases.clone();
        next_aliases.clear();
        let next_rows = super::durable::stored_rows(&next, &next_aliases)?;
        let next_epoch = self
            .ledger_epoch
            .checked_add(1)
            .ok_or(CommandError::LimitExceeded)?;
        permit.validate().map_err(|_| CommandError::InvalidTicket)?;
        self.db.db.storage.apply_sync_plan(SyncPlan {
            transition: SyncTransition::Reset {
                discard_recovery,
            },
            expected_model: self.db.db.state.clone(),
            next_model: self.db.db.state.clone(),
            expected_sync: self
                .persisted_rows
                .as_ref()
                .map(super::owner::clone_packet_inventory),
            next_sync: super::owner::clone_sync_rows(&next_rows),
            guards,
            mutations,
        })?;
        self.ledger = next;
        self.aliases = next_aliases;
        self.persisted_rows = Some(
            next_rows
                .into_iter()
                .map(|row| (row.id.clone(), row))
                .collect(),
        );
        self.ledger_epoch = next_epoch;
        self.tombstones = 0;
        self.metadata_ordinals.clear();
        self.pull = None;
        self.merge = None;
        self.delivery = super::sync::Delivery::default();
        self.db.reset_observations();
        Ok(())
    }

    pub fn sync_progress(&self) -> SyncProgress {
        progress(&self.ledger, self.tombstones)
    }
}

/// Shared ordinary/passive sequential pull planning; no storage or foreign callbacks.
pub(super) struct PullPlanner {
    pub(super) before: super::reconcile::Snapshot,
    pub(super) rows: super::reconcile::Snapshot,
    metadata: BTreeMap<(String, String), RowMetadata>,
    changed_metadata: BTreeSet<(String, String)>,
    page: PullPage,
    pub(super) index: usize,
}

impl PullPlanner {
    pub(super) fn new(
        ledger: &records::SyncLedger,
        before: super::reconcile::Snapshot,
        page: PullPage,
    ) -> Result<Self, CommandError> {
        page.to_json()?;
        let e = ledger.entity.as_ref().ok_or(CommandError::InvalidCommand)?;
        if !page.complete && e.checkpoint.as_ref() == Some(&page.checkpoint) {
            return Err(CommandError::InvalidCommand);
        }
        let mut metadata = BTreeMap::new();
        for incoming in &page.changes {
            let key = (incoming.table.clone(), incoming.record_id.clone());
            metadata.entry(key.clone()).or_insert_with(|| {
                crate::storage::count_work!(metadata_entries_visited, 1);
                e.metadata.get(&key).cloned().unwrap_or_default()
            });
        }
        Ok(Self {
            rows: super::reconcile::clone_snapshot(&before),
            before,
            metadata,
            changed_metadata: BTreeSet::new(),
            page,
            index: 0,
        })
    }

    pub(super) fn advance(
        &mut self,
        schema: &crate::model::SchemaDefinition,
        hooks: &BTreeMap<String, MergeHook>,
        foreign: &BTreeSet<String>,
        expected: &records::SyncLedger,
    ) -> Result<Option<OwnedMergeInput>, CommandError> {
        while let Some(incoming) = self.page.changes.get(self.index) {
            let table = self
                .rows
                .get(&incoming.table)
                .ok_or(CommandError::InvalidCommand)?;
            let key = (incoming.table.clone(), incoming.record_id.clone());
            let current = table.get(&incoming.record_id);
            crate::storage::count_work!(metadata_entries_visited, 1);
            let metadata = self.metadata.get(&key).cloned().unwrap_or_default();
            let host = foreign.contains(&incoming.table) && metadata.tombstone.is_none();
            let resolution = resolve_entity(
                schema,
                current,
                &metadata,
                incoming,
                true,
                if host {
                    None
                } else {
                    hooks.get(&incoming.table).copied()
                },
            )?;
            let changed = resolution.row.as_ref() != current || resolution.metadata != metadata;
            let permanent_noop = !changed
                && metadata
                    .tombstone
                    .as_ref()
                    .is_some_and(|stamp| stamp.authoritative);
            // API-0009 section 4: an absent row gaining a tombstone occupies its identity for
            // a later mapping exactly as an inserted row does (reconcile refuses an occupied
            // comparison target), so both count as insertion for unmapped producers.
            let occupying = current.is_none()
                && (resolution.row.is_some()
                    || (metadata.tombstone.is_none() && resolution.metadata.tombstone.is_some()));
            if !permanent_noop
                && protected(expected, &incoming.table, &incoming.record_id, occupying)
            {
                return Err(CommandError::PullConflict);
            }
            if host {
                crate::storage::count_work!(rows_cloned, u64::from(current.is_some()));
                return Ok(Some(OwnedMergeInput {
                    current: current.cloned(),
                    metadata,
                    incoming: incoming.clone(),
                    default: resolution,
                }));
            }
            self.accept(resolution)?;
        }
        Ok(None)
    }

    pub(super) fn accept(&mut self, resolution: EntityResolution) -> Result<(), CommandError> {
        let incoming = self
            .page
            .changes
            .get(self.index)
            .ok_or(CommandError::InvalidTicket)?;
        let table = self
            .rows
            .get_mut(&incoming.table)
            .ok_or(CommandError::InvalidCommand)?;
        let key = (incoming.table.clone(), incoming.record_id.clone());
        crate::storage::count_work!(metadata_entries_visited, 1);
        let metadata = self.metadata.get(&key).cloned().unwrap_or_default();
        if resolution.row.as_ref() != table.get(&incoming.record_id)
            || resolution.metadata != metadata
        {
            match resolution.row {
                Some(row) => {
                    table.insert(row.id.clone(), row);
                }
                None => {
                    table.remove(&incoming.record_id);
                }
            }
            self.changed_metadata.insert(key.clone());
            self.metadata.insert(key, resolution.metadata);
        }
        self.index += 1;
        Ok(())
    }

    pub(super) fn finish(&self) -> Result<PullPlan, CommandError> {
        if self.index != self.page.changes.len() {
            return Err(CommandError::InvalidTicket);
        }
        Ok(PullPlan {
            rows: super::reconcile::clone_snapshot(&self.rows),
            metadata: self
                .changed_metadata
                .iter()
                .map(|key| (key.clone(), self.metadata[key].clone()))
                .collect(),
            checkpoint: self.page.checkpoint.clone(),
            complete: self.page.complete,
            changes: i64::try_from(self.page.changes.len())
                .map_err(|_| CommandError::LimitExceeded)?,
        })
    }
}

/// Only the page's staged images cross the passive callback/await boundary.
/// A ledger transaction is created later, synchronously at owner commit.
pub(super) struct PullPlan {
    rows: super::reconcile::Snapshot,
    metadata: BTreeMap<(String, String), RowMetadata>,
    checkpoint: String,
    complete: bool,
    changes: i64,
}

impl PullPlan {
    pub(super) fn stage(
        self,
        txn: &mut super::ledger_txn::LedgerTxn<'_>,
    ) -> Result<super::reconcile::RowPlan, CommandError> {
        txn.edit_entity_header(|header| {
            header.checkpoint = Some(self.checkpoint);
            header.ready |= self.complete;
            header.pages = header
                .pages
                .checked_add(1)
                .ok_or(CommandError::LimitExceeded)?;
            header.changes = header
                .changes
                .checked_add(self.changes)
                .ok_or(CommandError::LimitExceeded)?;
            Ok(())
        })?;
        for (key, value) in self.metadata {
            if txn
                .entity
                .as_ref()
                .and_then(|entity| entity.metadata.get(&key))
                != Some(&value)
            {
                txn.put_metadata(key, value)?;
            }
        }
        Ok(super::reconcile::RowPlan {
            resolution: super::Resolution::Completed,
            rows: self.rows,
            conflict: false,
            outcomes: vec![],
        })
    }
}

fn protected(ledger: &records::SyncLedger, table: &str, id: &str, inserting: bool) -> bool {
    for key in ledger.live_keys() {
        let record = &ledger.records[&key];
        if record.undo.iter().any(|c| c.table == table && c.id == id)
            || record
                .metadata_undo
                .iter()
                .any(|c| c.table == table && c.record_id == id)
        {
            return true;
        }
        // API-0009 section 4: a cancellation whose inverse completed has no local row and can
        // never be mapped, so its created identity cannot collide with an unrelated insertion.
        let mappable =
            !(record.state == SyncState::Cancelled && record.rolled_back && record.reconciled);
        for (created, owned_table) in &record.created {
            if owned_table == table
                && (created == id
                    || record
                        .mappings
                        .get(created)
                        .is_some_and(|mapped| mapped == id)
                    || (inserting && mappable && !record.mappings.contains_key(created)))
            {
                return true;
            }
        }
    }
    false
}

pub(super) fn progress(ledger: &records::SyncLedger, tombstones: usize) -> SyncProgress {
    let e = ledger.entity.as_ref();
    let ready = e.is_some_and(|e| e.ready);
    let pages = e.map_or(0, |e| e.pages);
    SyncProgress {
        phase: if ready {
            ProgressPhase::Ready
        } else if pages > 0 {
            ProgressPhase::Bootstrapping
        } else {
            ProgressPhase::NotStarted
        },
        ready,
        pages,
        changes: e.map_or(0, |e| e.changes),
        tombstones,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SchemaDefinition, TableDefinition};
    use std::{
        collections::{BTreeMap, BTreeSet},
        future::Future,
        task::{Context, Poll, Waker},
    };
    fn run<F: Future>(f: F) -> F::Output {
        match std::pin::pin!(f).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(v) => v,
            Poll::Pending => panic!("snapshot suspended"),
        }
    }

    fn owner() -> CommandDatabase<SnapshotStorage> {
        let mut db = run(CommandDatabase::open(
            SnapshotStorage::default(),
            SchemaDefinition {
                version: 1,
                tables: BTreeMap::from([("items".into(), TableDefinition::default())]),
            },
            vec![],
            [0x42; 16],
            SyncOptions::default(),
        ))
        .unwrap();
        db.configure_entities(
            EntityConfig {
                scope: "backend:a".into(),
                tables: BTreeSet::from(["items".into()]),
            },
            || {
                Ok(EntityContext {
                    identity: Json::Null,
                    now_ms: 100,
                })
            },
        )
        .unwrap();
        db
    }

    #[test]
    fn e11_pull_tickets_capture_opaque_checkpoint_and_are_owner_bound() {
        let _lock = crate::command::TEST_OWNERS.lock().unwrap();
        let mut a = owner();
        let mut b = owner();
        let batch = a
            .begin_pull()
            .expect("configured owner admits pull without holding permit");
        assert_eq!(batch.since, None);
        assert_eq!(batch.scope, "backend:a");
        assert_eq!(batch.generation, 1);
        assert_eq!(a.begin_pull(), Err(CommandError::Busy));
        assert_eq!(
            b.abort_pull(batch.request),
            Err(CommandError::InvalidTicket)
        );
        a.abort_pull(batch.request).unwrap();
        assert_eq!(
            a.abort_pull(batch.request),
            Err(CommandError::InvalidTicket)
        );
        a.ledger.entity.as_mut().unwrap().checkpoint = Some("opaque cursor / 7".into());
        assert_eq!(
            a.begin_pull().unwrap().since,
            Some("opaque cursor / 7".into())
        );
    }

    #[test]
    fn e13_progress_is_redacted_and_readiness_is_sticky_snapshot() {
        let _lock = crate::command::TEST_OWNERS.lock().unwrap();
        let mut db = owner();
        assert_eq!(db.sync_progress().phase, ProgressPhase::NotStarted);
        let e = db.ledger.entity.as_mut().unwrap();
        e.pages = 2;
        e.changes = 4;
        e.ready = true;
        e.checkpoint = Some("secret".into());
        let p = db.sync_progress();
        assert_eq!(p.phase, ProgressPhase::Ready);
        assert!(p.ready);
        assert_eq!(p.pages, 2);
        assert_eq!(p.changes, 4);
    }

    #[test]
    fn e12_empty_pages_commit_checkpoint_progress_and_consumption_atomically() {
        let _lock = crate::command::TEST_OWNERS.lock().unwrap();
        let mut db = owner();
        let subscription = db.observe_progress().unwrap();
        db.drain_events(0);
        let request = db.begin_pull().unwrap().request;
        run(db.apply_pull(
            request,
            PullPage {
                checkpoint: "cursor:1".into(),
                complete: false,
                changes: vec![],
            },
        ))
        .expect("valid empty page still commits checkpoint and progress");
        assert_eq!(db.sync_progress().pages, 1);
        assert_eq!(db.sync_progress().phase, ProgressPhase::Bootstrapping);
        assert_eq!(db.begin_pull().unwrap().since, Some("cursor:1".into()));
        assert_eq!(
            db.drain_events(0),
            vec![CommandEvent::Progress {
                subscription,
                value: db.sync_progress()
            }]
        );
        assert_eq!(
            run(db.apply_pull(
                request,
                PullPage {
                    checkpoint: "cursor:2".into(),
                    complete: true,
                    changes: vec![]
                }
            )),
            Err(CommandError::InvalidTicket)
        );
    }

    #[test]
    fn e17_reset_preserves_identity_and_invalidates_pending_subscriptions() {
        let _lock = crate::command::TEST_OWNERS.lock().unwrap();
        let mut db = owner();
        let permit = db.execution_owner.idle_permit().unwrap();
        db.transact(db.db.db.state.clone(), &permit, |_| {
            Ok(((), vec![], vec![]))
        })
        .unwrap();
        drop(permit);
        let old = db.observe_progress().unwrap();
        let pull = db.begin_pull().unwrap().request;
        let sequence = db.ledger.header.sequence;
        let counter = db.db.db.state.next_id;
        run(db.reset(ResetMode::PreserveRecovery))
            .expect("idle pristine entity owner may explicitly reset");
        assert!(db.drain_events(0).is_empty());
        assert!(!db.unsubscribe_sync(old));
        assert_eq!(db.abort_pull(pull), Err(CommandError::InvalidTicket));
        assert_eq!(db.ledger.entity.as_ref().unwrap().generation, 2);
        assert_eq!(db.ledger.header.sequence, sequence);
        assert_eq!(db.db.db.state.next_id, counter);
        assert_eq!(db.begin_pull().unwrap().since, None);
    }

    #[test]
    fn e15_page_codec_is_exact_lossless_and_bounded() {
        let page = PullPage {
            checkpoint: "opaque".into(),
            complete: true,
            changes: vec![EntityChange {
                table: "items".into(),
                record_id: "r".into(),
                op: EntityOperation::Update,
                changes: BTreeMap::from([
                    ("i".into(), Value::Integer(i64::MAX)),
                    ("f".into(), Value::Float(FiniteFloat::new(42.0).unwrap())),
                ]),
                removed_fields: BTreeSet::new(),
                timestamp_ms: 2,
                field_timestamps: BTreeMap::from([("i".into(), 2), ("f".into(), 2)]),
            }],
        };
        let wire = page
            .to_json()
            .expect("owned page has bounded lossless tagged representation");
        assert_eq!(PullPage::from_json(&wire).unwrap(), page);
        let mut wrong = wire.clone();
        let Json::Object(m) = &mut wrong else {
            panic!()
        };
        m.insert("unknown".into(), Json::Null);
        assert!(PullPage::from_json(&wrong).is_err());
        let too_many = PullPage {
            changes: vec![page.changes[0].clone(); 1001],
            ..page.clone()
        };
        assert!(too_many.to_json().is_err());
        let huge = PullPage {
            checkpoint: "x".repeat(1_048_576),
            ..page
        };
        assert!(huge.to_json().is_err());
    }

    #[test]
    fn e14_stale_pull_retires_original_ticket_and_busy_retains_it() {
        let _lock = crate::command::TEST_OWNERS.lock().unwrap();
        let mut db = owner();
        let request = db.begin_pull().unwrap().request;
        let permit = db.execution_owner.idle_permit().unwrap();
        let page = PullPage {
            checkpoint: "one".into(),
            complete: true,
            changes: vec![],
        };
        assert_eq!(
            run(db.apply_pull(request, page.clone())),
            Err(CommandError::Busy)
        );
        drop(permit);
        db.ledger.entity.as_mut().unwrap().revision += 1;
        assert_eq!(
            run(db.apply_pull(request, page.clone())),
            Err(CommandError::StalePull)
        );
        assert_eq!(
            run(db.apply_pull(request, page)),
            Err(CommandError::InvalidTicket)
        );
        assert_eq!(db.sync_progress().pages, 0);
    }

    #[test]
    fn e04_adapter_validates_live_entity_payload_against_schema() {
        let _lock = crate::command::TEST_OWNERS.lock().unwrap();
        let mut db = owner();
        let permit = db.execution_owner.idle_permit().unwrap();
        db.transact(db.db.db.state.clone(), &permit, |_| {
            Ok(((), vec![], vec![]))
        })
        .unwrap();
        drop(permit);
        db.entity_provider = Some(Box::new(|| {
            Ok(EntityContext {
                identity: Json::Null,
                now_ms: 100,
            })
        }));
        run(db.insert("items", BTreeMap::new())).unwrap();
        validate_sync_model(&db.ledger, &db.db.db.state).unwrap();
        let mut corrupted = db.ledger.clone();
        let record = corrupted.records.values_mut().next().unwrap();
        let Json::Object(args) = &mut record.args else {
            panic!()
        };
        let Json::Object(changes) = args.get_mut("changes").unwrap() else {
            panic!()
        };
        changes.insert("undeclared".into(), Json::Integer(7));
        corrupted.validate().unwrap();
        assert!(
            validate_sync_model(&corrupted, &db.db.db.state).is_err(),
            "raw scalar metadata is not a schema-valid queued entity payload"
        );
    }

    fn delete_page(id: &str) -> PullPage {
        PullPage {
            checkpoint: format!("after-delete:{id}"),
            complete: true,
            changes: vec![EntityChange {
                table: "items".into(),
                record_id: id.into(),
                op: EntityOperation::Delete,
                changes: BTreeMap::new(),
                removed_fields: BTreeSet::new(),
                timestamp_ms: 300,
                field_timestamps: BTreeMap::new(),
            }],
        }
    }

    // API-0009 section 4: insertion protection exists to prevent lost-outcome mapping
    // collisions. A pulled Delete for an identity this device has never seen inserts a
    // tombstone at exactly the identity a pending create may still be mapped to.
    #[test]
    fn e14_pulled_delete_of_unseen_identity_cannot_occupy_a_pending_mapping_target() {
        let _lock = crate::command::TEST_OWNERS.lock().unwrap();
        let mut db = owner();
        // Without an unmapped producer a first-seen tombstone stays an ordinary change.
        let request = db.begin_pull().unwrap().request;
        run(db.apply_pull(request, delete_page("server:unrelated"))).unwrap();
        assert_eq!(db.sync_progress().tombstones, 1);

        let local = run(db.insert("items", BTreeMap::new())).unwrap().id;
        let key = db.ledger.records.keys().next().unwrap().clone();
        assert_eq!(db.prepare_push(200, 0).unwrap().envelopes.len(), 1);
        // The backend stored the row as server:9 and another client deleted it before
        // this device processed the create's outcome.
        let request = db.begin_pull().unwrap().request;
        let progress = db.sync_progress();
        assert_eq!(
            run(db.apply_pull(request, delete_page("server:9"))),
            Err(CommandError::PullConflict),
            "a first-seen tombstone would occupy the pending create's mapping target"
        );
        assert_eq!(db.sync_progress(), progress);
        db.abort_pull(request).unwrap();

        db.receive(&key, Some(RemoteOutcome::Applied(Json::Null)))
            .unwrap();
        run(db.reconcile(
            &key,
            ReconcilePlan {
                writes: vec![],
                mappings: BTreeMap::from([(local, "server:9".into())]),
            },
        ))
        .expect("the accepted create keeps its mapping exit");
        assert_eq!(run(db.fetch(&db.query("items"))).unwrap().len(), 1);

        let request = db.begin_pull().unwrap().request;
        run(db.apply_pull(request, delete_page("server:9"))).unwrap();
        assert!(run(db.fetch(&db.query("items"))).unwrap().is_empty());
        assert_eq!(db.sync_progress().tombstones, 2);
    }

    fn insert_page(id: &str) -> PullPage {
        PullPage {
            checkpoint: format!("after-insert:{id}"),
            complete: true,
            changes: vec![EntityChange {
                table: "items".into(),
                record_id: id.into(),
                op: EntityOperation::Create,
                changes: BTreeMap::new(),
                removed_fields: BTreeSet::new(),
                timestamp_ms: 300,
                field_timestamps: BTreeMap::new(),
            }],
        }
    }

    // API-0009 section 4: a sent cancellation that completed its rollback has no local row
    // and can never be mapped, so its retained created identity does not make every pulled
    // insertion into that table a mapping collision.
    #[test]
    fn e14_rolled_back_sent_cancellation_does_not_block_pulled_insertion() {
        let _lock = crate::command::TEST_OWNERS.lock().unwrap();
        let mut db = owner();
        let local = run(db.insert("items", BTreeMap::new())).unwrap().id;
        let key = db.ledger.records.keys().next().unwrap().clone();
        assert_eq!(db.prepare_push(200, 0).unwrap().envelopes.len(), 1);
        // Control: the sent create is still mappable and protects insertion.
        let request = db.begin_pull().unwrap().request;
        assert_eq!(
            run(db.apply_pull(request, insert_page("server:new"))),
            Err(CommandError::PullConflict)
        );
        db.abort_pull(request).unwrap();
        run(db.cancel(&key)).unwrap();
        let tombstone = db.ledger.records[&key].clone();
        assert_eq!(tombstone.state, SyncState::Cancelled);
        assert!(tombstone.rolled_back && tombstone.reconciled && tombstone.frozen.is_some());
        assert!(tombstone.created.contains_key(&local));
        assert!(db.ledger.live_keys().contains(&key), "live until forget");
        // Control: a cancellation whose inverse did not complete still protects insertion.
        let mut unresolved = db.ledger.clone();
        unresolved.records.get_mut(&key).unwrap().rolled_back = false;
        let page = insert_page("server:new");
        let before = run(db.touched_snapshot(&[("items".into(), "server:new".into())])).unwrap();
        let mut planner = PullPlanner::new(&unresolved, before, page).unwrap();
        assert_eq!(
            planner
                .advance(
                    &db.db.db.schema,
                    &BTreeMap::new(),
                    &BTreeSet::new(),
                    &unresolved
                )
                .map(|_| ()),
            Err(CommandError::PullConflict)
        );
        // The other clauses stay: the tombstone's own created identity remains protected.
        let request = db.begin_pull().unwrap().request;
        assert_eq!(
            run(db.apply_pull(request, insert_page(&local))),
            Err(CommandError::PullConflict)
        );
        assert_eq!(
            run(db.apply_pull(request, insert_page("server:new"))),
            Ok(()),
            "a rolled-back sent tombstone must not block unrelated pulled rows"
        );
        assert_eq!(run(db.fetch(&db.query("items"))).unwrap().len(), 1);
        assert!(db.ledger.live_keys().contains(&key));
    }

    #[test]
    fn e15_overflow_preserves_data_progress_and_subscriptions() {
        let _lock = crate::command::TEST_OWNERS.lock().unwrap();
        let mut db = owner();
        let subscription = db.observe_progress().unwrap();
        db.ledger.entity.as_mut().unwrap().pages = i64::MAX;
        let request = db.begin_pull().unwrap().request;
        assert_eq!(
            run(db.apply_pull(
                request,
                PullPage {
                    checkpoint: "next".into(),
                    complete: true,
                    changes: vec![]
                }
            )),
            Err(CommandError::LimitExceeded)
        );
        assert_eq!(db.sync_progress().pages, i64::MAX);
        db.abort_pull(request).unwrap();
        let e = db.ledger.entity.as_mut().unwrap();
        e.pages = 0;
        e.generation = i64::MAX;
        assert_eq!(
            run(db.reset(ResetMode::DiscardRecovery)),
            Err(CommandError::LimitExceeded)
        );
        assert!(db.unsubscribe_sync(subscription));
        assert!(db.drain_events(0).is_empty());
    }
}

#[cfg(test)]
mod incremental_tests {
    use super::*;
    use crate::model::{SchemaDefinition, TableDefinition};
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };

    fn run<F: Future>(f: F) -> F::Output {
        match std::pin::pin!(f).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("memory fixture suspended"),
        }
    }
    fn change(index: usize, body: &str, time: i64) -> EntityChange {
        EntityChange {
            table: "items".into(),
            record_id: format!("r{index:04}"),
            op: EntityOperation::Update,
            changes: BTreeMap::from([("body".into(), Value::String(body.into()))]),
            removed_fields: BTreeSet::new(),
            timestamp_ms: time,
            field_timestamps: BTreeMap::from([("body".into(), time)]),
        }
    }
    fn seeded(count: usize) -> CommandDatabase<SnapshotStorage> {
        let mut db = run(CommandDatabase::open(
            SnapshotStorage::default(),
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
            },
            vec![],
            [0x5d; 16],
            SyncOptions::default(),
        ))
        .unwrap();
        db.configure_entities(
            EntityConfig {
                scope: "incremental-pull".into(),
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
                changes: (0..count).map(|i| change(i, "old", 1)).collect(),
            },
        ))
        .unwrap();
        db
    }
    fn pull_work(count: usize, changed: usize) -> counters::WorkSnapshot {
        let mut db = seeded(count);
        let request = db.begin_pull().unwrap().request;
        counters::reset();
        run(db.apply_pull(
            request,
            PullPage {
                checkpoint: "next".into(),
                complete: true,
                changes: (0..changed).map(|i| change(i, "new", 2)).collect(),
            },
        ))
        .unwrap();
        let work = counters::snapshot();
        assert_eq!(db.sync_progress().changes, (count + changed) as i64);
        assert_eq!(run(db.fetch(&db.query("items"))).unwrap().len(), count);
        work
    }
    fn equal_work(small: counters::WorkSnapshot, large: counters::WorkSnapshot) {
        eprintln!("small {small:?}\nlarge {large:?}");
        assert_eq!(
            small.ledger_entries_cloned, large.ledger_entries_cloned,
            "ledger clones scale with unrelated metadata"
        );
        assert_eq!(
            small.metadata_entries_visited, large.metadata_entries_visited,
            "metadata visits scale with unrelated metadata"
        );
        assert_eq!(
            small.rows_cloned, large.rows_cloned,
            "row copies scale with unrelated rows"
        );
        assert_eq!(
            small.packets_encoded, large.packets_encoded,
            "packet encoding scales with unrelated metadata"
        );
        assert_eq!(
            small.packets_parsed, large.packets_parsed,
            "packet decoding scales with unrelated metadata"
        );
    }
    #[test]
    fn si_y01_pull_one_is_bounded_at_n_and_m() {
        let _lock = TEST_OWNERS.lock().unwrap_or_else(|e| e.into_inner());
        equal_work(pull_work(10, 1), pull_work(1000, 1));
    }
    #[test]
    fn si_y01_pull_hundred_is_bounded_at_n_and_m() {
        let _lock = TEST_OWNERS.lock().unwrap_or_else(|e| e.into_inner());
        equal_work(pull_work(100, 100), pull_work(1000, 100));
    }
    fn merge_work(count: usize) -> counters::WorkSnapshot {
        let mut db = seeded(count);
        let request = db.begin_pull().unwrap().request;
        counters::reset();
        let step = run(db.merge_prepare(
            request,
            PullPage {
                checkpoint: "next".into(),
                complete: true,
                changes: vec![change(0, "new", 2)],
            },
            BTreeSet::from(["items".into()]),
        ))
        .unwrap();
        let MergeStep::Offer {
            session,
            step,
            ..
        } = step
        else {
            panic!("one custom offer")
        };
        let input = db.merge_input(session, step).unwrap();
        assert!(matches!(
            run(db.merge_reply(session, step, input.default.row)),
            Ok(MergeStep::Ready { .. })
        ));
        run(db.merge_commit(session)).unwrap();
        let work = counters::snapshot();
        assert_eq!(db.sync_progress().changes, count as i64 + 1);
        work
    }
    #[test]
    fn si_y07_merge_prepare_reply_commit_is_bounded() {
        let _lock = TEST_OWNERS.lock().unwrap_or_else(|e| e.into_inner());
        equal_work(merge_work(10), merge_work(1000));
    }
    #[test]
    fn si_y09_merge_epoch_stales_after_record_only_write_without_metadata_walk() {
        let _lock = TEST_OWNERS.lock().unwrap_or_else(|e| e.into_inner());
        for count in [10, 1000] {
            let mut db = seeded(count);
            let request = db.begin_pull().unwrap().request;
            let MergeStep::Ready {
                session,
                ..
            } = run(db.merge_prepare(
                request,
                PullPage {
                    checkpoint: "next".into(),
                    complete: true,
                    changes: vec![],
                },
                BTreeSet::new(),
            ))
            .unwrap()
            else {
                panic!("empty merge ready")
            };
            let command = db.request().unwrap();
            assert!(
                run(db.begin(
                    command,
                    CommandDefinition {
                        name: "noop-v1".into(),
                        arguments: BTreeMap::new(),
                        reference_paths: vec![]
                    },
                    Json::Object(BTreeMap::new()),
                    Some(Json::Null),
                    10
                ))
                .unwrap()
            );
            run(db.finish(command, Json::Null)).unwrap();
            counters::reset();
            assert_eq!(run(db.merge_commit(session)), Err(CommandError::StalePull));
            let work = counters::snapshot();
            assert_eq!(
                work.metadata_entries_visited, 0,
                "staleness atM={count}: {work:?}"
            );
            assert_eq!(db.queue_status().pending, 1);
            assert_eq!(db.sync_progress().pages, 1);
        }
    }
    #[test]
    fn si_c04_passive_offer_counts_retained_current_image() {
        let _serial = TEST_OWNERS
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        for present in [true, false] {
            let db = seeded(1);
            let incoming = change(
                if present {
                    0
                } else {
                    1
                },
                "new",
                2,
            );
            let before =
                run(db.touched_snapshot(&[(incoming.table.clone(), incoming.record_id.clone())]))
                    .unwrap();
            let mut planner = PullPlanner::new(
                &db.ledger,
                before,
                PullPage {
                    checkpoint: "offer".into(),
                    complete: true,
                    changes: vec![incoming],
                },
            )
            .unwrap();
            counters::reset();
            let offer = planner
                .advance(
                    &db.db.db.schema,
                    &BTreeMap::new(),
                    &BTreeSet::from(["items".into()]),
                    &db.ledger,
                )
                .unwrap()
                .unwrap();
            assert_eq!(offer.current.is_some(), present);
            assert_eq!(
                offer.default.row.unwrap().fields["body"],
                Value::String("new".into())
            );
            assert_eq!(
                counters::snapshot().rows_cloned,
                if present {
                    3
                } else {
                    0
                },
                "two resolver candidates and one retained offer image"
            );
        }
    }

    #[test]
    fn si_c04_merge_reply_counts_internal_candidates_only() {
        let _serial = TEST_OWNERS
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        for (present, deleting, expected) in [(true, false, 4), (true, true, 3), (false, false, 1)]
        {
            let mut db = seeded(1);
            let mut incoming = change(
                if present {
                    0
                } else {
                    1
                },
                "new",
                2,
            );
            if deleting {
                incoming.op = EntityOperation::Delete;
                incoming.changes.clear();
                incoming.field_timestamps.clear();
            }
            let request = db.begin_pull().unwrap().request;
            let MergeStep::Offer {
                session,
                step,
                ..
            } = run(db.merge_prepare(
                request,
                PullPage {
                    checkpoint: "reply".into(),
                    complete: true,
                    changes: vec![incoming],
                },
                BTreeSet::from(["items".into()]),
            ))
            .unwrap()
            else {
                panic!("one offer")
            };
            counters::reset();
            let input = db.merge_input(session, step).unwrap();
            assert_eq!(
                counters::snapshot().rows_cloned,
                0,
                "public result copies stay outside staging accounting"
            );
            assert!(matches!(
                run(db.merge_reply(session, step, input.default.row)),
                Ok(MergeStep::Ready { .. })
            ));
            assert_eq!(
                counters::snapshot().rows_cloned,
                expected,
                "internal current/default candidate copies plus two present-row stale snapshots, present={present} delete={deleting}"
            );
            db.merge_abort(session).unwrap();
        }
    }
}
