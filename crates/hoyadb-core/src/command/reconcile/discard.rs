//! SPEC-0040: validate the whole dependent closure before retiring dispatch bookkeeping.
use super::*;
use crate::command::Resolution;

fn eligible(record: &SyncRecord) -> bool {
    match record.state {
        SyncState::Queued => record.frozen.is_none() && record.attempts == 0,
        SyncState::Rejected => record.reconciled && !record.rolled_back,
        // Never sent, or sent and finally rejected before a conflicted cancel retained it:
        // like a processed rejection, its stored terminal outcome rules out later execution.
        SyncState::Cancelled => {
            !record.rolled_back
                && ((record.frozen.is_none() && record.attempts == 0)
                    || matches!(record.outcome, Some(RemoteOutcome::Rejected(_))))
        }
        _ => false,
    }
}

/// Correction keeps its accepted root; all other still-pending members must be discardable.
pub(super) fn check(
    ledger: &SyncLedger,
    keys: &BTreeSet<String>,
    accepted: Option<&str>,
) -> Result<(), CommandError> {
    let mut affected = BTreeMap::new();
    for key in keys {
        let record = &ledger.records[key];
        if record.rolled_back || record.state == SyncState::Discarded {
            continue;
        }
        if record.entity.is_some()
            || !record.metadata_undo.is_empty()
            || record.undo.iter().any(|change| {
                ledger
                    .entity
                    .as_ref()
                    .is_some_and(|e| e.config.tables.contains(&change.table))
            })
            || (accepted != Some(key.as_str()) && !eligible(record))
        {
            return Err(CommandError::RollbackConflict);
        }
        for change in &record.undo {
            let earliest = affected
                .entry((change.table.as_str(), change.id.as_str()))
                .or_insert(record.order);
            *earliest = (*earliest).min(record.order);
        }
    }
    if ledger.records.values().any(|record| {
        !keys.contains(&record.key)
            && record.is_live()
            && record.undo.iter().any(|change| {
                affected
                    .get(&(change.table.as_str(), change.id.as_str()))
                    .is_some_and(|order| record.order > *order)
            })
    }) {
        return Err(CommandError::RollbackConflict);
    }
    Ok(())
}

pub(in crate::command) fn plan_in(
    ledger: &mut LedgerTxn<'_>,
    rows: &Snapshot,
    schema: &SchemaDefinition,
    key: &str,
) -> Result<RowPlan, CommandError> {
    ledger.validate()?;
    validate_snapshot(rows, schema)?;
    let root = ledger.records.get(key).ok_or(CommandError::Protocol)?;
    if !eligible(root) {
        return Err(CommandError::Protocol);
    }
    let keys = closure(ledger, key)?;
    check(ledger, &keys, None)?;
    let mut discarded_keys = Vec::new();
    let mut unmapped = BTreeSet::new();
    for key in keys {
        let record = ledger.record_mut(&key).expect("known closure member");
        if record.rolled_back || record.state == SyncState::Discarded {
            continue;
        }
        unmapped.extend(
            record
                .created
                .keys()
                .filter(|id| !record.mappings.contains_key(*id))
                .cloned(),
        );
        record.state = SyncState::Discarded;
        record.reconciled = true;
        record.rolled_back = false;
        record.reference_paths.clear();
        record.dependencies.clear();
        discarded_keys.push(key);
    }
    let resolution = Resolution::KeptCurrent {
        discarded_keys,
        unmapped: unmapped.into_iter().collect(),
        skipped_writes: 0,
    };
    // This early check precedes finish's trim/encoding work. The owner's shared
    // apply path checks again so unit and reporting callers have one commit boundary.
    resolution.preflight()?;
    let mut planned = finish_in(ledger, super::clone_snapshot(rows), schema, false)?;
    planned.resolution = resolution;
    Ok(planned)
}
