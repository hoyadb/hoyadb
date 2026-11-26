//! API-0010: bounded diagnostics and safe automatic formatting, without I/O.
use super::*;
use crate::storage::StorageError;
use std::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DiagnosticSnapshot {
    pub schema_version: u32,
    pub pending: usize,
    pub sending: usize,
    pub awaiting_outcome: usize,
    pub live: usize,
    pub blocked: bool,
    pub ready: bool,
    pub pages: i64,
    pub changes: i64,
    pub tombstones: usize,
}

impl<S> CommandDatabase<S> {
    pub fn diagnostics(&self) -> DiagnosticSnapshot {
        let records = self.ledger.records.values();
        let live = self.ledger.live_key_refs().len();
        let entity = self.ledger.entity.as_ref();
        DiagnosticSnapshot {
            schema_version: self.db.db.state.version,
            pending: records
                .clone()
                .filter(|record| record.state == SyncState::Queued)
                .count(),
            sending: records
                .clone()
                .filter(|record| record.state == SyncState::Sent)
                .count(),
            awaiting_outcome: records
                .filter(|record| record.state == SyncState::Acknowledged)
                .count(),
            live,
            blocked: live >= self.ledger.header.options.capacity,
            ready: entity.is_some_and(|state| state.ready),
            pages: entity.map_or(0, |state| state.pages),
            changes: entity.map_or(0, |state| state.changes),
            tombstones: entity.map_or(0, |state| {
                crate::storage::count_work!(metadata_entries_visited, state.metadata.len() as u64);
                state
                    .metadata
                    .values()
                    .filter(|value| value.tombstone.is_some())
                    .count()
            }),
        }
    }
}

// Deliberately do not traverse owned fields: output is bounded even for large
// payloads, and adding a sensitive field cannot silently expand diagnostics.
macro_rules! redacted_debug {
    ($($name:ty),+ $(,)?) => {$ (
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($name), " { .. }"))
            }
        }
    )+};
}

redacted_debug!(
    CapturedContext,
    EntityContext,
    CommandResult,
    PushBatch,
    records::SyncRecord,
    records::SyncLedger,
    PullBatch,
    PullPage,
    QueueEntry,
    QueueStatus,
    ConflictPolicy,
    Resolution
);

impl fmt::Debug for RemoteOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Applied(_) => "RemoteOutcome::Applied { .. }",
            Self::Corrected(_) => "RemoteOutcome::Corrected { .. }",
            Self::Rejected(_) => "RemoteOutcome::Rejected { .. }",
        })
    }
}

impl fmt::Debug for CommandOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Queued => "CommandOutcome::Queued",
            Self::Sent => "CommandOutcome::Sent",
            Self::Acknowledged => "CommandOutcome::Acknowledged",
            Self::Applied(_) => "CommandOutcome::Applied { .. }",
            Self::Corrected(_) => "CommandOutcome::Corrected { .. }",
            Self::Rejected(_) => "CommandOutcome::Rejected { .. }",
        })
    }
}

impl fmt::Debug for CommandEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Progress {
                ..
            } => "CommandEvent::Progress { .. }",
            Self::Data(_) => "CommandEvent::Data { .. }",
            Self::Status {
                ..
            } => "CommandEvent::Status { .. }",
            Self::Outcome {
                ..
            } => "CommandEvent::Outcome { .. }",
        })
    }
}

impl fmt::Debug for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Storage(_) => "CommandError::Storage { .. }",
            Self::InvalidCommand => "CommandError::InvalidCommand",
            Self::InvalidTicket => "CommandError::InvalidTicket",
            Self::Busy => "CommandError::Busy",
            Self::Capacity => "CommandError::Capacity",
            Self::InvalidPayload(_) => "CommandError::InvalidPayload { .. }",
            Self::Protocol => "CommandError::Protocol",
            Self::UnknownLocalReference => "CommandError::UnknownLocalReference",
            Self::RollbackConflict => "CommandError::RollbackConflict",
            Self::Closed => "CommandError::Closed",
            Self::LimitExceeded => "CommandError::LimitExceeded",
            Self::AuthorityRequired => "CommandError::AuthorityRequired",
            Self::StalePull => "CommandError::StalePull",
            Self::PullConflict => "CommandError::PullConflict",
        })
    }
}

