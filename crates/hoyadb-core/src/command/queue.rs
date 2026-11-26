//! API-0006 section 5 / TC-0032 C09/C12/C16/C19: pure durable queue planning.
use super::{CommandError, Json, ledger_txn::LedgerTxn, records::*};
use crate::{model::Notification, storage::Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, PartialEq, Eq)]
pub struct PushBatch {
    pub envelopes: Vec<String>,
    pub next_due_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReconcileWrite {
    Entity {
        change: super::entity::EntityChange,
    },
    Put {
        table: String,
        id: String,
        fields: BTreeMap<String, Value>,
    },
    Remove {
        table: String,
        id: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReconcilePlan {
    pub writes: Vec<ReconcileWrite>,
    pub mappings: BTreeMap<String, String>,
}

/// SPEC-0039: an application's explicit expectation for a row a correction writes outside the
/// command closure. `expected` is the complete public before-image (no `id`, no `__hoya_`
/// field); None means the row must be absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CorrectionGuard {
    pub table: String,
    pub id: String,
    pub expected: Option<BTreeMap<String, Value>>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum CommandOutcome {
    Queued,
    Sent,
    Acknowledged,
    Applied(Json),
    Corrected(Json),
    Rejected(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueuedWriteKind {
    Command,
    Entity,
}

#[derive(Clone, PartialEq, Eq)]
pub struct QueueEntry {
    pub kind: QueuedWriteKind,
    pub table: Option<String>,
    pub record_id: Option<String>,
    pub key: String,
    pub state: SyncState,
    pub blocked_on: Vec<String>,
    pub awaiting_local: bool,
    /// SPEC-0038 C2: key of the earliest record withholding this Queued record under
    /// row-causal dispatch. Computed, never persisted.
    pub awaiting_key: Option<String>,
    pub rolled_back: bool,
    pub attempts: u32,
    pub next_due_ms: Option<i64>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct QueueStatus {
    pub pending: usize,
    pub sending: usize,
    pub awaiting_outcome: usize,
    pub live: usize,
    pub blocked: bool,
    pub commands: Vec<QueueEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SyncSubscription(pub(super) u64);

#[derive(Clone, PartialEq, Eq)]
pub enum CommandEvent {
    Progress {
        subscription: SyncSubscription,
        value: super::pull::SyncProgress,
    },
    Data(Notification),
    Status {
        subscription: SyncSubscription,
        value: QueueStatus,
    },
    Outcome {
        subscription: SyncSubscription,
        value: CommandOutcome,
    },
}

/// SPEC-0038 C2: whether a record that wrote rows can still change their server outcome.
fn unresolved(record: &SyncRecord) -> bool {
    match record.state {
        SyncState::Queued | SyncState::Sent | SyncState::Acknowledged => true,
        SyncState::Applied | SyncState::Corrected => !record.reconciled,
        SyncState::Rejected => !record.rolled_back,
        SyncState::Cancelled | SyncState::Discarded => false,
    }
}

/// SPEC-0038 C2: the earliest unresolved earlier record that wrote any row this record wrote.
/// Entity-entity pairs are exempt; they merge per field with stamps.
pub(super) fn blocker<'a>(ledger: &'a SyncLedger, record: &SyncRecord) -> Option<&'a SyncRecord> {
    let rows = record
        .undo
        .iter()
        .map(|change| (change.table.as_str(), change.id.as_str()))
        .collect::<BTreeSet<_>>();
    if rows.is_empty() {
        return None;
    }
    ledger
        .records
        .values()
        .filter(|other| {
            other.order < record.order
                && unresolved(other)
                && !(other.entity.is_some() && record.entity.is_some())
                && other
                    .undo
                    .iter()
                    .any(|change| rows.contains(&(change.table.as_str(), change.id.as_str())))
        })
        .min_by_key(|other| other.order)
}

pub(super) fn push_in(
    next: &mut LedgerTxn<'_>,
    now: i64,
    jitter: u16,
) -> Result<PushBatch, CommandError> {
    if !(0..=253_402_300_799_999).contains(&now) || jitter > 2500 {
        return Err(CommandError::InvalidCommand);
    }
    next.validate()?;
    // SPEC-0035: last_ms is only the monotonic identity floor; scheduling uses the raw
    // sample. A Sent deadline beyond the largest delay the planner can produce from this
    // sample (maximum backoff plus maximum jitter) is capped, so a corrected forward jump
    // cannot postpone a retry indefinitely. Deadlines produced at earlier-or-equal samples
    // are never shortened.
    next.header_mut().last_ms = next.header.last_ms.max(now);
    let max_delay = i128::from(next.header.options.max_delay_ms);
    let ceiling =
        (i128::from(now) + max_delay + max_delay * 2500 / 10000).min(i128::from(i64::MAX));
    let ceiling = i64::try_from(ceiling).map_err(|_| CommandError::LimitExceeded)?;
    let capped = next
        .records
        .values()
        .filter(|record| record.state == SyncState::Sent && record.due_ms > ceiling)
        .map(|record| record.key.clone())
        .collect::<Vec<_>>();
    let mut eligible = next
        .records
        .values()
        .filter(|r| {
            r.dependencies.is_empty()
                && (r.state == SyncState::Queued && blocker(next, r).is_none()
                    || r.state == SyncState::Sent && r.due_ms.min(ceiling) <= now)
        })
        .map(|r| (r.order, r.key.clone()))
        .collect::<Vec<_>>();
    for key in capped {
        next.record_mut(&key).expect("selected retry").due_ms = ceiling;
    }
    eligible.sort();
    let mut envelopes = Vec::new();
    let options = next.header.options.clone();
    for (_, key) in eligible {
        let record = next.record_mut(&key).expect("selected record");
        if record.frozen.is_none() {
            record.frozen = Some(record.envelope().canonical()?);
        }
        record.attempts = record
            .attempts
            .checked_add(1)
            .ok_or(CommandError::LimitExceeded)?;
        let mut base = options.initial_delay_ms;
        for _ in 1..record.attempts {
            if base >= options.max_delay_ms {
                break;
            }
            base = base
                .saturating_mul(i64::from(options.multiplier))
                .min(options.max_delay_ms);
        }
        let delay = i128::from(base) + i128::from(base) * i128::from(jitter) / 10000;
        let delay = i64::try_from(delay).map_err(|_| CommandError::LimitExceeded)?;
        record.due_ms = now.checked_add(delay).ok_or(CommandError::LimitExceeded)?;
        record.state = SyncState::Sent;
        envelopes.push(record.frozen.clone().expect("frozen before dispatch"));
    }
    next.serialization_check()?;
    let next_due_ms = next
        .records
        .values()
        .filter(|r| r.state == SyncState::Sent)
        .map(|r| r.due_ms)
        .min();
    Ok(PushBatch {
        envelopes,
        next_due_ms,
    })
}

/// SPEC-0038 C3: explicitly return an Acknowledged record without outcome to Sent, due at the
/// validated raw sample, keeping frozen bytes, attempts and key. Sent is a no-op. Never sends.
pub(super) fn redeliver_in(
    next: &mut LedgerTxn<'_>,
    key: &str,
    now: i64,
) -> Result<(), CommandError> {
    if !(0..=253_402_300_799_999).contains(&now) {
        return Err(CommandError::InvalidCommand);
    }
    next.validate()?;
    let record = next.records.get(key).ok_or(CommandError::Protocol)?;
    match record.state {
        SyncState::Sent => Ok(()),
        SyncState::Acknowledged if record.outcome.is_none() => {
            let record = next.record_mut(key).expect("known redelivery key");
            record.state = SyncState::Sent;
            record.due_ms = now;
            next.serialization_check()?;
            Ok(())
        }
        _ => Err(CommandError::Protocol),
    }
}

pub(super) fn receive_in(
    next: &mut LedgerTxn<'_>,
    key: &str,
    incoming: Option<RemoteOutcome>,
) -> Result<(), CommandError> {
    next.validate()?;
    let old = next.records.get(key).ok_or(CommandError::Protocol)?;
    if old.state == SyncState::Discarded && old.frozen.is_none() {
        return Err(CommandError::Protocol);
    }
    if old.state == SyncState::Cancelled {
        return Ok(());
    }
    if old.state == SyncState::Queued {
        return Err(CommandError::Protocol);
    }
    if let Some(previous) = &old.outcome {
        return if incoming.as_ref().is_none_or(|value| value == previous) {
            Ok(())
        } else {
            Err(CommandError::Protocol)
        };
    }
    let record = next.record_mut(key).expect("known receipt key");
    record.state = match &incoming {
        None => SyncState::Acknowledged,
        Some(RemoteOutcome::Applied(_)) => SyncState::Applied,
        Some(RemoteOutcome::Corrected(_)) => SyncState::Corrected,
        Some(RemoteOutcome::Rejected(_)) => SyncState::Rejected,
    };
    record.outcome = incoming;
    next.serialization_check()?;
    Ok(())
}

// Test-only owned results preserve the original planner characterization surface.
// Production planners mutate a rollback guard and never copy the full ledger.
#[cfg(test)]
pub(super) fn plan_push(
    ledger: &SyncLedger,
    now: i64,
    jitter: u16,
) -> Result<(SyncLedger, PushBatch), CommandError> {
    let mut next = ledger.clone();
    let mut txn = LedgerTxn::new(&mut next, false);
    let batch = push_in(&mut txn, now, jitter)?;
    txn.commit();
    Ok((next, batch))
}
#[cfg(test)]
pub(super) fn plan_receive(
    ledger: &SyncLedger,
    key: &str,
    incoming: Option<RemoteOutcome>,
) -> Result<SyncLedger, CommandError> {
    let mut next = ledger.clone();
    let mut txn = LedgerTxn::new(&mut next, false);
    receive_in(&mut txn, key, incoming)?;
    txn.commit();
    Ok(next)
}

pub(super) fn trim_in(ledger: &mut LedgerTxn<'_>) {
    let protected = ledger.live_keys();
    let cleanup = ledger
        .records
        .values()
        .filter(|record| {
            !protected.contains(&record.key)
                && record.state == SyncState::Cancelled
                && record.frozen.is_none()
                && record.rolled_back
                && record.reconciled
                && (!record.reference_paths.is_empty()
                    || !record.dependencies.is_empty()
                    || !record.created.is_empty()
                    || !record.undo.is_empty())
        })
        .map(|record| record.key.clone())
        .collect::<Vec<_>>();
    for key in cleanup {
        let record = ledger.record_mut(&key).expect("selected cancelled record");
        record.reference_paths.clear();
        record.dependencies.clear();
        record.created.clear();
        record.undo.clear();
    }
    let live = ledger.live_keys();
    let mut history = ledger
        .records
        .values()
        .filter(|record| !live.contains(&record.key))
        .map(|record| (record.order, record.key.clone()))
        .collect::<Vec<_>>();
    history.sort_by(|a, b| b.cmp(a));
    for (_, key) in history.into_iter().skip(ledger.header.options.history) {
        ledger.remove_record(&key);
    }
}

#[cfg(test)]
fn trim(ledger: &mut SyncLedger) {
    let mut txn = LedgerTxn::new(ledger, false);
    trim_in(&mut txn);
    txn.commit();
}

pub(super) fn status(ledger: &SyncLedger) -> QueueStatus {
    let live = ledger.live_keys().len();
    let mut records = ledger.records.values().collect::<Vec<_>>();
    records.sort_by_key(|r| r.order);
    QueueStatus {
        pending: records
            .iter()
            .filter(|r| r.state == SyncState::Queued)
            .count(),
        sending: records
            .iter()
            .filter(|r| r.state == SyncState::Sent)
            .count(),
        awaiting_outcome: records
            .iter()
            .filter(|r| r.state == SyncState::Acknowledged)
            .count(),
        live,
        blocked: live >= ledger.header.options.capacity,
        commands: records
            .into_iter()
            .map(|r| QueueEntry {
                kind: if r.entity.is_some() {
                    QueuedWriteKind::Entity
                } else {
                    QueuedWriteKind::Command
                },
                table: r.entity.as_ref().map(|e| e.table.clone()),
                record_id: if r.entity.is_some() {
                    match &r.args {
                        Json::Object(args) => match args.get("recordId") {
                            Some(Json::String(id)) => Some(id.clone()),
                            _ => None,
                        },
                        _ => None,
                    }
                } else {
                    None
                },
                key: r.key.clone(),
                state: r.state.clone(),
                blocked_on: r.dependencies.iter().cloned().collect(),
                awaiting_local: r.outcome.is_some() && !r.reconciled,
                awaiting_key: (r.state == SyncState::Queued)
                    .then(|| blocker(ledger, r).map(|blocker| blocker.key.clone()))
                    .flatten(),
                rolled_back: r.rolled_back,
                attempts: r.attempts,
                next_due_ms: (r.state == SyncState::Sent).then_some(r.due_ms),
            })
            .collect(),
    }
}

pub(super) fn outcome(record: &SyncRecord) -> Option<CommandOutcome> {
    Some(match record.state {
        SyncState::Queued => CommandOutcome::Queued,
        SyncState::Sent => CommandOutcome::Sent,
        SyncState::Acknowledged => CommandOutcome::Acknowledged,
        SyncState::Cancelled => return None,
        _ if !record.reconciled => CommandOutcome::Acknowledged,
        _ => match record.outcome.as_ref()? {
            RemoteOutcome::Applied(value) => CommandOutcome::Applied(value.clone()),
            RemoteOutcome::Corrected(value) => CommandOutcome::Corrected(value.clone()),
            RemoteOutcome::Rejected(reason) => CommandOutcome::Rejected(reason.clone()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn queued(order: i64) -> SyncRecord {
        SyncRecord {
            entity: None,
            metadata_undo: vec![],
            key: format!("{:016x}-{}-{order:016x}", 100, "1".repeat(32)),
            command: "order-v1".into(),
            args: Json::Object(BTreeMap::from([(
                "integer".into(),
                Json::Integer(i64::MAX),
            )])),
            context: Json::Object(BTreeMap::from([
                (
                    "identity".into(),
                    Json::String("fixture-private-identity".into()),
                ),
                (
                    "capturedAt".into(),
                    Json::String("1970-01-01T00:00:00.100Z".into()),
                ),
            ])),
            schema_version: 1,
            order,
            reference_paths: vec![],
            dependencies: BTreeSet::new(),
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

    fn ledger(records: Vec<SyncRecord>) -> SyncLedger {
        let ledger = SyncLedger {
            entity: None,
            header: SyncHeader {
                namespace: "1".repeat(32),
                sequence: 20,
                last_ms: 100,
                options: SyncOptions::default(),
            },
            records: records.into_iter().map(|r| (r.key.clone(), r)).collect(),
        };
        ledger.validate().unwrap();
        ledger
    }

    #[test]
    fn c06_largest_preflight_accepted_record_can_send_retry_and_acknowledge() {
        let mut record = queued(1);
        let mut low = 0usize;
        let mut high = 1_048_576usize;
        while low < high {
            let mid = low + (high - low).div_ceil(2);
            record.args = Json::Object(BTreeMap::from([(
                "padding".into(),
                Json::String("x".repeat(mid)),
            )]));
            if record.check_send_size().is_ok() {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        record.args = Json::Object(BTreeMap::from([(
            "padding".into(),
            Json::String("x".repeat(low)),
        )]));
        record.check_send_size().unwrap();
        let key = record.key.clone();
        let (mut sent, batch) = plan_push(&ledger(vec![record]), 253_402_300_799_999, 2500)
            .expect("accepted record must retain room for real send metadata");
        assert_eq!(batch.envelopes.len(), 1);
        let record = sent.records.get_mut(&key).unwrap();
        record.attempts = u32::MAX;
        record.due_ms = i64::MAX;
        sent.to_rows().expect("retry scalar widths were reserved");
        let ack = plan_receive(&sent, &key, None).expect("receipt state width was reserved");
        assert_eq!(ack.records[&key].state, SyncState::Acknowledged);
    }

    #[test]
    fn c12_push_orders_freezes_and_blocks_only_unresolved_entries() {
        let mut a = queued(1);
        a.created.insert("local:1".into(), "orders".into());
        let mut b = queued(2);
        b.args = Json::Object(BTreeMap::from([(
            "parent".into(),
            Json::String("local:1".into()),
        )]));
        b.reference_paths = vec![vec![PathPart::Key("parent".into())]];
        b.dependencies.insert("local:1".into());
        let c = queued(3);
        let original = ledger(vec![a.clone(), b.clone(), c.clone()]);
        let (sent, batch) = plan_push(&original, 100, 0).unwrap();
        assert_eq!(
            batch.envelopes,
            vec![
                a.envelope().canonical().unwrap(),
                c.envelope().canonical().unwrap()
            ]
        );
        assert_eq!(batch.next_due_ms, Some(1100));
        assert_eq!(sent.records[&b.key].state, SyncState::Queued);
        assert_eq!(sent.records[&a.key].attempts, 1);
        assert_eq!(original.records[&a.key].state, SyncState::Queued);
        assert!(batch.envelopes[0].contains("9223372036854775807"));
        sent.validate().unwrap();
        let one = plan_push(&ledger(vec![queued(1)]), 100, 0).unwrap().1;
        assert_eq!(one.envelopes.len(), 1);
    }

    #[test]
    fn g18_status_exposes_only_persisted_attempts_and_sent_deadlines() {
        let original = ledger(vec![queued(1)]);
        assert_eq!(
            (
                status(&original).commands[0].attempts,
                status(&original).commands[0].next_due_ms
            ),
            (0, None)
        );
        let (first, _) = plan_push(&original, 100, 0).unwrap();
        assert_eq!(
            (
                status(&first).commands[0].attempts,
                status(&first).commands[0].next_due_ms
            ),
            (1, Some(1100))
        );
        let restored = SyncLedger::from_rows(&first.to_rows().unwrap()).unwrap();
        let (second, _) = plan_push(&restored, 1100, 0).unwrap();
        let key = queued(1).key;
        assert_eq!(
            (
                status(&second).commands[0].attempts,
                status(&second).commands[0].next_due_ms
            ),
            (2, Some(3100))
        );
        let acknowledged = plan_receive(&second, &key, None).unwrap();
        assert_eq!(
            (
                status(&acknowledged).commands[0].attempts,
                status(&acknowledged).commands[0].next_due_ms
            ),
            (2, None)
        );
        for state in [
            SyncState::Applied,
            SyncState::Corrected,
            SyncState::Rejected,
            SyncState::Cancelled,
        ] {
            let mut final_state = acknowledged.clone();
            final_state.records.get_mut(&key).unwrap().state = state;
            let entry = &status(&final_state).commands[0];
            assert_eq!(entry.attempts, 2);
            assert_eq!(entry.next_due_ms, None);
        }
    }

    #[test]
    fn c16_deadlines_backoff_jitter_and_restart_keep_exact_frozen_bytes() {
        let mut original = ledger(vec![queued(1)]);
        original.header.options.max_delay_ms = 4000;
        let (first, a) = plan_push(&original, 100, 2500).unwrap();
        assert_eq!(a.next_due_ms, Some(1350));
        let restored = SyncLedger::from_rows(&first.to_rows().unwrap()).unwrap();
        assert!(
            plan_push(&restored, 1349, 0)
                .unwrap()
                .1
                .envelopes
                .is_empty()
        );
        let (second, b) = plan_push(&restored, 1350, 2500).unwrap();
        assert_eq!(a.envelopes, b.envelopes);
        assert_eq!(b.next_due_ms, Some(3850));
        assert!(plan_push(&second, 1, 0).unwrap().1.envelopes.is_empty());
        let (_, c) = plan_push(&second, 3850, 2500).unwrap();
        assert_eq!(c.next_due_ms, Some(8850));
        assert_ne!(
            plan_push(&original, 100, 0).unwrap().1.next_due_ms,
            a.next_due_ms
        );
        assert!(plan_push(&original, 100, 2501).is_err());
        assert!(plan_push(&original, -1, 0).is_err());
    }

    #[test]
    fn c09_c12_receipt_stops_retry_and_outcomes_are_correlated_idempotently() {
        let (sent, _) = plan_push(&ledger(vec![queued(1), queued(2)]), 100, 0).unwrap();
        let a = queued(1).key;
        let b = queued(2).key;
        let acknowledged = plan_receive(&sent, &a, None).unwrap();
        assert_eq!(acknowledged.records[&a].state, SyncState::Acknowledged);
        let restored = SyncLedger::from_rows(&acknowledged.to_rows().unwrap()).unwrap();
        assert_eq!(
            plan_push(&restored, 1100, 0).unwrap().1.envelopes,
            vec![sent.records[&b].frozen.clone().unwrap()]
        );
        let result = RemoteOutcome::Applied(Json::Integer(42));
        let received = plan_receive(&restored, &a, Some(result.clone())).unwrap();
        assert_eq!(received.records[&a].state, SyncState::Applied);
        assert!(!received.records[&a].reconciled);
        assert_eq!(
            outcome(&received.records[&a]),
            Some(CommandOutcome::Acknowledged)
        );
        assert_eq!(plan_receive(&received, &a, Some(result)).unwrap(), received);
        assert_eq!(plan_receive(&received, &a, None).unwrap(), received);
        assert_eq!(
            plan_receive(
                &received,
                &a,
                Some(RemoteOutcome::Rejected("different".into()))
            ),
            Err(CommandError::Protocol)
        );
        assert_eq!(
            plan_receive(&received, "unknown", None),
            Err(CommandError::Protocol)
        );
    }

    #[test]
    fn c19_status_is_payload_free_and_retention_cannot_evict_live_work() {
        let mut records = vec![];
        for i in 1..=5 {
            let mut r = queued(i);
            r.state = SyncState::Cancelled;
            r.rolled_back = true;
            r.reconciled = true;
            records.push(r);
        }
        records.push(queued(6));
        let mut state = ledger(records);
        state.header.options.history = 2;
        state.header.options.capacity = 1;
        trim(&mut state);
        assert_eq!(
            state.records.values().map(|r| r.order).collect::<Vec<_>>(),
            vec![4, 5, 6]
        );
        let status = status(&state);
        assert_eq!(
            (status.pending, status.sending, status.live, status.blocked),
            (1, 0, 1, true)
        );
        assert!(!format!("{status:?}").contains("fixture-private-identity"));
        let contains_identity = |status: &QueueStatus| {
            let QueueStatus {
                pending: _,
                sending: _,
                awaiting_outcome: _,
                live: _,
                blocked: _,
                commands,
            } = status;
            commands.iter().any(|entry| {
                // Exhaustive patterns require reviewing newly added status fields.
                let QueueEntry {
                    kind: _,
                    table,
                    record_id,
                    key,
                    state: _,
                    blocked_on,
                    awaiting_local: _,
                    awaiting_key,
                    rolled_back: _,
                    attempts: _,
                    next_due_ms: _,
                } = entry;
                [Some(key.as_str()), table.as_deref(), record_id.as_deref()]
                    .into_iter()
                    .flatten()
                    .chain(blocked_on.iter().map(String::as_str))
                    .chain(awaiting_key.as_deref())
                    .any(|value| value.contains("fixture-private-identity"))
            })
        };
        assert!(
            !contains_identity(&status),
            "status values must omit identity independently of Debug"
        );
        // A redacted Debug string alone would miss each of these payload leaks.
        for field in 0..4 {
            let mut contaminated = status.clone();
            let entry = &mut contaminated.commands[0];
            match field {
                0 => entry.key = "fixture-private-identity".into(),
                1 => entry.table = Some("fixture-private-identity".into()),
                2 => entry.record_id = Some("fixture-private-identity".into()),
                _ => entry.blocked_on.push("fixture-private-identity".into()),
            }
            assert!(
                contains_identity(&contaminated),
                "negative control must detect string field {field}"
            );
            assert!(!format!("{contaminated:?}").contains("fixture-private-identity"));
        }
        state.validate().unwrap();
    }

    #[test]
    fn c15_c19_tombstones_ignore_late_results_and_retired_dependency_history_is_closed() {
        let mut a = queued(1);
        a.created.insert("local:1".into(), "orders".into());
        let mut b = queued(2);
        b.args = Json::Object(BTreeMap::from([(
            "parent".into(),
            Json::String("local:1".into()),
        )]));
        b.reference_paths = vec![vec![PathPart::Key("parent".into())]];
        b.dependencies.insert("local:1".into());
        a.state = SyncState::Cancelled;
        a.rolled_back = true;
        a.reconciled = true;
        b.state = SyncState::Cancelled;
        b.rolled_back = true;
        b.reconciled = true;
        let mut state = ledger(vec![a, b]);
        state.header.options.history = 1;
        trim(&mut state);
        state.validate().unwrap();
        assert_eq!(state.records.len(), 1);
        let (mut sent, _) = plan_push(&ledger(vec![queued(3)]), 100, 0).unwrap();
        let key = queued(3).key;
        let r = sent.records.get_mut(&key).unwrap();
        r.state = SyncState::Cancelled;
        r.rolled_back = true;
        r.reconciled = true;
        assert_eq!(
            plan_receive(&sent, &key, Some(RemoteOutcome::Applied(Json::Null))).unwrap(),
            sent
        );
        assert!(sent.has_live_recovery());
        assert_eq!(outcome(&sent.records[&key]), None);
    }

    // SPEC-0035 / TC-0039: corrected clock samples recover retries; the header
    // identity floor stays monotonic. initial 1000, max 4000, so C = 5000.
    const CAP: i64 = 5000;
    const RAW: i64 = 1_000_000;
    const FORWARD: i64 = RAW + 10 * CAP;

    fn clock_ledger(records: Vec<SyncRecord>) -> SyncLedger {
        let mut ledger = ledger(records);
        ledger.header.options.max_delay_ms = 4000;
        ledger.validate().unwrap();
        ledger
    }

    #[test]
    fn cs01_cs02_corrected_forward_jump_is_capped_then_retried_exactly_once() {
        let key = queued(1).key;
        let (sent, first) = plan_push(&clock_ledger(vec![queued(1)]), FORWARD, 0).unwrap();
        let original = sent.records[&key].clone();
        let (capped, batch) = plan_push(&sent, RAW, 0).unwrap();
        assert!(batch.envelopes.is_empty(), "no premature envelope");
        let due = batch.next_due_ms.unwrap();
        assert!(
            due <= RAW + CAP,
            "deadline {due} beyond the recovery ceiling"
        );
        let record = &capped.records[&key];
        assert_eq!(record.due_ms, RAW + CAP);
        assert_eq!(
            (&record.state, record.attempts, &record.frozen, &record.key),
            (
                &original.state,
                original.attempts,
                &original.frozen,
                &original.key
            )
        );
        let mut state = capped;
        for sample in [RAW + 1, RAW + CAP - 1] {
            let (next, batch) = plan_push(&state, sample, 0).unwrap();
            assert!(batch.envelopes.is_empty());
            assert_eq!(
                next.records[&key].due_ms,
                RAW + CAP,
                "no extension at {sample}"
            );
            state = next;
        }
        let (retried, batch) = plan_push(&state, RAW + CAP, 0).unwrap();
        assert_eq!(batch.envelopes, first.envelopes);
        assert_eq!(retried.records[&key].attempts, original.attempts + 1);
    }

    #[test]
    fn cs04_deadline_inside_recovery_ceiling_is_never_shortened() {
        // Characterization: holds before and after the correction.
        let key = queued(1).key;
        let (sent, _) = plan_push(&clock_ledger(vec![queued(1)]), 100, 2500).unwrap();
        let due = sent.records[&key].due_ms;
        for sample in [0, 100, 101, due - 1] {
            let (next, batch) = plan_push(&sent, sample, 0).unwrap();
            assert!(batch.envelopes.is_empty());
            assert_eq!(next.records[&key].due_ms, due);
        }
    }

    #[test]
    fn cs05_ceiling_saturates_and_unrepresentable_deadlines_fail_unchanged() {
        let key = queued(1).key;
        let mut huge = clock_ledger(vec![queued(1)]);
        huge.header.options.initial_delay_ms = 1;
        huge.header.options.max_delay_ms = i64::MAX;
        let (sent, _) = plan_push(&huge, 100, 0).unwrap();
        let top = 253_402_300_799_999;
        let (kept, _) = plan_push(&sent, top, 0).unwrap();
        assert_eq!(
            kept.records[&key].attempts, 2,
            "due retry at the top sample"
        );
        let mut overflow = clock_ledger(vec![queued(1)]);
        overflow.header.options.initial_delay_ms = i64::MAX;
        overflow.header.options.max_delay_ms = i64::MAX;
        assert_eq!(
            plan_push(&overflow, top, 2500).err(),
            Some(CommandError::LimitExceeded)
        );
        assert!(plan_push(&overflow, top + 1, 0).is_err());
        assert!(plan_push(&overflow, -1, 0).is_err());
        assert!(plan_push(&overflow, 100, 2501).is_err());
    }

    #[test]
    fn cs06_cs08_only_sent_deadlines_cap_and_queued_work_uses_the_raw_sample() {
        let keys = (1..=5).map(|order| queued(order).key).collect::<Vec<_>>();
        let (sent, _) =
            plan_push(&clock_ledger((1..=3).map(queued).collect()), FORWARD, 0).unwrap();
        let acknowledged = plan_receive(&sent, &keys[1], None).unwrap();
        let mut terminal = plan_receive(
            &acknowledged,
            &keys[2],
            Some(RemoteOutcome::Applied(Json::Null)),
        )
        .unwrap();
        for order in [4, 5] {
            let record = queued(order);
            terminal.records.insert(record.key.clone(), record);
        }
        terminal.validate().unwrap();
        let (next, batch) = plan_push(&terminal, RAW, 0).unwrap();
        assert_eq!(
            batch.envelopes,
            vec![
                next.records[&keys[3]].frozen.clone().unwrap(),
                next.records[&keys[4]].frozen.clone().unwrap()
            ],
            "exactly the Queued records 4 and 5 are sent"
        );
        assert_eq!(
            next.records[&keys[0]].due_ms,
            RAW + CAP,
            "Sent deadline capped"
        );
        assert_eq!(
            next.records[&keys[1]].due_ms, sent.records[&keys[1]].due_ms,
            "Acknowledged untouched"
        );
        assert_eq!(next.records[&keys[1]].state, SyncState::Acknowledged);
        assert_eq!(
            next.records[&keys[2]].due_ms, sent.records[&keys[2]].due_ms,
            "terminal untouched"
        );
        for key in &keys[3..] {
            assert_eq!(
                next.records[key].due_ms,
                RAW + 1000,
                "raw sample schedules new work"
            );
        }
        assert_eq!(
            next.header.last_ms, FORWARD,
            "identity floor never decreases"
        );
    }

    #[test]
    fn cs09_backward_sample_does_not_use_the_identity_floor_as_elapsed_time() {
        let key = queued(1).key;
        let (mut sent, first) = plan_push(&clock_ledger(vec![queued(1)]), 100_000, 0).unwrap();
        assert_eq!(sent.records[&key].due_ms, 101_000);
        sent.header.last_ms = 101_500;
        let (waiting, batch) = plan_push(&sent, 100_500, 0).unwrap();
        assert!(
            batch.envelopes.is_empty(),
            "raw sample has not reached the deadline"
        );
        assert_eq!(waiting.records[&key].due_ms, 101_000);
        let (_, batch) = plan_push(&waiting, 101_000, 0).unwrap();
        assert_eq!(batch.envelopes, first.envelopes);
    }

    // SPEC-0038 C2 / TC-0042 RD: row-causal dispatch withholds a later Queued record
    // while an earlier unresolved record wrote any of the same rows.
    fn touching(order: i64, id: &str) -> SyncRecord {
        let mut record = queued(order);
        let row = crate::storage::Row {
            id: id.into(),
            fields: BTreeMap::new(),
        };
        record.undo = vec![crate::storage::CommittedChange {
            table: "items".into(),
            id: id.into(),
            before: Some(row.clone()),
            after: Some(row),
        }];
        record
    }

    fn sent_keys(batch: &PushBatch) -> Vec<String> {
        batch
            .envelopes
            .iter()
            .map(|envelope| {
                let Json::Object(wire) = Json::parse(envelope).unwrap() else {
                    panic!("object envelope")
                };
                let Json::String(key) = &wire["idempotencyKey"] else {
                    panic!("command key")
                };
                key.clone()
            })
            .collect()
    }

    #[test]
    fn rd1_later_same_row_queued_record_is_withheld_and_unrelated_work_is_sent() {
        let (a, b, c) = (touching(1, "r"), touching(2, "r"), touching(3, "s"));
        let (next, batch) =
            plan_push(&ledger(vec![a.clone(), b.clone(), c.clone()]), 100, 0).unwrap();
        assert_eq!(sent_keys(&batch), vec![a.key.clone(), c.key.clone()]);
        assert_eq!(next.records[&b.key].state, SyncState::Queued);
        // Applied but unreconciled still withholds; reconciled releases.
        let applied =
            plan_receive(&next, &a.key, Some(RemoteOutcome::Applied(Json::Null))).unwrap();
        assert!(sent_keys(&plan_push(&applied, 200, 0).unwrap().1).is_empty());
        let mut reconciled = applied.clone();
        reconciled.records.get_mut(&a.key).unwrap().reconciled = true;
        reconciled.records.get_mut(&a.key).unwrap().undo.clear();
        assert_eq!(
            sent_keys(&plan_push(&reconciled, 200, 0).unwrap().1),
            vec![b.key.clone()]
        );
        // A processed rejection that is not rolled back still withholds.
        let mut rejected =
            plan_receive(&next, &a.key, Some(RemoteOutcome::Rejected("no".into()))).unwrap();
        rejected.records.get_mut(&a.key).unwrap().reconciled = true;
        assert!(sent_keys(&plan_push(&rejected, 200, 0).unwrap().1).is_empty());
    }

    #[test]
    fn rd_sent_records_are_never_withheld_and_cancelled_records_never_block() {
        let (a, b) = (touching(1, "r"), touching(2, "r"));
        let mut legacy = ledger(vec![a.clone(), b.clone()]);
        for key in [&a.key, &b.key] {
            let record = legacy.records.get_mut(key).unwrap();
            record.frozen = Some(record.envelope().canonical().unwrap());
            record.state = SyncState::Sent;
            record.attempts = 1;
        }
        legacy.validate().unwrap();
        assert_eq!(
            sent_keys(&plan_push(&legacy, 100, 0).unwrap().1),
            vec![a.key.clone(), b.key.clone()],
            "pre-upgrade Sent pairs keep retrying"
        );
        let mut cancelled = ledger(vec![a.clone(), b.clone()]);
        let record = cancelled.records.get_mut(&a.key).unwrap();
        record.state = SyncState::Cancelled;
        record.rolled_back = true;
        record.reconciled = true;
        record.undo.clear();
        cancelled.validate().unwrap();
        assert_eq!(
            sent_keys(&plan_push(&cancelled, 100, 0).unwrap().1),
            vec![b.key.clone()]
        );
    }
}