impl fmt::Debug for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MigrationFailure {
                ..
            } => "StorageError::MigrationFailure { .. }",
            Self::SchemaConflict {
                ..
            } => "StorageError::SchemaConflict { .. }",
            Self::InvalidSchema {
                ..
            } => "StorageError::InvalidSchema { .. }",
            Self::InvalidQuery {
                ..
            } => "StorageError::InvalidQuery { .. }",
            Self::InvalidRow {
                ..
            } => "StorageError::InvalidRow { .. }",
            Self::BackendFailure {
                ..
            } => "StorageError::BackendFailure { .. }",
            Self::TableNotFound {
                ..
            } => "StorageError::TableNotFound { .. }",
            Self::RowAlreadyExists {
                ..
            } => "StorageError::RowAlreadyExists { .. }",
            Self::RowNotFound {
                ..
            } => "StorageError::RowNotFound { .. }",
            Self::StoreInUse => "StorageError::StoreInUse",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{model::*, storage::*};
    use std::{
        collections::{BTreeMap, BTreeSet},
        future::Future,
        task::{Context, Poll, Waker},
    };

    const PRIVATE: &str = "synthetic-private-value";
    const NAME: &str = "private-table";

    fn run<T>(future: impl Future<Output = T>) -> T {
        match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("snapshot operation unexpectedly suspended"),
        }
    }

    fn owner() -> CommandDatabase<SnapshotStorage> {
        run(CommandDatabase::open(
            SnapshotStorage::default(),
            SchemaDefinition {
                version: 7,
                tables: BTreeMap::from([(NAME.into(), TableDefinition::default())]),
            },
            vec![],
            [0xab; 16],
            SyncOptions {
                capacity: 2,
                ..SyncOptions::default()
            },
        ))
        .unwrap()
    }

    fn command(db: &mut CommandDatabase<SnapshotStorage>) -> (String, String) {
        let request = db.request().unwrap();
        assert!(
            run(db.begin(
                request,
                CommandDefinition {
                    name: "private-command-v1".into(),
                    arguments: BTreeMap::from([(
                        "value".into(),
                        Argument {
                            kind: ArgumentType::String,
                            optional: false
                        }
                    )]),
                    reference_paths: vec![],
                },
                Json::Object(BTreeMap::from([(
                    "value".into(),
                    Json::String(PRIVATE.into())
                )])),
                Some(Json::String(PRIVATE.into())),
                1,
            ))
            .unwrap()
        );
        let context = format!("{:?}", db.scope_context(request).unwrap());
        run(db.scope_insert(request, NAME, BTreeMap::new())).unwrap();
        let result = run(db.finish(request, Json::String(PRIVATE.into()))).unwrap();
        (result.key, context)
    }

    #[test]
    fn s12_owned_debug_omits_contents_but_explicit_serialization_retains_them() {
        let _serial = TEST_OWNERS.lock().unwrap();
        let mut db = owner();
        let (key, context) = command(&mut db);
        let push = db.prepare_push(1, 0).unwrap();
        let status = db.queue_status();
        let rows_before = db.ledger.to_rows().unwrap();
        let pull = PullPage {
            checkpoint: PRIVATE.into(),
            complete: false,
            changes: vec![EntityChange {
                table: NAME.into(),
                record_id: PRIVATE.into(),
                op: EntityOperation::Update,
                changes: BTreeMap::from([("private-field".into(), Value::String(PRIVATE.into()))]),
                removed_fields: BTreeSet::new(),
                timestamp_ms: 1,
                field_timestamps: BTreeMap::from([("private-field".into(), 1)]),
            }],
        };
        db.ledger.entity = Some(EntityLedger::new(EntityConfig {
            scope: PRIVATE.into(),
            tables: BTreeSet::from([NAME.into()]),
        }));
        let batch = db.begin_pull().unwrap();
        let data = CommandEvent::Data(Notification {
            subscription: Subscription(92),
            value: Ok(ObservationValue::Rows(vec![Row {
                id: PRIVATE.into(),
                fields: BTreeMap::new(),
            }])),
        });
        let mut outputs = vec![
            ("CapturedContext", context),
            (
                "EntityContext",
                format!(
                    "{:?}",
                    EntityContext {
                        identity: Json::String(PRIVATE.into()),
                        now_ms: 1
                    }
                ),
            ),
            (
                "CommandResult",
                format!(
                    "{:?}",
                    CommandResult {
                        key: key.clone(),
                        result: Json::String(PRIVATE.into())
                    }
                ),
            ),
            ("PushBatch", format!("{push:?}")),
            ("SyncRecord", format!("{:?}", db.ledger.records[&key])),
            ("SyncLedger", format!("{:?}", db.ledger)),
            ("PullBatch", format!("{batch:?}")),
            ("PullPage", format!("{pull:?}")),
            ("QueueEntry", format!("{:?}", status.commands[0])),
            ("QueueStatus", format!("{status:?}")),
            ("CommandEvent::Data", format!("{data:?}")),
            (
                "CommandEvent::Status",
                format!(
                    "{:?}",
                    CommandEvent::Status {
                        subscription: SyncSubscription(93),
                        value: status.clone()
                    }
                ),
            ),
            (
                "CommandEvent::Progress",
                format!(
                    "{:?}",
                    CommandEvent::Progress {
                        subscription: SyncSubscription(94),
                        value: db.sync_progress()
                    }
                ),
            ),
        ];
        for remote in [
            RemoteOutcome::Applied(Json::String(PRIVATE.into())),
            RemoteOutcome::Corrected(Json::String(PRIVATE.into())),
            RemoteOutcome::Rejected(PRIVATE.into()),
        ] {
            outputs.push(("RemoteOutcome", format!("{remote:?}")));
        }
        for value in [
            CommandOutcome::Applied(Json::String(PRIVATE.into())),
            CommandOutcome::Corrected(Json::String(PRIVATE.into())),
            CommandOutcome::Rejected(PRIVATE.into()),
        ] {
            outputs.push(("CommandOutcome", format!("{value:?}")));
            outputs.push((
                "CommandEvent::Outcome",
                format!(
                    "{:?}",
                    CommandEvent::Outcome {
                        subscription: SyncSubscription(91),
                        value
                    }
                ),
            ));
        }
        for error in [
            StorageError::MigrationFailure {
                message: PRIVATE.into(),
            },
            StorageError::SchemaConflict {
                message: PRIVATE.into(),
            },
            StorageError::InvalidSchema {
                message: PRIVATE.into(),
            },
            StorageError::InvalidQuery {
                message: PRIVATE.into(),
            },
            StorageError::InvalidRow {
                table: NAME.into(),
                id: PRIVATE.into(),
                message: PRIVATE.into(),
            },
            StorageError::BackendFailure {
                message: PRIVATE.into(),
            },
            StorageError::TableNotFound {
                table: NAME.into(),
            },
            StorageError::RowAlreadyExists {
                table: NAME.into(),
                id: PRIVATE.into(),
            },
            StorageError::RowNotFound {
                table: NAME.into(),
                id: PRIVATE.into(),
            },
        ] {
            outputs.push(("StorageError", format!("{error:?}")));
            outputs.push((
                "CommandError",
                format!("{:?}", CommandError::Storage(error)),
            ));
        }
        let leaked = outputs
            .iter()
            .filter(|(_, value)| {
                [
                    PRIVATE,
                    NAME,
                    "private-command",
                    "private-field",
                    &key,
                    &"ab".repeat(16),
                ]
                .iter()
                .any(|secret| value.contains(secret))
            })
            .map(|(name, _)| *name)
            .collect::<Vec<_>>();
        assert!(
            leaked.is_empty(),
            "automatic Debug disclosed protected fields in {leaked:?}"
        );
        assert!(
            outputs.iter().all(|(_, text)| text.len() <= 100),
            "safe Debug is bounded independently of payload size"
        );
        db.ledger.entity = None;
        assert_eq!(db.ledger.to_rows().unwrap(), rows_before);
        assert!(
            push.envelopes[0].contains(PRIVATE),
            "explicit retry data stays complete"
        );
        assert!(
            Json::String(PRIVATE.into())
                .canonical()
                .unwrap()
                .contains(PRIVATE)
        );
    }

    #[test]
    fn s12_diagnostics_counts_dependency_closure_without_payload_projection() {
        let _serial = TEST_OWNERS.lock().unwrap();
        let mut db = owner();
        assert_eq!(
            db.diagnostics(),
            DiagnosticSnapshot {
                schema_version: 7,
                ..DiagnosticSnapshot::default()
            }
        );
        let (producer_key, _) = command(&mut db);
        assert_eq!(db.diagnostics().pending, 1);
        assert_eq!(db.diagnostics().sending, 0);
        db.prepare_push(1, 0).unwrap();
        assert_eq!(db.diagnostics().pending, 0);
        assert_eq!(db.diagnostics().sending, 1);
        let mut producer = db.ledger.records[&producer_key].clone();
        let local = producer.created.keys().next().unwrap().clone();
        producer.state = SyncState::Applied;
        producer.reconciled = true;
        producer.outcome = Some(RemoteOutcome::Applied(Json::String(PRIVATE.into())));
        producer.frozen = Some(producer.envelope().canonical().unwrap());
        producer.attempts = 1;
        producer.due_ms = 1;
        producer
            .mappings
            .insert(local.clone(), "remote:producer".into());
        let mut consumer = producer.clone();
        consumer.key = format!("0000000000000001-{}-0000000000000002", "ab".repeat(16));
        consumer.order = 2;
        consumer.state = SyncState::Acknowledged;
        consumer.reconciled = false;
        consumer.created.clear();
        consumer.undo.clear();
        consumer.outcome = None;
        consumer.mappings.clear();
        consumer.undo = vec![CommittedChange {
            table: NAME.into(),
            id: "private-child".into(),
            before: Some(Row {
                id: "private-child".into(),
                fields: BTreeMap::from([("parent".into(), Value::Reference(local))]),
            }),
            after: None,
        }];
        consumer.frozen = Some(consumer.envelope().canonical().unwrap());
        db.ledger.records.insert(producer_key, producer);
        db.ledger.records.insert(consumer.key.clone(), consumer);
        db.ledger.header.sequence = 3;
        let metadata = RowMetadata {
            tombstone: Some(FieldStamp {
                timestamp_ms: 1,
                authoritative: true,
            }),
            ..RowMetadata::default()
        };
        let mut entity = EntityLedger::new(EntityConfig {
            scope: PRIVATE.into(),
            tables: BTreeSet::from([NAME.into()]),
        });
        entity.ready = true;
        entity.pages = 3;
        entity.changes = 4;
        entity.checkpoint = Some(PRIVATE.into());
        entity
            .metadata
            .insert((NAME.into(), PRIVATE.into()), metadata);
        db.ledger.entity = Some(entity);
        db.ledger
            .validate()
            .expect("counter fixture is an accepted dependency/recovery ledger");
        let expected = DiagnosticSnapshot {
            schema_version: 7,
            pending: 0,
            sending: 0,
            awaiting_outcome: 1,
            live: 2,
            blocked: true,
            ready: true,
            pages: 3,
            changes: 4,
            tombstones: 1,
        };
        assert_eq!(
            db.diagnostics(),
            expected,
            "diagnostics preserve queue closure and progress counts"
        );
        let status = db.queue_status();
        assert_eq!(expected.live, status.live);
        assert!(!format!("{:?}", db.diagnostics()).contains(PRIVATE));
        db.ledger.records.clear();
        db.ledger.entity = None;
        assert_eq!(
            db.diagnostics(),
            DiagnosticSnapshot {
                schema_version: 7,
                ..DiagnosticSnapshot::default()
            }
        );
    }
}
