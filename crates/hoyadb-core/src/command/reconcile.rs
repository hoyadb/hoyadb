//! API-0006 section 5: pure authoritative correction and guarded inverse planning.
use super::{
    CommandError,
    ledger_txn::LedgerTxn,
    queue::{CorrectionGuard, ReconcilePlan, ReconcileWrite},
    records::*,
};
use crate::{model::*, storage::*};
use std::collections::{BTreeMap, BTreeSet};

mod discard;
mod fallback;
pub(super) use discard::plan_in as discard_in;

pub(super) type Snapshot = BTreeMap<String, BTreeMap<String, Row>>;
pub(super) fn clone_snapshot(rows: &Snapshot) -> Snapshot {
    crate::storage::count_work!(
        rows_cloned,
        rows.values().map(BTreeMap::len).sum::<usize>() as u64
    );
    rows.clone()
}

fn clone_row(row: &Row) -> Row {
    crate::storage::count_work!(rows_cloned, 1);
    row.clone()
}

type MetadataSnapshot = BTreeMap<(String, String), Option<super::entity::RowMetadata>>;
/// The owner's registered merge hooks; C4 only checks membership (E4) and never runs one.
pub(super) type Hooks = BTreeMap<String, super::entity::MergeHook>;

pub(super) struct RowPlan {
    pub(super) resolution: super::Resolution,
    pub(super) rows: Snapshot,
    pub(super) conflict: bool,
    pub(super) outcomes: Vec<(String, super::CommandOutcome)>,
}

#[cfg(test)]
pub(super) struct Planned {
    pub(super) ledger: SyncLedger,
    pub(super) rows: Snapshot,
    pub(super) conflict: bool,
}

#[cfg(test)]
pub(super) fn reconcile(
    ledger: &SyncLedger,
    rows: &Snapshot,
    schema: &SchemaDefinition,
    key: &str,
    plan: ReconcilePlan,
) -> Result<Planned, CommandError> {
    reconcile_with_hooks(
        ledger,
        rows,
        schema,
        key,
        plan,
        vec![],
        &Hooks::new(),
        &super::ConflictPolicy::default(),
    )
    .map(|(planned, _)| planned)
}

#[cfg(test)]
pub(super) fn inverse(
    ledger: &SyncLedger,
    rows: &Snapshot,
    schema: &SchemaDefinition,
    key: &str,
    cancel: bool,
) -> Result<Planned, CommandError> {
    inverse_with_hooks(
        ledger,
        rows,
        schema,
        key,
        cancel,
        &Hooks::new(),
        &super::ConflictPolicy::default(),
    )
}

#[cfg(test)]
fn owned(
    ledger: &SyncLedger,
    plan: impl FnOnce(&mut LedgerTxn<'_>) -> Result<RowPlan, CommandError>,
) -> Result<Planned, CommandError> {
    let mut next = ledger.clone();
    let mut txn = LedgerTxn::new(&mut next, false);
    let plan = plan(&mut txn)?;
    txn.commit();
    Ok(Planned {
        ledger: next,
        rows: plan.rows,
        conflict: plan.conflict,
    })
}
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn reconcile_with_hooks(
    ledger: &SyncLedger,
    rows: &Snapshot,
    schema: &SchemaDefinition,
    key: &str,
    plan: ReconcilePlan,
    guards: Vec<CorrectionGuard>,
    hooks: &Hooks,
    policy: &super::ConflictPolicy,
) -> Result<(Planned, Vec<RowGuard>), CommandError> {
    let mut expectations = vec![];
    let planned = owned(ledger, |txn| {
        let (planned, guards) = reconcile_in(txn, rows, schema, key, plan, guards, hooks, policy)?;
        expectations = guards;
        Ok(planned)
    })?;
    Ok((planned, expectations))
}
#[cfg(test)]
pub(super) fn inverse_with_hooks(
    ledger: &SyncLedger,
    rows: &Snapshot,
    schema: &SchemaDefinition,
    key: &str,
    cancel: bool,
    hooks: &Hooks,
    policy: &super::ConflictPolicy,
) -> Result<Planned, CommandError> {
    owned(ledger, |txn| {
        inverse_in(txn, rows, schema, key, cancel, hooks, policy)
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn reconcile_in(
    ledger: &mut LedgerTxn<'_>,
    rows: &Snapshot,
    schema: &SchemaDefinition,
    key: &str,
    plan: ReconcilePlan,
    guards: Vec<CorrectionGuard>,
    hooks: &Hooks,
    policy: &super::ConflictPolicy,
) -> Result<(RowPlan, Vec<RowGuard>), CommandError> {
    ledger.validate()?;
    let projection = validate_snapshot(rows, schema)?;
    let record = ledger
        .records
        .get(key)
        .cloned()
        .ok_or(CommandError::Protocol)?;
    if !matches!(
        record.state,
        SyncState::Applied | SyncState::Corrected | SyncState::Rejected
    ) {
        return Err(CommandError::Protocol);
    }
    if record.reconciled {
        return if plan.writes.is_empty() && plan.mappings == record.mappings && guards.is_empty() {
            Ok((
                RowPlan {
                    resolution: super::Resolution::Completed,
                    rows: clone_snapshot(rows),
                    conflict: false,
                    outcomes: vec![],
                },
                vec![],
            ))
        } else {
            Err(CommandError::Protocol)
        };
    }
    if record.schema_version != schema.version {
        return Err(CommandError::Protocol);
    }
    if matches!(record.outcome, Some(RemoteOutcome::Rejected(_))) {
        if !plan.writes.is_empty() || !plan.mappings.is_empty() || !guards.is_empty() {
            return Err(CommandError::InvalidCommand);
        }
        ledger.record_mut(key).expect("known rejection").reconciled = true;
        return finish_in(ledger, clone_snapshot(rows), schema, false)
            .map(|planned| (planned, vec![]));
    }
    if !matches!(
        record.outcome,
        Some(RemoteOutcome::Applied(_) | RemoteOutcome::Corrected(_))
    ) {
        return Err(CommandError::Protocol);
    }
    if !plan.mappings.keys().eq(record.created.keys()) {
        return Err(CommandError::InvalidCommand);
    }
    let mut mappings = BTreeMap::new();
    let mut targets = BTreeSet::new();
    for (local, remote) in &plan.mappings {
        let table = &record.created[local];
        if remote.is_empty()
            || remote.starts_with("local:")
            || !targets.insert((table.clone(), remote.clone()))
            || rows
                .get(table)
                .is_none_or(|table| table.contains_key(remote) && local != remote)
            || ledger.records.values().any(|other| {
                other.key != key
                    && other.mappings.iter().any(|(id, existing)| {
                        other.created.get(id) == Some(table) && existing == remote
                    })
            })
        {
            return Err(CommandError::InvalidCommand);
        }
        mappings.insert(local.clone(), (table.clone(), remote.clone()));
    }
    if record.entity.is_some()
        && plan
            .writes
            .iter()
            .any(|write| !matches!(write, ReconcileWrite::Entity { .. }))
    {
        return Err(CommandError::InvalidCommand);
    }
    let typed = plan
        .writes
        .iter()
        .filter_map(|write| match write {
            ReconcileWrite::Entity {
                change,
            } => Some(change.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !typed.is_empty() {
        if ledger.entity.is_none() {
            return Err(CommandError::InvalidCommand);
        }
        super::PullPage {
            checkpoint: String::new(),
            complete: true,
            changes: typed,
        }
        .to_json()?;
    }
    let closure = closure(ledger, key)?;
    let expectations = guard_outside_closure(
        ledger,
        rows,
        &projection,
        schema.version,
        &closure,
        &mappings,
        &plan,
        guards,
    )?;
    // Prove the entire optimistic closure against current data without applying
    // its inverse. Known dependent writes are accounted for; unrelated edits are not.
    // SPEC-0038 C1: a confirmation without writes changes no cell (only identities,
    // references, undo identities and queued arguments), so it cannot overwrite a later
    // local edit and needs no inverse proof. A plan with writes keeps the full proof.
    if !plan.writes.is_empty() {
        let proof = simulate_inverse(ledger, rows, &projection, schema.version, &closure)
            .and_then(|_| simulate_metadata_inverse(ledger, &closure));
        if let Err(CommandError::RollbackConflict) = proof {
            // SPEC-0038 C4: only this closure-proof mismatch may fall back; any failed C4
            // condition keeps the original RollbackConflict with nothing changed.
            let changes = plan
                .writes
                .iter()
                .filter_map(|write| match write {
                    ReconcileWrite::Entity {
                        change,
                    } => Some(change.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let path = fallback::Path::Correction(&changes);
            let Some(result) = fallback::rebase_in(ledger, rows, schema, key, path, hooks) else {
                if policy.reaction(&record.command) != super::ConflictReaction::Discard {
                    return Err(CommandError::RollbackConflict);
                }
                discard::check(ledger, &closure, Some(key))?;
                let skipped_writes = plan.writes.len();
                // Reuse ordinary empty-plan confirmation and global identity rewriting.
                // Guards were validated above and remain attached to this atomic commit.
                let (mut planned, _) = reconcile_in(
                    ledger,
                    rows,
                    schema,
                    key,
                    ReconcilePlan {
                        writes: vec![],
                        mappings: plan.mappings,
                    },
                    vec![],
                    hooks,
                    policy,
                )?;
                planned.resolution = super::Resolution::KeptCurrent {
                    discarded_keys: vec![],
                    unmapped: vec![],
                    skipped_writes,
                };
                return Ok((planned, expectations));
            };
            let producer = ledger.record_mut(key).expect("known outcome");
            producer.mappings = plan.mappings;
            producer.reconciled = true;
            return Ok((settle(ledger, result, schema, key)?, expectations));
        }
        proof?;
    }
    let mut result = clone_snapshot(rows);
    for (local, (table, remote)) in &mappings {
        if local != remote && ledger.entity.is_some() {
            let from = (table.clone(), local.clone());
            let to = (table.clone(), remote.clone());
            // The occupied target and the moved source entry.
            crate::storage::count_work!(metadata_entries_visited, 2);
            if ledger
                .entity
                .as_ref()
                .is_some_and(|entity| entity.metadata.contains_key(&to))
            {
                return Err(CommandError::InvalidCommand);
            }
            if let Some(metadata) = ledger.remove_metadata(&from) {
                ledger.put_metadata(to, metadata)?;
            }
        }
        let table_rows = result.get_mut(table).ok_or(CommandError::InvalidCommand)?;
        if let Some(mut row) = table_rows.remove(local) {
            row.id = remote.clone();
            table_rows.insert(remote.clone(), row);
        }
    }
    // SPEC-0038 C1a: explicitly corrected fields keep their selected stamps.
    let mut corrected = BTreeSet::new();
    for write in &plan.writes {
        if let ReconcileWrite::Entity {
            change,
        } = write
        {
            let identity = (change.table.clone(), change.record_id.clone());
            for name in change.changes.keys().chain(&change.removed_fields) {
                corrected.insert((identity.clone(), Some(name.clone())));
            }
            if change.op == super::EntityOperation::Delete {
                corrected.insert((identity, None));
            }
        }
    }
    for write in plan.writes {
        match write {
            ReconcileWrite::Entity {
                change,
            } => {
                if mappings
                    .get(&change.record_id)
                    .is_some_and(|(owner, remote)| {
                        owner == &change.table && remote != &change.record_id
                    })
                {
                    return Err(CommandError::InvalidCommand);
                }
                apply_entity_correction(ledger, &mut result, schema, &change)?;
            }
            ReconcileWrite::Put {
                table,
                id,
                fields,
            } => {
                if mappings
                    .get(&id)
                    .is_some_and(|(owner, remote)| owner == &table && remote != &id)
                    || fields
                        .keys()
                        .any(|name| name == "id" || name.starts_with(RESERVED_PREFIX))
                {
                    return Err(CommandError::InvalidCommand);
                }
                let row = Row {
                    id: id.clone(),
                    fields,
                };
                validate_public_row(&projection, schema.version, &table, &row)?;
                result
                    .get_mut(&table)
                    .ok_or(CommandError::InvalidCommand)?
                    .insert(id, row);
            }
            ReconcileWrite::Remove {
                table,
                id,
            } => {
                if id.is_empty()
                    || mappings
                        .get(&id)
                        .is_some_and(|(owner, remote)| owner == &table && remote != &id)
                {
                    return Err(CommandError::InvalidCommand);
                }
                result
                    .get_mut(&table)
                    .ok_or(CommandError::InvalidCommand)?
                    .remove(&id);
            }
        }
    }
    for (table, rows) in &mut result {
        for row in rows.values_mut() {
            rewrite_row(&projection, table, row, &mappings, false);
        }
    }
    {
        let producer = ledger.record_mut(key).expect("known outcome");
        producer.mappings = plan.mappings;
        producer.reconciled = true;
    }
    if record.entity.is_some() {
        for (local, (table, remote)) in &mappings {
            ledger.put_alias(table.clone(), local.clone(), remote.clone())?;
        }
    }
    let record_keys = ledger.records.keys().cloned().collect::<Vec<_>>();
    for record_key in record_keys {
        let mut record = ledger.records[&record_key].clone();
        for change in &mut record.metadata_undo {
            if let Some((owner, remote)) = mappings.get(&change.record_id)
                && owner == &change.table
            {
                change.record_id = remote.clone();
            }
        }
        for change in &mut record.undo {
            if let Some((owner, remote)) = mappings.get(&change.id)
                && owner == &change.table
            {
                change.id = remote.clone();
            }
            for row in change.before.iter_mut().chain(change.after.iter_mut()) {
                rewrite_row(&projection, &change.table, row, &mappings, true);
            }
        }
        if record != ledger.records[&record_key] {
            ledger.insert_record(record_key, record);
        }
    }
    // Compare confirmation against canonical identities after explicit correction.
    // Selected backend clocks no longer match the superseded optimistic image.
    let accepted = ledger.records[key].clone();
    promote_metadata(ledger, &accepted, &result, &corrected);
    Ok((settle(ledger, result, schema, key)?, expectations))
}

/// The accepted producer's last step on every correction path: drop its recovery images,
/// renormalize queued arguments against the new identities, then finish.
fn settle(
    ledger: &mut LedgerTxn<'_>,
    result: Snapshot,
    schema: &SchemaDefinition,
    key: &str,
) -> Result<RowPlan, CommandError> {
    let producer = ledger.record_mut(key).expect("known outcome");
    producer.undo.clear();
    producer.metadata_undo.clear();
    // A never-sent Cancelled record retained by a conflicted cancel still names local
    // identities in its arguments; it is renormalized like queued work so the accepted
    // mapping cannot leave a reference to a mapped-away identity. It is never sent.
    let unsent = ledger
        .records
        .values()
        .filter(|record| {
            record.state == SyncState::Queued
                || (record.state == SyncState::Cancelled
                    && record.frozen.is_none()
                    && !record.reference_paths.is_empty())
        })
        .map(|record| record.key.clone())
        .collect::<Vec<_>>();
    for key in unsent {
        let record = &ledger.records[&key];
        let mut args = record.args.clone();
        let dependencies =
            super::owner::normalize_references(&mut args, &record.reference_paths, ledger)?;
        let record = ledger.record_mut(&key).expect("unsent candidate");
        record.args = args;
        record.dependencies = dependencies;
        if record.state == SyncState::Queued {
            record
                .check_send_size()
                .map_err(|_| CommandError::LimitExceeded)?;
        }
    }
    finish_in(ledger, result, schema, false)
}

/// SPEC-0039: classify every plan write against the command closure's written identities
/// (undo, comparison undo and explicit mapping targets). Validation order: write shapes and
/// guard shapes (InvalidCommand) before any data conflict (RollbackConflict). Outside the
/// closure, a Put/Remove needs one matching explicit expectation; typed entity writes, writes
/// to entity-managed tables and rows in another live record's undo are refused regardless.
#[allow(clippy::too_many_arguments)]
fn guard_outside_closure(
    ledger: &SyncLedger,
    rows: &Snapshot,
    projection: &Schema,
    version: u32,
    closure: &BTreeSet<String>,
    mappings: &BTreeMap<String, (String, String)>,
    plan: &ReconcilePlan,
    guards: Vec<CorrectionGuard>,
) -> Result<Vec<RowGuard>, CommandError> {
    let mut owned = mappings.values().cloned().collect::<BTreeSet<_>>();
    for key in closure {
        let record = &ledger.records[key];
        owned.extend(record.undo.iter().map(|c| (c.table.clone(), c.id.clone())));
        owned.extend(
            record
                .metadata_undo
                .iter()
                .map(|c| (c.table.clone(), c.record_id.clone())),
        );
    }
    let mut written = vec![];
    for write in &plan.writes {
        let (table, id, typed) = match write {
            ReconcileWrite::Entity {
                change,
            } => (&change.table, &change.record_id, true),
            ReconcileWrite::Put {
                table,
                id,
                fields,
            } => {
                validate_public_row(
                    projection,
                    version,
                    table,
                    &Row {
                        id: id.clone(),
                        fields: fields.clone(),
                    },
                )?;
                (table, id, false)
            }
            ReconcileWrite::Remove {
                table,
                id,
            } => (table, id, false),
        };
        if id.is_empty()
            || !rows.contains_key(table)
            || mappings
                .get(id)
                .is_some_and(|(owner, remote)| owner == table && remote != id)
        {
            return Err(CommandError::InvalidCommand);
        }
        written.push(((table.clone(), id.clone()), typed));
    }
    let mut expected = BTreeMap::new();
    for guard in guards {
        let identity = (guard.table.clone(), guard.id.clone());
        let row = guard.expected.map(|fields| Row {
            id: guard.id.clone(),
            fields,
        });
        if guard.id.is_empty() || !rows.contains_key(&guard.table) {
            return Err(CommandError::InvalidCommand);
        }
        // A malformed expectation is a malformed guard (InvalidCommand), not a storage error.
        if let Some(row) = &row {
            validate_public_row(projection, version, &guard.table, row)
                .map_err(|_| CommandError::InvalidCommand)?;
        }
        // A guard is used only by a plan write outside the closure; one per identity.
        if owned.contains(&identity)
            || !written.iter().any(|(written, _)| written == &identity)
            || expected.insert(identity, row).is_some()
        {
            return Err(CommandError::InvalidCommand);
        }
    }
    let entity_tables = ledger
        .entity
        .as_ref()
        .map(|entity| entity.config.tables.clone())
        .unwrap_or_default();
    // SPEC-0039 step 4: only a live record outside the closure protects its undo rows.
    let live_keys = ledger.live_key_refs();
    for ((table, id), typed) in &written {
        let identity = (table.clone(), id.clone());
        if owned.contains(&identity) {
            continue;
        }
        let live = ledger.records.values().any(|record| {
            !closure.contains(&record.key)
                && live_keys.contains(record.key.as_str())
                && record.undo.iter().any(|c| &c.table == table && &c.id == id)
        });
        if *typed || entity_tables.contains(table) || live {
            return Err(CommandError::RollbackConflict);
        }
        match expected.get(&identity) {
            Some(row) if rows[table].get(id) == row.as_ref() => {}
            _ => return Err(CommandError::RollbackConflict),
        }
    }
    Ok(expected
        .into_iter()
        .map(|((table, id), expected)| RowGuard {
            table,
            id,
            expected,
        })
        .collect())
}

fn apply_entity_correction(
    ledger: &mut LedgerTxn<'_>,
    rows: &mut Snapshot,
    schema: &SchemaDefinition,
    change: &super::entity::EntityChange,
) -> Result<(), CommandError> {
    let entity = ledger.entity.as_ref().ok_or(CommandError::InvalidCommand)?;
    let table = rows
        .get_mut(&change.table)
        .ok_or(CommandError::InvalidCommand)?;
    let identity = (change.table.clone(), change.record_id.clone());
    crate::storage::count_work!(metadata_entries_visited, 1);
    let baseline = entity.metadata.get(&identity).cloned().unwrap_or_default();
    let resolved = corrected_image(schema, table.get(&change.record_id), baseline, change)?;
    match resolved.row {
        Some(row) => {
            table.insert(row.id.clone(), row);
        }
        None => {
            table.remove(&change.record_id);
        }
    }
    ledger.put_metadata(identity, resolved.metadata)?;
    Ok(())
}

/// One typed correction applied to a row image and its metadata, with the backend's exact
/// selected values and clocks (API-0009).
fn corrected_image(
    schema: &SchemaDefinition,
    current: Option<&Row>,
    mut baseline: super::entity::RowMetadata,
    change: &super::entity::EntityChange,
) -> Result<super::entity::EntityResolution, CommandError> {
    use super::entity::{EntityOperation, resolve_entity};
    if change.op != EntityOperation::Delete
        && baseline
            .tombstone
            .as_ref()
            .is_some_and(|stamp| stamp.authoritative)
    {
        return Err(CommandError::InvalidCommand);
    }
    // The backend has already selected these values/clocks. Reuse the schema,
    // completeness and bounds checks while removing only their client clocks.
    baseline.tombstone = None;
    for name in change.changes.keys() {
        baseline.fields.remove(name);
    }
    resolve_entity(schema, current, &baseline, change, true, None)
}

pub(super) fn inverse_in(
    ledger: &mut LedgerTxn<'_>,
    rows: &Snapshot,
    schema: &SchemaDefinition,
    key: &str,
    cancel: bool,
    hooks: &Hooks,
    policy: &super::ConflictPolicy,
) -> Result<RowPlan, CommandError> {
    ledger.validate()?;
    let projection = validate_snapshot(rows, schema)?;
    let root = ledger
        .records
        .get(key)
        .cloned()
        .ok_or(CommandError::Protocol)?;
    if cancel {
        if !matches!(
            root.state,
            SyncState::Queued | SyncState::Sent | SyncState::Rejected | SyncState::Cancelled
        ) {
            return Err(CommandError::Protocol);
        }
    } else if root.state != SyncState::Cancelled
        && !(root.state == SyncState::Rejected && root.reconciled)
    {
        return Err(CommandError::Protocol);
    }
    if root.rolled_back {
        return finish_in(ledger, clone_snapshot(rows), schema, false);
    }
    let closure = closure(ledger, key)?;
    let simulated =
        simulate_inverse(ledger, rows, &projection, schema.version, &closure).and_then(|rows| {
            simulate_metadata_inverse(ledger, &closure).map(|metadata| (rows, metadata))
        });
    let (result, metadata, conflict) = match simulated {
        Ok((result, metadata)) => (result, metadata, false),
        Err(CommandError::RollbackConflict) if cancel => (clone_snapshot(rows), None, true),
        Err(CommandError::RollbackConflict) => {
            // SPEC-0038 C4 (rollback only, never cancel): a processed rejection whose closure
            // proof mismatched may fall back; otherwise the original RollbackConflict stands.
            let path = fallback::Path::Rollback;
            let Some(result) = fallback::rebase_in(ledger, rows, schema, key, path, hooks) else {
                if root.state == SyncState::Rejected
                    && policy.reaction(&root.command) == super::ConflictReaction::Discard
                {
                    return discard::plan_in(ledger, rows, schema, key);
                }
                return Err(CommandError::RollbackConflict);
            };
            let record = ledger.record_mut(key).expect("C4 root");
            record.reconciled = true;
            record.rolled_back = true;
            record.undo.clear();
            record.metadata_undo.clear();
            return finish_in(ledger, result, schema, false);
        }
        Err(error) => return Err(error),
    };
    if let Some(metadata) = metadata {
        for (identity, value) in metadata {
            if let Some(value) = value {
                ledger.put_metadata(identity, value)?;
            } else {
                ledger.remove_metadata(&identity);
            }
        }
    }
    for member in closure {
        let record = ledger.record_mut(&member).expect("closure member");
        if cancel || member != key || record.state == SyncState::Cancelled {
            record.state = SyncState::Cancelled;
        }
        record.reconciled = true;
        if !conflict {
            record.rolled_back = true;
            record.undo.clear();
            record.metadata_undo.clear();
        }
    }
    finish_in(ledger, result, schema, conflict)
}

fn finish_in(
    ledger: &mut LedgerTxn<'_>,
    rows: Snapshot,
    schema: &SchemaDefinition,
    conflict: bool,
) -> Result<RowPlan, CommandError> {
    let outcomes = ledger
        .records
        .values()
        .filter_map(|record| {
            let value = super::queue::outcome(record)?;
            let prior = ledger
                .prior_record(&record.key)
                .and_then(super::queue::outcome);
            (prior.as_ref() != Some(&value)).then(|| (record.key.clone(), value))
        })
        .collect();
    super::queue::trim_in(ledger);
    ledger.serialization_check()?;
    validate_snapshot(&rows, schema)?;
    Ok(RowPlan {
        resolution: super::Resolution::Completed,
        rows,
        conflict,
        outcomes,
    })
}

fn validate_snapshot(rows: &Snapshot, schema: &SchemaDefinition) -> Result<Schema, CommandError> {
    let projection = schema.storage_schema()?;
    if !rows.keys().eq(projection.tables.keys()) {
        return Err(CommandError::InvalidCommand);
    }
    for (table, contents) in rows {
        for (id, row) in contents {
            if id != &row.id {
                return Err(CommandError::InvalidCommand);
            }
            validate_public_row(&projection, schema.version, table, row)?;
        }
    }
    Ok(projection)
}

fn validate_public_row(
    schema: &Schema,
    version: u32,
    table: &str,
    row: &Row,
) -> Result<(), CommandError> {
    if row.id.is_empty()
        || row
            .fields
            .keys()
            .any(|name| name == "id" || name.starts_with(RESERVED_PREFIX))
    {
        return Err(CommandError::InvalidCommand);
    }
    let mut raw = clone_row(row);
    raw.fields
        .insert(ROW_VERSION.into(), Value::Integer(i64::from(version)));
    validate_row(schema, table, &raw)?;
    Ok(())
}

pub(super) fn closure(ledger: &SyncLedger, key: &str) -> Result<BTreeSet<String>, CommandError> {
    if !ledger.records.contains_key(key) {
        return Err(CommandError::Protocol);
    }
    let mut keys = BTreeSet::from([key.to_owned()]);
    loop {
        let produced = keys
            .iter()
            .flat_map(|key| ledger.records[key].created.keys().cloned())
            .collect::<BTreeSet<_>>();
        let dependents = ledger
            .records
            .values()
            .filter(|record| !record.dependencies.is_disjoint(&produced))
            .map(|record| record.key.clone())
            .collect::<BTreeSet<_>>();
        let before = keys.len();
        keys.extend(dependents);
        if before == keys.len() {
            return Ok(keys);
        }
    }
}

fn simulate_inverse(
    ledger: &SyncLedger,
    rows: &Snapshot,
    schema: &Schema,
    version: u32,
    keys: &BTreeSet<String>,
) -> Result<Snapshot, CommandError> {
    let mut records = keys
        .iter()
        .map(|key| &ledger.records[key])
        .collect::<Vec<_>>();
    records.sort_by_key(|record| std::cmp::Reverse(record.order));
    let mut result = clone_snapshot(rows);
    for record in records {
        if record.rolled_back {
            continue;
        }
        if record.schema_version != version {
            return Err(CommandError::Protocol);
        }
        for change in record.undo.iter().rev() {
            for row in change.before.iter().chain(change.after.iter()) {
                if row.id != change.id {
                    return Err(CommandError::InvalidCommand);
                }
                validate_public_row(schema, version, &change.table, row)?;
            }
            let table = result
                .get_mut(&change.table)
                .ok_or(CommandError::InvalidCommand)?;
            if table.get(&change.id) != change.after.as_ref() {
                return Err(CommandError::RollbackConflict);
            }
            if let Some(before) = &change.before {
                table.insert(change.id.clone(), clone_row(before));
            } else {
                table.remove(&change.id);
            }
        }
    }
    Ok(result)
}

// Comparison state is part of the same inverse proof as application rows.
fn simulate_metadata_inverse(
    ledger: &SyncLedger,
    keys: &BTreeSet<String>,
) -> Result<Option<MetadataSnapshot>, CommandError> {
    let Some(entity) = &ledger.entity else {
        return Ok(None);
    };
    let mut result = MetadataSnapshot::new();
    let mut records = keys
        .iter()
        .map(|key| &ledger.records[key])
        .collect::<Vec<_>>();
    records.sort_by_key(|record| std::cmp::Reverse(record.order));
    for record in records {
        if record.rolled_back {
            continue;
        }
        for change in record.metadata_undo.iter().rev() {
            let identity = (change.table.clone(), change.record_id.clone());
            crate::storage::count_work!(metadata_entries_visited, 1);
            let current = result
                .get(&identity)
                .map_or_else(|| entity.metadata.get(&identity), Option::as_ref);
            if current != change.after.as_ref() {
                return Err(CommandError::RollbackConflict);
            }
            result.insert(identity, change.before.clone());
        }
    }
    Ok(Some(result))
}

// A dependent's undo baseline must acknowledge the accepted producer too.
/// `(identity, Some(field))` for an explicitly corrected field, `(identity, None)` for an
/// explicitly corrected tombstone.
type Corrected = BTreeSet<((String, String), Option<String>)>;

/// SPEC-0038 C1a (DEF-0039): promote only what the accepted write declared, and only where the
/// paired row cell still holds the accepted value (or absence) with a present stamp of the same
/// timestamp. A FieldStamp alone does not bind a value: equal clocks are legal for independent
/// pending writes. The authoritative flag is ignored in the match; a missing stamp is never
/// synthesized; explicitly corrected fields and tombstones keep their selected stamps.
fn promote_metadata(
    ledger: &mut LedgerTxn<'_>,
    accepted: &SyncRecord,
    rows: &Snapshot,
    corrected: &Corrected,
) {
    use super::entity::RowMetadata;
    let Some(descriptor) = &accepted.entity else {
        return;
    };
    let mut declared = descriptor.removed_fields.clone();
    if let super::Json::Object(args) = &accepted.args
        && let Some(super::Json::Object(changes)) = args.get("changes")
    {
        declared.extend(changes.keys().cloned());
    }
    let declared_delete = descriptor.op == super::EntityOperation::Delete;
    struct Accepted<'a> {
        identity: (String, String),
        meta: &'a RowMetadata,
        row: Option<&'a Row>,
        fields: Vec<&'a String>,
        tombstone: bool,
    }
    fn promote(target: &mut RowMetadata, row: Option<&Row>, accepted: &Accepted) {
        if row.is_some() != accepted.row.is_some() {
            return;
        }
        for name in &accepted.fields {
            let Some(stamp) = accepted.meta.fields.get(*name) else {
                continue;
            };
            if row.and_then(|r| r.fields.get(*name))
                != accepted.row.and_then(|r| r.fields.get(*name))
            {
                continue;
            }
            if let Some(current) = target.fields.get_mut(*name)
                && current.timestamp_ms == stamp.timestamp_ms
            {
                current.authoritative = true;
            }
        }
        if accepted.tombstone
            && row.is_none()
            && let (Some(stamp), Some(current)) =
                (accepted.meta.tombstone.as_ref(), target.tombstone.as_mut())
            && current.timestamp_ms == stamp.timestamp_ms
        {
            current.authoritative = true;
        }
    }
    for change in &accepted.metadata_undo {
        let Some(after) = &change.after else {
            continue;
        };
        crate::storage::count_work!(metadata_entries_visited, 1);
        let identity = (change.table.clone(), change.record_id.clone());
        // C1a-PAIR: X proves an identity only with exactly one matching row-undo entry and
        // exactly one matching metadata-undo entry; otherwise nothing is promoted from it.
        let Some(accepted_undo) = unique_pair(
            &accepted.undo,
            &accepted.metadata_undo,
            &change.table,
            &change.record_id,
        ) else {
            continue;
        };
        let accepted_row = accepted_undo.after.as_ref();
        let target = Accepted {
            fields: declared
                .iter()
                .filter(|name| !corrected.contains(&(identity.clone(), Some((*name).clone()))))
                .collect(),
            tombstone: declared_delete && !corrected.contains(&(identity.clone(), None)),
            identity,
            meta: after,
            row: accepted_row,
        };
        let current_row = rows
            .get(&target.identity.0)
            .and_then(|table| table.get(&target.identity.1));
        if let Some(current) = ledger.metadata_mut(&target.identity) {
            promote(current, current_row, &target);
        }
        let record_keys = ledger.records.keys().cloned().collect::<Vec<_>>();
        for record_key in record_keys {
            let mut record = ledger.records[&record_key].clone();
            if record.key == accepted.key {
                continue;
            }
            let SyncRecord {
                undo,
                metadata_undo,
                ..
            } = &mut record;
            // C1a-PAIR: another record without a unique pair is skipped; the others still apply.
            let Some(images) = unique_pair(undo, metadata_undo, &change.table, &change.record_id)
            else {
                continue;
            };
            for meta in metadata_undo.iter_mut() {
                if meta.table == change.table && meta.record_id == change.record_id {
                    if let Some(before) = meta.before.as_mut() {
                        promote(before, images.before.as_ref(), &target);
                    }
                    if let Some(after) = meta.after.as_mut() {
                        promote(after, images.after.as_ref(), &target);
                    }
                }
            }
            if record != ledger.records[&record_key] {
                ledger.insert_record(record_key, record);
            }
        }
    }
}

/// The single row-undo entry for an identity, when the record also has exactly one metadata-undo
/// entry for it. A missing or duplicated entry in either list is unknown, not explicit absence.
fn unique_pair<'a>(
    undo: &'a [CommittedChange],
    metadata_undo: &[super::entity::MetadataChange],
    table: &str,
    id: &str,
) -> Option<&'a CommittedChange> {
    let mut rows = undo
        .iter()
        .filter(|undo| undo.table == table && undo.id == id);
    let metas = metadata_undo
        .iter()
        .filter(|meta| meta.table == table && meta.record_id == id)
        .count();
    match (rows.next(), rows.next(), metas) {
        (Some(row), None, 1) => Some(row),
        _ => None,
    }
}

fn rewrite_row(
    schema: &Schema,
    table: &str,
    row: &mut Row,
    mappings: &BTreeMap<String, (String, String)>,
    rename: bool,
) {
    if rename
        && let Some((owner, remote)) = mappings.get(&row.id)
        && owner == table
    {
        row.id = remote.clone();
    }
    let Some(declaration) = schema.tables.get(table) else {
        return;
    };
    for (name, value) in &mut row.fields {
        if !declaration
            .columns
            .get(name)
            .is_some_and(|column| column.kind == ColumnType::Reference)
        {
            continue;
        }
        let Value::Reference(id) = value else {
            continue;
        };
        let Some((owner, remote)) = mappings.get(id) else {
            continue;
        };
        let target = declaration
            .relations
            .values()
            .find_map(|relation| match relation {
                Relation::ToOne {
                    table,
                    foreign_key,
                } if foreign_key == name => Some(table),
                _ => None,
            });
        if target.is_none_or(|target| target == owner) {
            *id = remote.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Json;

    #[test]
    fn si_w03_public_validation_candidate_counts_its_row_clone() {
        let schema = schema().storage_schema().unwrap();
        let candidate = row("server:a", "a");
        crate::storage::counters::reset();
        validate_public_row(&schema, 1, "parents", &candidate).unwrap();
        assert_eq!(crate::storage::counters::snapshot().rows_cloned, 1);
    }

    fn row(id: &str, name: &str) -> Row {
        Row {
            id: id.into(),
            fields: BTreeMap::from([("name".into(), Value::String(name.into()))]),
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
                        relations: BTreeMap::new(),
                    },
                ),
                (
                    "children".into(),
                    TableDefinition {
                        columns: BTreeMap::from([(
                            "note".into(),
                            Column {
                                kind: ColumnType::String,
                                optional: false,
                            },
                        )]),
                        relations: BTreeMap::from([
                            (
                                "left".into(),
                                RelationDefinition::ToOne {
                                    table: "parents".into(),
                                    foreign_key: "left_id".into(),
                                    optional: true,
                                },
                            ),
                            (
                                "right".into(),
                                RelationDefinition::ToOne {
                                    table: "parents".into(),
                                    foreign_key: "right_id".into(),
                                    optional: true,
                                },
                            ),
                        ]),
                    },
                ),
            ]),
        }
    }

    fn change(table: &str, before: Option<Row>, after: Option<Row>) -> CommittedChange {
        CommittedChange {
            table: table.into(),
            id: before.as_ref().or(after.as_ref()).unwrap().id.clone(),
            before,
            after,
        }
    }

    fn queued(
        order: i64,
        created: &str,
        table: &str,
        undo: Vec<CommittedChange>,
        refs: Vec<&str>,
    ) -> SyncRecord {
        SyncRecord {
            entity: None,
            metadata_undo: vec![],
            key: format!("0000000000000064-{}-{order:016x}", "e".repeat(32)),
            command: "write-v1".into(),
            args: Json::Object(BTreeMap::from([
                (
                    "refs".into(),
                    Json::Array(refs.iter().map(|id| Json::String((*id).into())).collect()),
                ),
                ("plain".into(), Json::String("local:a".into())),
            ])),
            context: Json::Object(BTreeMap::from([
                ("identity".into(), Json::Null),
                (
                    "capturedAt".into(),
                    Json::String("1970-01-01T00:00:00.100Z".into()),
                ),
            ])),
            schema_version: 1,
            order,
            reference_paths: (0..refs.len())
                .map(|index| vec![PathPart::Key("refs".into()), PathPart::Index(index)])
                .collect(),
            dependencies: refs.into_iter().map(str::to_owned).collect(),
            created: BTreeMap::from([(created.into(), table.into())]),
            mappings: BTreeMap::new(),
            undo,
            state: SyncState::Queued,
            frozen: None,
            attempts: 0,
            due_ms: 0,
            outcome: None,
            rolled_back: false,
            reconciled: false,
        }
    }

    fn received(record: &mut SyncRecord, rejected: bool) {
        record.frozen = Some(record.envelope().canonical().unwrap());
        record.attempts = 1;
        record.state = if rejected {
            SyncState::Rejected
        } else {
            SyncState::Applied
        };
        record.outcome = Some(if rejected {
            RemoteOutcome::Rejected("no".into())
        } else {
            RemoteOutcome::Applied(Json::Null)
        });
    }

    fn ledger(records: Vec<SyncRecord>) -> SyncLedger {
        let ledger = SyncLedger {
            entity: None,
            header: SyncHeader {
                namespace: "e".repeat(32),
                sequence: 20,
                last_ms: 100,
                options: SyncOptions::default(),
            },
            records: records
                .into_iter()
                .map(|record| (record.key.clone(), record))
                .collect(),
        };
        ledger.validate().unwrap();
        ledger
    }

    #[test]
    fn e06_inverse_restores_and_guards_comparison_metadata() {
        use super::super::entity::*;
        let prior = row("server:1", "before");
        let mut rejected = queued(
            1,
            "local:unused",
            "parents",
            vec![change("parents", Some(prior.clone()), None)],
            vec![],
        );
        rejected.created.clear();
        received(&mut rejected, true);
        rejected.reconciled = true;
        let key = rejected.key.clone();
        let mut ledger = ledger(vec![rejected]);
        let before = RowMetadata {
            fields: BTreeMap::from([(
                "name".into(),
                FieldStamp {
                    timestamp_ms: 10,
                    authoritative: true,
                },
            )]),
            tombstone: None,
        };
        let mut after = before.clone();
        after.tombstone = Some(FieldStamp {
            timestamp_ms: 20,
            authoritative: false,
        });
        let identity = ("parents".into(), "server:1".into());
        let mut entity = EntityLedger::new(EntityConfig {
            scope: "test-scope".into(),
            tables: BTreeSet::from(["parents".into()]),
        });
        entity.metadata.insert(identity.clone(), after.clone());
        ledger.entity = Some(entity);
        ledger
            .records
            .get_mut(&key)
            .unwrap()
            .metadata_undo
            .push(MetadataChange {
                table: "parents".into(),
                record_id: "server:1".into(),
                before: Some(before.clone()),
                after: Some(after),
            });
        let rows = Snapshot::from([
            ("parents".into(), BTreeMap::new()),
            ("children".into(), BTreeMap::new()),
        ]);
        let planned = inverse(&ledger, &rows, &schema(), &key, false).unwrap();
        assert_eq!(
            planned.ledger.entity.as_ref().unwrap().metadata[&identity],
            before,
            "E06 rollback must remove the rejected optimistic deletion tombstone"
        );
        ledger
            .entity
            .as_mut()
            .unwrap()
            .metadata
            .get_mut(&identity)
            .unwrap()
            .tombstone
            .as_mut()
            .unwrap()
            .timestamp_ms = 21;
        assert!(
            matches!(
                inverse(&ledger, &rows, &schema(), &key, false),
                Err(CommandError::RollbackConflict)
            ),
            "metadata conflicts must preserve row and comparison state together"
        );
    }

    fn mapped_fixture() -> (SyncLedger, Snapshot, String, String, String) {
        let arow = row("local:a", "local:a");
        let brow = row("local:b", "B");
        let child = Row {
            id: "local:c".into(),
            fields: BTreeMap::from([
                ("note".into(), Value::String("local:a".into())),
                ("left_id".into(), Value::Reference("local:a".into())),
                ("right_id".into(), Value::Reference("local:b".into())),
            ]),
        };
        let mut a = queued(
            1,
            "local:a",
            "parents",
            vec![change("parents", None, Some(arow.clone()))],
            vec![],
        );
        received(&mut a, false);
        let b = queued(
            2,
            "local:b",
            "parents",
            vec![change("parents", None, Some(brow.clone()))],
            vec![],
        );
        let c = queued(
            3,
            "local:c",
            "children",
            vec![change("children", None, Some(child.clone()))],
            vec!["local:a", "local:b"],
        );
        let keys = (a.key.clone(), b.key.clone(), c.key.clone());
        (
            ledger(vec![a, b, c]),
            BTreeMap::from([
                (
                    "parents".into(),
                    BTreeMap::from([(arow.id.clone(), arow), (brow.id.clone(), brow)]),
                ),
                (
                    "children".into(),
                    BTreeMap::from([(child.id.clone(), child)]),
                ),
            ]),
            keys.0,
            keys.1,
            keys.2,
        )
    }

    fn plan(local: &str, remote: &str) -> ReconcilePlan {
        ReconcilePlan {
            mappings: BTreeMap::from([(local.into(), remote.into())]),
            writes: vec![],
        }
    }

    #[test]
    fn c10_c13_mapping_rewrites_only_identities_and_releases_all_parents_atomically() {
        let (original, rows, a, b, c) = mapped_fixture();
        let frozen = original.records[&a].frozen.clone();
        let mut correction = plan("local:a", "server:a");
        correction.writes.push(ReconcileWrite::Put {
            table: "parents".into(),
            id: "server:a".into(),
            fields: row("server:a", "corrected").fields,
        });
        let first = reconcile(&original, &rows, &schema(), &a, correction).unwrap();
        assert_eq!(
            first.rows["parents"]["server:a"].fields["name"],
            Value::String("corrected".into())
        );
        assert!(!first.rows["parents"].contains_key("local:a"));
        assert_eq!(
            first.rows["children"]["local:c"].fields["left_id"],
            Value::Reference("server:a".into())
        );
        assert_eq!(
            first.rows["children"]["local:c"].fields["note"],
            Value::String("local:a".into())
        );
        assert_eq!(
            first.ledger.records[&c].dependencies,
            BTreeSet::from(["local:b".into()])
        );
        assert_eq!(first.ledger.records[&a].frozen, frozen);
        assert!(first.ledger.records[&a].undo.is_empty());
        assert_eq!(
            first.ledger.records[&c].undo[0]
                .after
                .as_ref()
                .unwrap()
                .fields["left_id"],
            Value::Reference("server:a".into())
        );
        assert!(
            matches!(&first.ledger.records[&c].args,Json::Object(args) if args["plain"]==Json::String("local:a".into()))
        );
        assert_eq!(
            rows["children"]["local:c"].fields["left_id"],
            Value::Reference("local:a".into())
        );
        let mut next = first.ledger;
        received(next.records.get_mut(&b).unwrap(), false);
        let final_plan = reconcile(
            &next,
            &first.rows,
            &schema(),
            &b,
            plan("local:b", "server:b"),
        )
        .unwrap();
        assert!(final_plan.ledger.records[&c].dependencies.is_empty());
        assert_eq!(
            final_plan.rows["children"]["local:c"].fields["right_id"],
            Value::Reference("server:b".into())
        );
        final_plan.ledger.validate().unwrap();
    }

    #[test]
    fn c11_collision_invalid_fields_and_independent_overlap_change_nothing() {
        let (original, mut rows, a, _, _) = mapped_fixture();
        rows.get_mut("parents")
            .unwrap()
            .insert("server:a".into(), row("server:a", "occupied"));
        assert!(matches!(
            reconcile(&original, &rows, &schema(), &a, plan("local:a", "server:a")),
            Err(CommandError::InvalidCommand)
        ));
        rows.get_mut("parents").unwrap().remove("server:a");
        assert!(reconcile(&original, &rows, &schema(), &a, ReconcilePlan::default()).is_err());
        assert!(reconcile(&original, &rows, &schema(), &a, plan("local:b", "server:a")).is_err());
        let mut invalid = plan("local:a", "server:a");
        invalid.writes.push(ReconcileWrite::Put {
            table: "parents".into(),
            id: "server:a".into(),
            fields: BTreeMap::from([("id".into(), Value::String("managed".into()))]),
        });
        assert!(reconcile(&original, &rows, &schema(), &a, invalid).is_err());
        rows.get_mut("parents")
            .unwrap()
            .get_mut("local:a")
            .unwrap()
            .fields = row("local:a", "independent").fields;
        // SPEC-0038 relabel: the independent-overlap conflict is now asserted on a plan
        // WITH a write (the guarded path); a no-write confirmation is SPEC-0038 C1.
        let mut guarded = plan("local:a", "server:a");
        guarded.writes.push(ReconcileWrite::Put {
            table: "parents".into(),
            id: "server:a".into(),
            fields: row("server:a", "backend").fields,
        });
        assert!(matches!(
            reconcile(&original, &rows, &schema(), &a, guarded),
            Err(CommandError::RollbackConflict)
        ));
        assert_eq!(
            rows["parents"]["local:a"].fields["name"],
            Value::String("independent".into())
        );
        assert!(!original.records[&a].reconciled);
    }

    // SPEC-0038 C1 / TC-0042: a confirmation without writes needs no inverse proof.
    #[test]
    fn c1_no_write_confirmation_succeeds_under_an_independent_overlap() {
        let (original, mut rows, a, _, _) = mapped_fixture();
        rows.get_mut("parents")
            .unwrap()
            .get_mut("local:a")
            .unwrap()
            .fields = row("local:a", "independent").fields;
        let planned =
            reconcile(&original, &rows, &schema(), &a, plan("local:a", "server:a")).unwrap();
        assert!(planned.ledger.records[&a].reconciled);
        assert!(!planned.rows["parents"].contains_key("local:a"));
        assert_eq!(
            planned.rows["parents"]["server:a"].fields["name"],
            Value::String("independent".into()),
            "the later edit is kept; only the identity changes"
        );
    }

    fn inverse_fixture() -> (SyncLedger, Snapshot, String, String, String) {
        let before = row("local:a", "a");
        let middle = row("local:a", "b");
        let after = row("local:a", "c");
        let brow = row("local:b", "B");
        let crow = row("local:c", "C");
        let mut a = queued(
            1,
            "local:a",
            "parents",
            vec![change("parents", None, Some(before.clone()))],
            vec![],
        );
        received(&mut a, true);
        a.reconciled = true;
        let b = queued(
            2,
            "local:b",
            "parents",
            vec![
                change("parents", None, Some(brow.clone())),
                change("parents", Some(before), Some(middle.clone())),
            ],
            vec!["local:a"],
        );
        let c = queued(
            3,
            "local:c",
            "parents",
            vec![
                change("parents", None, Some(crow.clone())),
                change("parents", Some(middle), Some(after.clone())),
            ],
            vec!["local:a", "local:b"],
        );
        let keys = (a.key.clone(), b.key.clone(), c.key.clone());
        (
            ledger(vec![a, b, c]),
            BTreeMap::from([
                (
                    "parents".into(),
                    BTreeMap::from([
                        (after.id.clone(), after),
                        (brow.id.clone(), brow),
                        (crow.id.clone(), crow),
                        ("unrelated".into(), row("unrelated", "keep")),
                    ]),
                ),
                ("children".into(), BTreeMap::new()),
            ]),
            keys.0,
            keys.1,
            keys.2,
        )
    }

    #[test]
    fn cp07_in_flight_descendant_is_invalid_and_defensively_refused() {
        let (original, _, root, dependent, _) = inverse_fixture();
        let keys = closure(&original, &root).unwrap();
        assert!(keys.contains(&dependent));
        assert_eq!(discard::check(&original, &keys, None), Ok(()));
        for state in [
            SyncState::Sent,
            SyncState::Acknowledged,
            SyncState::Cancelled,
        ] {
            let mut invalid = original.clone();
            let record = invalid.records.get_mut(&dependent).unwrap();
            record.state = state;
            record.frozen = Some(record.envelope().canonical().unwrap());
            record.attempts = 1;
            // Dispatch cannot create this state: frozen records require empty dependencies.
            assert!(invalid.validate().is_err());
            let before = invalid.clone();
            assert_eq!(
                discard::check(&invalid, &keys, None),
                Err(CommandError::RollbackConflict)
            );
            assert_eq!(
                invalid, before,
                "defensive closure check has no side effects"
            );
        }
    }

    #[test]
    fn cp09_metadata_only_descriptor_refuses_discard() {
        let (mut original, _, root, _, _) = inverse_fixture();
        let keys = closure(&original, &root).unwrap();
        assert_eq!(discard::check(&original, &keys, None), Ok(()));
        // Isolate the metadata_undo condition from entity descriptors and configured tables.
        original
            .records
            .get_mut(&root)
            .unwrap()
            .metadata_undo
            .push(super::super::MetadataChange {
                table: "children".into(),
                record_id: "metadata-only".into(),
                before: None,
                after: Some(super::super::RowMetadata::default()),
            });
        assert_eq!(
            discard::check(&original, &keys, None),
            Err(CommandError::RollbackConflict)
        );
    }

    #[test]
    fn c14_inverse_reverses_ordered_transitive_multi_parent_closure_and_is_idempotent() {
        let (original, rows, a, b, c) = inverse_fixture();
        let result = inverse(&original, &rows, &schema(), &a, false).unwrap();
        assert!(!result.conflict);
        assert_eq!(
            result.rows["parents"],
            BTreeMap::from([("unrelated".into(), row("unrelated", "keep"))])
        );
        for key in [&a, &b, &c] {
            assert!(result.ledger.records[key].rolled_back);
            assert!(result.ledger.records[key].undo.is_empty());
        }
        assert_eq!(result.ledger.records[&b].state, SyncState::Cancelled);
        assert_eq!(result.ledger.header, original.header);
        let repeated = inverse(&result.ledger, &result.rows, &schema(), &a, false).unwrap();
        assert_eq!(repeated.rows, result.rows);
        assert_eq!(repeated.ledger, result.ledger);
    }

    #[test]
    fn c15_cancel_conflict_persists_stop_without_partial_inverse_then_can_recover() {
        let (original, mut rows, a, b, c) = inverse_fixture();
        rows.get_mut("parents")
            .unwrap()
            .get_mut("local:a")
            .unwrap()
            .fields = row("local:a", "independent").fields;
        assert!(matches!(
            inverse(&original, &rows, &schema(), &a, false),
            Err(CommandError::RollbackConflict)
        ));
        let cancelled = inverse(&original, &rows, &schema(), &a, true).unwrap();
        assert!(cancelled.conflict);
        assert_eq!(cancelled.rows, rows);
        for key in [&a, &b, &c] {
            let record = &cancelled.ledger.records[key];
            assert_eq!(record.state, SyncState::Cancelled);
            assert!(!record.rolled_back);
            assert!(!record.undo.is_empty());
        }
        rows.get_mut("parents")
            .unwrap()
            .get_mut("local:a")
            .unwrap()
            .fields = row("local:a", "c").fields;
        let recovered = inverse(&cancelled.ledger, &rows, &schema(), &a, false).unwrap();
        assert!(recovered.ledger.records[&a].rolled_back);
        assert!(recovered.ledger.records[&a].frozen.is_some());
        assert!(recovered.ledger.live_keys().contains(&a));
        assert_eq!(recovered.rows["parents"].len(), 1);
    }

    // A conflicted cancel of a processed rejection persists Cancelled (C15). Its stored
    // Rejected outcome is terminal, so the manual SPEC-0040 discard exit must survive.
    #[test]
    fn c15_conflicted_cancel_of_a_processed_rejection_keeps_the_discard_exit() {
        let (original, mut rows, a, b, c) = inverse_fixture();
        rows.get_mut("parents")
            .unwrap()
            .get_mut("local:a")
            .unwrap()
            .fields = row("local:a", "independent").fields;
        let before = owned(&original, |txn| discard::plan_in(txn, &rows, &schema(), &a))
            .expect("a processed rejection is discardable before the cancel");
        assert_eq!(before.ledger.records[&a].state, SyncState::Discarded);
        let cancelled = inverse(&original, &rows, &schema(), &a, true).unwrap();
        assert!(cancelled.conflict);
        let root = &cancelled.ledger.records[&a];
        assert_eq!(root.state, SyncState::Cancelled);
        assert!(root.frozen.is_some() && root.attempts > 0);
        assert!(matches!(root.outcome, Some(RemoteOutcome::Rejected(_))));
        let discarded = owned(&cancelled.ledger, |txn| {
            discard::plan_in(txn, &rows, &schema(), &a)
        })
        .expect("the terminal rejection keeps the conflicted cancel discardable");
        assert_eq!(discarded.rows, rows, "discard keeps every current row");
        for key in [&a, &b, &c] {
            assert_eq!(discarded.ledger.records[key].state, SyncState::Discarded);
        }
        assert!(discarded.ledger.live_keys().is_empty());
        discarded.ledger.validate().unwrap();
        // Control: a sent cancellation without a terminal outcome may still execute remotely.
        let mut in_flight = cancelled.ledger.clone();
        in_flight.records.get_mut(&a).unwrap().outcome = None;
        in_flight.validate().unwrap();
        assert!(matches!(
            owned(&in_flight, |txn| discard::plan_in(
                txn,
                &rows,
                &schema(),
                &a
            )),
            Err(CommandError::Protocol)
        ));
    }

    // SPEC-0038 C1: a no-write confirmation commits mappings and queued normalization under
    // later edits. A dependent retained by a conflicted cancel still names the local identity.
    #[test]
    fn c1_no_write_confirmation_renormalizes_a_conflict_cancelled_dependent() {
        let (original, mut rows, a, _, c) = mapped_fixture();
        rows.get_mut("children")
            .unwrap()
            .get_mut("local:c")
            .unwrap()
            .fields
            .insert("note".into(), Value::String("independent".into()));
        let cancelled = inverse(&original, &rows, &schema(), &c, true).unwrap();
        assert!(cancelled.conflict);
        let dependent = &cancelled.ledger.records[&c];
        assert_eq!(dependent.state, SyncState::Cancelled);
        assert!(!dependent.rolled_back && dependent.dependencies.contains("local:a"));
        let confirmed = reconcile(
            &cancelled.ledger,
            &rows,
            &schema(),
            &a,
            plan("local:a", "server:a"),
        )
        .expect("a retained cancelled dependent cannot strand the accepted producer");
        confirmed.ledger.validate().unwrap();
        let dependent = &confirmed.ledger.records[&c];
        assert_eq!(dependent.state, SyncState::Cancelled);
        assert!(!dependent.rolled_back && !dependent.undo.is_empty());
        assert_eq!(
            dependent.dependencies,
            BTreeSet::from(["local:b".to_owned()])
        );
        let Json::Object(args) = &dependent.args else {
            panic!("fixture arguments")
        };
        assert_eq!(
            args["refs"],
            Json::Array(vec![
                Json::String("server:a".into()),
                Json::String("local:b".into())
            ])
        );
        assert_eq!(
            confirmed.rows["children"]["local:c"].fields["left_id"],
            Value::Reference("server:a".into())
        );
        // The retained inverse still proves and restores against the rewritten identity.
        let mut restored = confirmed.rows.clone();
        restored
            .get_mut("children")
            .unwrap()
            .insert("local:c".into(), dependent.undo[0].after.clone().unwrap());
        let rolled = inverse(&confirmed.ledger, &restored, &schema(), &c, false).unwrap();
        assert!(rolled.ledger.records[&c].rolled_back);
        assert!(!rolled.rows["children"].contains_key("local:c"));
    }

    // SPEC-0039 step 4: only a live record outside the closure protects a row from a guarded
    // correction. A Discarded record keeps its undo (SPEC-0040) without being live.
    #[test]
    fn gw_outside_closure_refusal_requires_a_live_record() {
        let (mut original, mut rows, a, _, _) = mapped_fixture();
        let shared = row("server:s", "mine");
        rows.get_mut("parents")
            .unwrap()
            .insert(shared.id.clone(), shared.clone());
        let later = queued(
            4,
            "local:d",
            "parents",
            vec![change(
                "parents",
                Some(row("server:s", "old")),
                Some(shared.clone()),
            )],
            vec![],
        );
        let d = later.key.clone();
        original.records.insert(d.clone(), later);
        original.validate().unwrap();
        let attempt = |ledger: &SyncLedger| {
            let mut correction = plan("local:a", "server:a");
            correction.writes.push(ReconcileWrite::Put {
                table: "parents".into(),
                id: "server:s".into(),
                fields: row("server:s", "corrected").fields,
            });
            reconcile_with_hooks(
                ledger,
                &rows,
                &schema(),
                &a,
                correction,
                vec![CorrectionGuard {
                    table: "parents".into(),
                    id: "server:s".into(),
                    expected: Some(shared.fields.clone()),
                }],
                &Hooks::new(),
                &crate::command::ConflictPolicy::default(),
            )
        };
        assert!(
            matches!(attempt(&original), Err(CommandError::RollbackConflict)),
            "a live later intent on the row still refuses the guarded write"
        );
        let discarded = owned(&original, |txn| discard::plan_in(txn, &rows, &schema(), &d))
            .unwrap()
            .ledger;
        assert_eq!(discarded.records[&d].state, SyncState::Discarded);
        assert!(!discarded.records[&d].undo.is_empty());
        assert!(!discarded.live_keys().contains(&d));
        let (planned, guards) = attempt(&discarded)
            .expect("retired Discarded history does not protect the row any longer");
        assert_eq!(
            planned.rows["parents"]["server:s"].fields["name"],
            Value::String("corrected".into())
        );
        assert_eq!(guards.len(), 1);
    }

    #[test]
    fn c13_processed_plans_are_exact_noops_and_rejection_never_auto_rolls_back() {
        let (original, rows, a, _, _) = mapped_fixture();
        let mut retired = plan("local:a", "server:a");
        retired.writes.push(ReconcileWrite::Put {
            table: "parents".into(),
            id: "local:a".into(),
            fields: row("local:a", "retired").fields,
        });
        assert!(matches!(
            reconcile(&original, &rows, &schema(), &a, retired),
            Err(CommandError::InvalidCommand)
        ));
        let accepted =
            reconcile(&original, &rows, &schema(), &a, plan("local:a", "server:a")).unwrap();
        let repeated = reconcile(
            &accepted.ledger,
            &accepted.rows,
            &schema(),
            &a,
            plan("local:a", "server:a"),
        )
        .unwrap();
        assert_eq!(repeated.ledger, accepted.ledger);
        assert_eq!(repeated.rows, accepted.rows);
        assert!(matches!(
            reconcile(
                &accepted.ledger,
                &accepted.rows,
                &schema(),
                &a,
                plan("local:a", "different")
            ),
            Err(CommandError::Protocol)
        ));
        let mut extra = plan("local:a", "server:a");
        extra.writes.push(ReconcileWrite::Remove {
            table: "parents".into(),
            id: "server:a".into(),
        });
        assert!(matches!(
            reconcile(&accepted.ledger, &accepted.rows, &schema(), &a, extra),
            Err(CommandError::Protocol)
        ));
        let mut rejected = original;
        received(rejected.records.get_mut(&a).unwrap(), true);
        let processed =
            reconcile(&rejected, &rows, &schema(), &a, ReconcilePlan::default()).unwrap();
        assert_eq!(processed.rows, rows);
        assert!(processed.ledger.records[&a].reconciled);
        assert!(!processed.ledger.records[&a].rolled_back);
        assert!(!processed.ledger.records[&a].undo.is_empty());
        assert!(reconcile(&rejected, &rows, &schema(), &a, plan("local:a", "server:a")).is_err());
    }

    #[test]
    fn c19_inverse_trims_resolved_history_before_exact_ledger_validation() {
        let (mut original, rows, a, _, c) = inverse_fixture();
        original.header.options.history = 1;
        let result = inverse(&original, &rows, &schema(), &a, false).unwrap();
        assert_eq!(result.ledger.records.len(), 1);
        assert!(result.ledger.records.contains_key(&c));
        result.ledger.validate().unwrap();
        assert_eq!(result.rows["parents"].len(), 1);
        assert_eq!(result.ledger.header, original.header);
    }

    #[test]
    fn c13_same_final_id_in_other_table_does_not_retarget_dangling_reference() {
        let (mut ledger, mut rows, a, _, c) = mapped_fixture();
        let mut schema = schema();
        schema.tables.insert(
            "others".into(),
            TableDefinition {
                columns: BTreeMap::from([(
                    "name".into(),
                    Column {
                        kind: ColumnType::String,
                        optional: false,
                    },
                )]),
                relations: BTreeMap::new(),
            },
        );
        schema.tables.get_mut("children").unwrap().relations.insert(
            "other".into(),
            RelationDefinition::ToOne {
                table: "others".into(),
                foreign_key: "other_id".into(),
                optional: true,
            },
        );
        rows.insert(
            "others".into(),
            BTreeMap::from([("server:a".into(), row("server:a", "other table"))]),
        );
        rows.get_mut("children")
            .unwrap()
            .get_mut("local:c")
            .unwrap()
            .fields
            .insert("other_id".into(), Value::Reference("local:a".into()));
        ledger.records.get_mut(&c).unwrap().undo[0]
            .after
            .as_mut()
            .unwrap()
            .fields
            .insert("other_id".into(), Value::Reference("local:a".into()));
        let result = reconcile(&ledger, &rows, &schema, &a, plan("local:a", "server:a")).unwrap();
        assert_eq!(
            result.rows["others"]["server:a"],
            rows["others"]["server:a"]
        );
        assert_eq!(
            result.rows["children"]["local:c"].fields["left_id"],
            Value::Reference("server:a".into())
        );
        assert_eq!(
            result.rows["children"]["local:c"].fields["other_id"],
            Value::Reference("local:a".into())
        );
    }

    #[test]
    fn c14_unrolled_cancelled_record_does_not_require_prior_outcome_processing() {
        let (mut ledger, mut rows, a, b, c) = mapped_fixture();
        ledger.records.remove(&b);
        ledger.records.remove(&c);
        rows.get_mut("children").unwrap().clear();
        let record = ledger.records.get_mut(&a).unwrap();
        record.state = SyncState::Cancelled;
        record.outcome = None;
        record.reconciled = false;
        ledger.validate().unwrap();
        let result = inverse(&ledger, &rows, &schema(), &a, false).unwrap();
        assert!(result.ledger.records[&a].rolled_back);
        assert!(result.ledger.records[&a].reconciled);
        assert!(!result.rows["parents"].contains_key("local:a"));
        assert!(result.rows["parents"].contains_key("local:b"));
    }
    fn correction_schema() -> SchemaDefinition {
        let mut s = schema();
        let columns = &mut s.tables.get_mut("parents").unwrap().columns;
        for name in ["other", "drop"] {
            columns.insert(
                name.into(),
                Column {
                    kind: ColumnType::String,
                    optional: true,
                },
            );
        }
        s
    }
    fn correction_fixture(
        op: crate::command::EntityOperation,
    ) -> (SyncLedger, Snapshot, String, String) {
        use crate::command::entity::*;
        let id = if op == EntityOperation::Create {
            format!("entity:{}:0000000000000001", "e".repeat(32))
        } else {
            "server:row".into()
        };
        let mut prior = row(&id, "before");
        prior
            .fields
            .insert("other".into(), Value::String("untouched".into()));
        prior
            .fields
            .insert("drop".into(), Value::String("prior".into()));
        let mut after = prior.clone();
        after
            .fields
            .insert("name".into(), Value::String("optimistic".into()));
        let before_meta = RowMetadata {
            fields: BTreeMap::from([
                (
                    "name".into(),
                    FieldStamp {
                        timestamp_ms: 10,
                        authoritative: true,
                    },
                ),
                (
                    "other".into(),
                    FieldStamp {
                        timestamp_ms: 20,
                        authoritative: true,
                    },
                ),
                (
                    "drop".into(),
                    FieldStamp {
                        timestamp_ms: 30,
                        authoritative: true,
                    },
                ),
            ]),
            tombstone: None,
        };
        let mut after_meta = before_meta.clone();
        if op == EntityOperation::Delete {
            after_meta.tombstone = Some(FieldStamp {
                timestamp_ms: 100,
                authoritative: false,
            });
        } else {
            for name in if op == EntityOperation::Create {
                vec!["name", "other", "drop"]
            } else {
                vec!["name", "drop"]
            } {
                after_meta.fields.insert(
                    name.into(),
                    FieldStamp {
                        timestamp_ms: 100,
                        authoritative: false,
                    },
                );
            }
        }
        let change = change(
            "parents",
            (op != EntityOperation::Create).then_some(prior),
            (op != EntityOperation::Delete).then_some(after.clone()),
        );
        let mut record = queued(1, &id, "parents", vec![change], vec![]);
        if op != EntityOperation::Create {
            record.created.clear();
        }
        record.command.clear();
        record.entity = Some(EntityDescriptor {
            table: "parents".into(),
            op,
            timestamp_ms: 100,
            removed_fields: BTreeSet::new(),
        });
        let fields = if op == EntityOperation::Delete {
            BTreeMap::new()
        } else {
            after
                .fields
                .iter()
                .filter(|(k, _)| op == EntityOperation::Create || k.as_str() != "other")
                .map(|(k, v)| {
                    (
                        k.clone(),
                        match v {
                            Value::String(s) => Json::String(s.clone()),
                            _ => panic!(),
                        },
                    )
                })
                .collect()
        };
        record.args = Json::Object(BTreeMap::from([
            ("recordId".into(), Json::String(id.clone())),
            ("changes".into(), Json::Object(fields)),
        ]));
        record.reference_paths = if op == EntityOperation::Create {
            vec![]
        } else {
            vec![vec![PathPart::Key("recordId".into())]]
        };
        record.metadata_undo = vec![MetadataChange {
            table: "parents".into(),
            record_id: id.clone(),
            before: (op != EntityOperation::Create).then_some(before_meta),
            after: Some(after_meta.clone()),
        }];
        received(&mut record, false);
        record.state = SyncState::Corrected;
        record.outcome = Some(RemoteOutcome::Corrected(Json::Null));
        let key = record.key.clone();
        let mut ledger = ledger(vec![]);
        let mut e = EntityLedger::new(EntityConfig {
            scope: "correction".into(),
            tables: BTreeSet::from(["parents".into()]),
        });
        e.metadata
            .insert(("parents".into(), id.clone()), after_meta);
        ledger.entity = Some(e);
        ledger.records.insert(key.clone(), record);
        ledger.validate().unwrap();
        let parents = if op == EntityOperation::Delete {
            BTreeMap::new()
        } else {
            BTreeMap::from([(id.clone(), after)])
        };
        (
            ledger,
            Snapshot::from([
                ("parents".into(), parents),
                ("children".into(), BTreeMap::new()),
            ]),
            key,
            id,
        )
    }
    fn typed_change(id: &str) -> crate::command::EntityChange {
        crate::command::EntityChange {
            table: "parents".into(),
            record_id: id.into(),
            op: crate::command::EntityOperation::Update,
            changes: BTreeMap::from([
                ("name".into(), Value::String("corrected".into())),
                ("drop".into(), Value::Null),
            ]),
            removed_fields: BTreeSet::from(["drop".into()]),
            timestamp_ms: 50,
            field_timestamps: BTreeMap::from([("name".into(), 50), ("drop".into(), 50)]),
        }
    }
    #[test]
    fn e06_typed_correction_uses_exact_backend_clocks_and_preserves_partial_state() {
        use crate::command::entity::*;
        let (ledger, rows, key, id) = correction_fixture(EntityOperation::Update);
        let planned = reconcile(
            &ledger,
            &rows,
            &correction_schema(),
            &key,
            ReconcilePlan {
                writes: vec![ReconcileWrite::Entity {
                    change: typed_change(&id),
                }],
                ..Default::default()
            },
        )
        .expect(
            "typed backend correction must override superseded optimistic clock100 with selected50",
        );
        let row = &planned.rows["parents"][&id];
        assert_eq!(row.fields["name"], Value::String("corrected".into()));
        assert_eq!(row.fields["other"], Value::String("untouched".into()));
        assert!(!row.fields.contains_key("drop"));
        let metadata =
            &planned.ledger.entity.as_ref().unwrap().metadata[&("parents".into(), id.clone())];
        assert_eq!(
            metadata.fields["name"],
            FieldStamp {
                timestamp_ms: 50,
                authoritative: true
            }
        );
        assert_eq!(
            metadata.fields["drop"],
            FieldStamp {
                timestamp_ms: 50,
                authoritative: true
            }
        );
        assert_eq!(
            metadata.fields["other"],
            FieldStamp {
                timestamp_ms: 20,
                authoritative: true
            }
        );
        let mut stale = typed_change(&id);
        stale.changes = BTreeMap::from([("name".into(), Value::String("stale".into()))]);
        stale.removed_fields.clear();
        stale.field_timestamps = BTreeMap::from([("name".into(), 40)]);
        assert_eq!(
            resolve_entity(
                &correction_schema(),
                Some(row),
                metadata,
                &stale,
                true,
                None
            )
            .unwrap()
            .row
            .as_ref(),
            Some(row)
        );
        assert!(planned.ledger.records[&key].reconciled);
        assert!(planned.ledger.records[&key].metadata_undo.is_empty());
        assert_eq!(planned.ledger.records.len(), ledger.records.len());
    }
    #[test]
    fn e06_entity_origin_untyped_corrections_reject_without_mutation() {
        let (ledger, rows, key, id) = correction_fixture(crate::command::EntityOperation::Update);
        for write in [
            ReconcileWrite::Put {
                table: "parents".into(),
                id: id.clone(),
                fields: rows["parents"][&id].fields.clone(),
            },
            ReconcileWrite::Remove {
                table: "parents".into(),
                id: id.clone(),
            },
        ] {
            assert!(
                matches!(
                    reconcile(
                        &ledger,
                        &rows,
                        &correction_schema(),
                        &key,
                        ReconcilePlan {
                            writes: vec![write],
                            ..Default::default()
                        }
                    ),
                    Err(CommandError::InvalidCommand)
                ),
                "entity outcome must not infer clocks for legacy writes"
            );
        }
        assert!(!ledger.records[&key].reconciled);
        assert!(rows["parents"].contains_key(&id));
    }
    #[test]
    fn e06_typed_self_mapping_correction_and_invalid_later_write_are_atomic() {
        use crate::command::EntityOperation;
        let (ledger, rows, key, id) = correction_fixture(EntityOperation::Create);
        let mappings = BTreeMap::from([(id.clone(), id.clone())]);
        let result = reconcile(
            &ledger,
            &rows,
            &correction_schema(),
            &key,
            ReconcilePlan {
                mappings: mappings.clone(),
                writes: vec![ReconcileWrite::Entity {
                    change: typed_change(&id),
                }],
            },
        )
        .expect("identity preserving mapping retains its valid correction identity");
        assert!(result.rows["parents"].contains_key(&id));
        let remote = "server:canonical".to_owned();
        let renamed = BTreeMap::from([(id.clone(), remote.clone())]);
        assert!(
            matches!(
                reconcile(
                    &ledger,
                    &rows,
                    &correction_schema(),
                    &key,
                    ReconcilePlan {
                        mappings: renamed.clone(),
                        writes: vec![ReconcileWrite::Entity {
                            change: typed_change(&id)
                        }]
                    }
                ),
                Err(CommandError::InvalidCommand)
            ),
            "actual renamed old identity is retired"
        );
        let renamed_result = reconcile(
            &ledger,
            &rows,
            &correction_schema(),
            &key,
            ReconcilePlan {
                mappings: renamed,
                writes: vec![ReconcileWrite::Entity {
                    change: typed_change(&remote),
                }],
            },
        )
        .unwrap();
        let metadata = &renamed_result.ledger.entity.as_ref().unwrap().metadata;
        assert!(!metadata.contains_key(&("parents".into(), id.clone())));
        assert_eq!(
            metadata[&("parents".into(), remote)].fields["other"],
            crate::command::FieldStamp {
                timestamp_ms: 100,
                authoritative: true
            },
            "promotion follows canonical metadata-undo identity"
        );
        let mut invalid = typed_change(&id);
        invalid.field_timestamps.clear();
        assert!(
            reconcile(
                &ledger,
                &rows,
                &correction_schema(),
                &key,
                ReconcilePlan {
                    mappings,
                    writes: vec![
                        ReconcileWrite::Entity {
                            change: typed_change(&id)
                        },
                        ReconcileWrite::Entity {
                            change: invalid
                        }
                    ]
                }
            )
            .is_err()
        );
        assert_eq!(
            rows["parents"][&id].fields["name"],
            Value::String("optimistic".into())
        );
        assert!(!ledger.records[&key].reconciled);
    }
    #[test]
    fn e06_typed_correction_clears_only_optimistic_tombstones_and_guards_metadata() {
        use crate::command::EntityOperation;
        let (mut ledger, rows, key, id) = correction_fixture(EntityOperation::Delete);
        let planned = reconcile(
            &ledger,
            &rows,
            &correction_schema(),
            &key,
            ReconcilePlan {
                writes: vec![ReconcileWrite::Entity {
                    change: typed_change(&id),
                }],
                ..Default::default()
            },
        )
        .expect("complete corrected row may clear optimistic delete");
        assert!(planned.rows["parents"].contains_key(&id));
        assert!(
            planned.ledger.entity.as_ref().unwrap().metadata[&("parents".into(), id.clone())]
                .tombstone
                .is_none()
        );
        let mut incomplete = typed_change(&id);
        incomplete.changes.remove("name");
        incomplete.field_timestamps.remove("name");
        assert!(
            reconcile(
                &ledger,
                &rows,
                &correction_schema(),
                &key,
                ReconcilePlan {
                    writes: vec![ReconcileWrite::Entity {
                        change: incomplete
                    }],
                    ..Default::default()
                }
            )
            .is_err(),
            "clearing optimistic tombstone requires a schema-complete row"
        );
        ledger
            .entity
            .as_mut()
            .unwrap()
            .metadata
            .get_mut(&("parents".into(), id.clone()))
            .unwrap()
            .tombstone
            .as_mut()
            .unwrap()
            .authoritative = true;
        assert!(
            matches!(
                reconcile(
                    &ledger,
                    &rows,
                    &correction_schema(),
                    &key,
                    ReconcilePlan {
                        writes: vec![ReconcileWrite::Entity {
                            change: typed_change(&id)
                        }],
                        ..Default::default()
                    }
                ),
                Err(CommandError::RollbackConflict)
            ),
            "independent comparison change must fail before correction"
        );
        ledger.records.get_mut(&key).unwrap().metadata_undo[0]
            .after
            .as_mut()
            .unwrap()
            .tombstone
            .as_mut()
            .unwrap()
            .authoritative = true;
        assert!(
            matches!(
                reconcile(
                    &ledger,
                    &rows,
                    &correction_schema(),
                    &key,
                    ReconcilePlan {
                        writes: vec![ReconcileWrite::Entity {
                            change: typed_change(&id)
                        }],
                        ..Default::default()
                    }
                ),
                Err(CommandError::InvalidCommand)
            ),
            "confirmed deletion cannot be resurrected"
        );
    }

    #[test]
    fn e06_typed_delete_stamps_exact_authority_and_requires_entity_opt_in() {
        use crate::command::{EntityChange, EntityOperation};
        let (ledger, rows, key, id) = correction_fixture(EntityOperation::Update);
        let deletion = EntityChange {
            table: "parents".into(),
            record_id: id.clone(),
            op: EntityOperation::Delete,
            changes: BTreeMap::new(),
            removed_fields: BTreeSet::new(),
            timestamp_ms: 50,
            field_timestamps: BTreeMap::new(),
        };
        let planned = reconcile(
            &ledger,
            &rows,
            &correction_schema(),
            &key,
            ReconcilePlan {
                writes: vec![ReconcileWrite::Entity {
                    change: deletion.clone(),
                }],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!planned.rows["parents"].contains_key(&id));
        assert_eq!(
            planned.ledger.entity.as_ref().unwrap().metadata[&("parents".into(), id)].tombstone,
            Some(crate::command::FieldStamp {
                timestamp_ms: 50,
                authoritative: true
            })
        );
        let (command, command_rows, key, _, _) = mapped_fixture();
        let mut command_plan = plan("local:a", "server:a");
        command_plan.writes.push(ReconcileWrite::Entity {
            change: deletion,
        });
        assert!(
            matches!(
                reconcile(&command, &command_rows, &schema(), &key, command_plan),
                Err(CommandError::InvalidCommand)
            ),
            "typed clocks require opt-in, command-only legacy corrections stay independent"
        );
    }

    // SPEC-0038 C1a / TC-0042 C1a-SAME (pure planner: the public API elides an identical
    // patch): a later record whose images hold the accepted value with the same timestamp is
    // promoted, like the current cell.
    #[test]
    fn c1a_same_value_equal_clock_images_are_promoted() {
        use crate::command::{EntityOperation, entity::*};
        let (mut ledger, rows, key, id) = correction_fixture(EntityOperation::Update);
        let accepted = ledger.records[&key].clone();
        let mut later = accepted.clone();
        later.key = format!("0000000000000064-{}-{:016x}", "e".repeat(32), 2);
        later.order = 2;
        later.state = SyncState::Queued;
        later.outcome = None;
        later.reconciled = false;
        later.frozen = None;
        later.attempts = 0;
        later.due_ms = 0;
        let after_row = accepted.undo[0].after.clone();
        later.undo[0].before = after_row.clone();
        later.undo[0].after = after_row;
        let after_meta = accepted.metadata_undo[0].after.clone();
        later.metadata_undo[0].before = after_meta.clone();
        later.metadata_undo[0].after = after_meta;
        let later_key = later.key.clone();
        ledger.records.insert(later_key.clone(), later);
        ledger.header.sequence = ledger.header.sequence.max(3);
        ledger.validate().unwrap();
        let planned = reconcile(
            &ledger,
            &rows,
            &correction_schema(),
            &key,
            ReconcilePlan::default(),
        )
        .unwrap();
        let promoted = FieldStamp {
            timestamp_ms: 100,
            authoritative: true,
        };
        let images = &planned.ledger.records[&later_key].metadata_undo[0];
        for meta in [
            images.before.as_ref().unwrap(),
            images.after.as_ref().unwrap(),
        ] {
            assert_eq!(meta.fields["name"], promoted);
        }
        let current = &planned.ledger.entity.as_ref().unwrap().metadata[&("parents".into(), id)];
        assert_eq!(current.fields["name"], promoted);
    }

    // SPEC-0038 C1a / TC-0042 C1a-CTRL: a target without a stamp is never promoted, and no stamp
    // (not even timestamp 0) is synthesized for it.
    #[test]
    fn c1a_missing_stamp_is_not_synthesized() {
        use crate::command::EntityOperation;
        let (mut ledger, rows, key, id) = correction_fixture(EntityOperation::Update);
        let identity = ("parents".to_string(), id.clone());
        ledger
            .entity
            .as_mut()
            .unwrap()
            .metadata
            .get_mut(&identity)
            .unwrap()
            .fields
            .remove("drop");
        ledger.validate().unwrap();
        let planned = reconcile(
            &ledger,
            &rows,
            &correction_schema(),
            &key,
            ReconcilePlan::default(),
        )
        .unwrap();
        let current = &planned.ledger.entity.as_ref().unwrap().metadata[&identity];
        assert!(!current.fields.contains_key("drop"));
        assert!(current.fields["name"].authoritative);
    }

    // SPEC-0038 C1a-PAIR / TC-0042 C1a-PAIR: promotion evidence for an identity needs exactly one
    // matching row-undo entry and exactly one matching metadata-undo entry. A missing or
    // duplicated entry is unknown, not explicit absence. Pure planner fixtures (validated first).
    fn pair_fixture() -> (SyncLedger, Snapshot, String, (String, String)) {
        use crate::command::EntityOperation;
        let (ledger, rows, key, id) = correction_fixture(EntityOperation::Delete);
        (ledger, rows, key, ("parents".to_string(), id))
    }

    /// A pending later entity record on the same identity, cloned from the accepted Delete.
    fn pair_later(ledger: &mut SyncLedger, key: &str) -> String {
        let mut later = ledger.records[key].clone();
        later.key = format!("0000000000000064-{}-{:016x}", "e".repeat(32), 2);
        later.order = 2;
        later.state = SyncState::Queued;
        later.outcome = None;
        later.reconciled = false;
        later.frozen = None;
        later.attempts = 0;
        later.due_ms = 0;
        ledger.header.sequence = ledger.header.sequence.max(3);
        let later_key = later.key.clone();
        ledger.records.insert(later_key.clone(), later);
        later_key
    }

    fn pair_confirm(ledger: &SyncLedger, rows: &Snapshot, key: &str) -> SyncLedger {
        ledger.validate().unwrap();
        reconcile(
            ledger,
            rows,
            &correction_schema(),
            key,
            ReconcilePlan::default(),
        )
        .unwrap()
        .ledger
    }

    fn tombstone_authority(meta: Option<&crate::command::entity::RowMetadata>) -> Option<bool> {
        meta.and_then(|m| m.tombstone.as_ref())
            .map(|t| t.authoritative)
    }

    fn current_tombstone(ledger: &SyncLedger, identity: &(String, String)) -> Option<bool> {
        tombstone_authority(ledger.entity.as_ref().unwrap().metadata.get(identity))
    }

    #[test]
    fn c1a_pair_accepted_explicit_after_none_promotes_the_tombstone() {
        let (ledger, rows, key, identity) = pair_fixture();
        assert_eq!(ledger.records[&key].undo.len(), 1);
        assert!(
            ledger.records[&key].undo[0].after.is_none(),
            "explicit absence"
        );
        let next = pair_confirm(&ledger, &rows, &key);
        assert_eq!(current_tombstone(&next, &identity), Some(true));
    }

    #[test]
    fn c1a_pair_accepted_missing_row_undo_promotes_nothing() {
        let (mut ledger, rows, key, identity) = pair_fixture();
        ledger.records.get_mut(&key).unwrap().undo.clear();
        let next = pair_confirm(&ledger, &rows, &key);
        assert_eq!(
            current_tombstone(&next, &identity),
            Some(false),
            "missing is unknown"
        );
    }

    #[test]
    fn c1a_pair_accepted_duplicate_row_undo_promotes_nothing() {
        let (mut ledger, rows, key, identity) = pair_fixture();
        let record = ledger.records.get_mut(&key).unwrap();
        record.undo.push(record.undo[0].clone());
        let next = pair_confirm(&ledger, &rows, &key);
        assert_eq!(
            current_tombstone(&next, &identity),
            Some(false),
            "duplicate is ambiguous"
        );
    }

    #[test]
    fn c1a_pair_accepted_duplicate_metadata_undo_promotes_nothing() {
        let (mut ledger, rows, key, identity) = pair_fixture();
        let record = ledger.records.get_mut(&key).unwrap();
        record.metadata_undo.push(record.metadata_undo[0].clone());
        let next = pair_confirm(&ledger, &rows, &key);
        assert_eq!(
            current_tombstone(&next, &identity),
            Some(false),
            "duplicate is ambiguous"
        );
    }

    /// An unproven pairing on another record skips only that record; the current metadata still
    /// promotes from the accepted record's unique pair.
    fn pair_other_unproven(spoil: impl Fn(&mut SyncRecord)) {
        let (mut ledger, rows, key, identity) = pair_fixture();
        let later = pair_later(&mut ledger, &key);
        spoil(ledger.records.get_mut(&later).unwrap());
        let original = ledger.records[&later].metadata_undo.clone();
        assert!(
            original
                .iter()
                .any(|meta| tombstone_authority(meta.after.as_ref()) == Some(false)),
            "setup: an unpromoted matching tombstone image exists"
        );
        let next = pair_confirm(&ledger, &rows, &key);
        assert_eq!(
            current_tombstone(&next, &identity),
            Some(true),
            "current still promotes"
        );
        assert_eq!(
            next.records[&later].metadata_undo, original,
            "the unproven record's images stay exactly unchanged"
        );
    }

    #[test]
    fn c1a_pair_other_missing_row_undo_skips_only_that_record() {
        pair_other_unproven(|record| record.undo.clear());
    }

    #[test]
    fn c1a_pair_other_duplicate_row_undo_skips_only_that_record() {
        pair_other_unproven(|record| record.undo.push(record.undo[0].clone()));
    }

    #[test]
    fn c1a_pair_other_duplicate_metadata_undo_skips_only_that_record() {
        pair_other_unproven(|record| record.metadata_undo.push(record.metadata_undo[0].clone()));
    }

    /// Another record's unique entry with an explicit `before: None` row stays eligible: its
    /// matching tombstone image is promoted.
    #[test]
    fn c1a_pair_other_unique_before_none_stays_eligible() {
        let (mut ledger, rows, key, identity) = pair_fixture();
        let later = pair_later(&mut ledger, &key);
        let record = ledger.records.get_mut(&later).unwrap();
        record.undo[0].before = None;
        let tombstone = record.metadata_undo[0].after.clone();
        record.metadata_undo[0].before = tombstone;
        let next = pair_confirm(&ledger, &rows, &key);
        assert_eq!(current_tombstone(&next, &identity), Some(true));
        let meta = &next.records[&later].metadata_undo[0];
        assert_eq!(
            tombstone_authority(meta.before.as_ref()),
            Some(true),
            "explicit None eligible"
        );
    }

    // SPEC-0038 C4 / TC-0042 pure planner cases (EO14, EO15, E5, the shape refusals). The public
    // path is blocked by pull protection (pull.rs `protected`) and by every entity-table write
    // creating a record, so these synthetic external changes are reachable only here. The
    // fixture is a Corrected entity A (an Update declaring `name` and `drop`, or a Create) with
    // pending later entity Updates of the same row; every fixture passes `ledger.validate()`.
    fn c4_fixture(
        op: crate::command::EntityOperation,
        external_other: bool,
        later_names: &[(&str, i64)],
    ) -> (SyncLedger, Snapshot, String, String, Vec<String>) {
        use crate::command::{EntityOperation, entity::*};
        let (mut ledger, _, key, id) = correction_fixture(op);
        let identity = ("parents".to_string(), id.clone());
        let accepted = ledger.records[&key].clone();
        let mut row = accepted.undo[0].after.clone().unwrap();
        let mut meta = accepted.metadata_undo[0].after.clone().unwrap();
        if external_other {
            // An accepted external change to a field A did not declare, already in B.
            row.fields
                .insert("other".into(), Value::String("external".into()));
            meta.fields.insert(
                "other".into(),
                FieldStamp {
                    timestamp_ms: 150,
                    authoritative: true,
                },
            );
        }
        let mut keys = vec![];
        for (order, (value, time)) in later_names.iter().enumerate() {
            let order = order as i64 + 2;
            let mut later = accepted.clone();
            later.key = format!("{:016x}-{}-{order:016x}", *time, "e".repeat(32));
            later.order = order;
            later.state = SyncState::Sent;
            later.attempts = 1;
            later.outcome = None;
            later.reconciled = false;
            later.due_ms = 0;
            later.created.clear();
            // A later write on a locally created row depends on its producer (closure(K)).
            later.reference_paths = vec![vec![PathPart::Key("recordId".into())]];
            later.dependencies = if op == EntityOperation::Create {
                BTreeSet::from([id.clone()])
            } else {
                BTreeSet::new()
            };
            later.args = Json::Object(BTreeMap::from([
                ("recordId".into(), Json::String(id.clone())),
                (
                    "changes".into(),
                    Json::Object(BTreeMap::from([(
                        "name".into(),
                        Json::String((*value).into()),
                    )])),
                ),
            ]));
            let descriptor = later.entity.as_mut().unwrap();
            descriptor.op = EntityOperation::Update;
            descriptor.timestamp_ms = *time;
            // An entity record's captured context time is bound to its descriptor time.
            later.context = Json::Object(BTreeMap::from([
                ("identity".into(), Json::Null),
                (
                    "capturedAt".into(),
                    Json::String(super::super::owner::utc_time(*time)),
                ),
            ]));
            // A dependent is Queued: a sent (frozen) record carries no dependencies.
            if op == EntityOperation::Create {
                later.state = SyncState::Queued;
                later.attempts = 0;
                later.frozen = None;
            } else {
                later.frozen = Some(later.envelope().canonical().unwrap());
            }
            let before_row = row.clone();
            let before_meta = meta.clone();
            row.fields
                .insert("name".into(), Value::String((*value).into()));
            meta.fields.insert(
                "name".into(),
                FieldStamp {
                    timestamp_ms: *time,
                    authoritative: false,
                },
            );
            later.undo[0].before = Some(before_row);
            later.undo[0].after = Some(row.clone());
            later.metadata_undo[0].before = Some(before_meta);
            later.metadata_undo[0].after = Some(meta.clone());
            keys.push(later.key.clone());
            ledger.header.sequence = ledger.header.sequence.max(order + 1);
            ledger.header.last_ms = ledger.header.last_ms.max(*time);
            ledger.records.insert(later.key.clone(), later);
        }
        ledger
            .entity
            .as_mut()
            .unwrap()
            .metadata
            .insert(identity, meta);
        ledger.validate().unwrap();
        let rows = Snapshot::from([
            ("parents".into(), BTreeMap::from([(id.clone(), row)])),
            ("children".into(), BTreeMap::new()),
        ]);
        (ledger, rows, key, id, keys)
    }

    fn c4_typed(id: &str, field: &str, value: &str, time: i64) -> ReconcilePlan {
        use crate::command::EntityOperation;
        ReconcilePlan {
            mappings: BTreeMap::new(),
            writes: vec![ReconcileWrite::Entity {
                change: crate::command::EntityChange {
                    table: "parents".into(),
                    record_id: id.into(),
                    op: EntityOperation::Update,
                    changes: BTreeMap::from([(field.into(), Value::String(value.into()))]),
                    removed_fields: BTreeSet::new(),
                    timestamp_ms: time,
                    field_timestamps: BTreeMap::from([(field.into(), time)]),
                },
            }],
        }
    }

    /// A whole `parents` image: exactly these fields (`name`, `other`, `drop`) and stamps.
    fn c4_image(
        id: &str,
        [name, other, drop]: [(&str, i64, bool); 3],
    ) -> (Row, super::super::entity::RowMetadata) {
        use super::super::entity::*;
        let cells = [("name", name), ("other", other), ("drop", drop)];
        (
            Row {
                id: id.into(),
                fields: cells
                    .iter()
                    .map(|(field, (value, _, _))| {
                        (field.to_string(), Value::String((*value).into()))
                    })
                    .collect(),
            },
            RowMetadata {
                fields: cells
                    .iter()
                    .map(|(field, (_, time, authoritative))| {
                        (
                            field.to_string(),
                            FieldStamp {
                                timestamp_ms: *time,
                                authoritative: *authoritative,
                            },
                        )
                    })
                    .collect(),
                tombstone: None,
            },
        )
    }

    /// A pending record's single change pair, compared whole, and every other field of the record
    /// byte-identical to the input (frozen, args, descriptor, key, order, attempts, state, context).
    fn c4_rebased(
        input: &SyncLedger,
        planned: &Planned,
        key: &str,
        id: &str,
        before: (Row, super::super::entity::RowMetadata),
        after: (Row, super::super::entity::RowMetadata),
    ) {
        let record = &planned.ledger.records[key];
        assert_eq!((record.undo.len(), record.metadata_undo.len()), (1, 1));
        assert_eq!(
            (&record.undo[0].table, &record.undo[0].id),
            (&"parents".to_string(), &id.to_string())
        );
        assert_eq!(
            (
                record.undo[0].before.clone(),
                record.metadata_undo[0].before.clone()
            ),
            (Some(before.0), Some(before.1)),
            "{key}: whole before image"
        );
        assert_eq!(
            (
                record.undo[0].after.clone(),
                record.metadata_undo[0].after.clone()
            ),
            (Some(after.0), Some(after.1)),
            "{key}: whole after image"
        );
        let mut expected = input.records[key].clone();
        expected.undo = record.undo.clone();
        expected.metadata_undo = record.metadata_undo.clone();
        assert!(
            expected == *record,
            "{key}: only undo and metadata undo change"
        );
    }

    /// On C4 success every record outside K and L is byte-identical, and none is added or removed.
    fn c4_others(input: &SyncLedger, planned: &Planned, key: &str, keys: &[String]) {
        assert!(input.records.keys().eq(planned.ledger.records.keys()));
        for (other, record) in &input.records {
            if other != key && !keys.contains(other) {
                assert!(
                    planned.ledger.records[other] == *record,
                    "{other}: identical"
                );
            }
        }
    }

    /// The current row and metadata of W, compared whole, and K's transition.
    fn c4_current(
        planned: &Planned,
        key: &str,
        id: &str,
        image: (Row, super::super::entity::RowMetadata),
    ) {
        assert_eq!(
            (
                planned.rows["parents"].get(id).cloned(),
                planned.ledger.entity.as_ref().unwrap().metadata
                    [&("parents".to_string(), id.to_string())]
                    .clone()
            ),
            (Some(image.0), image.1),
            "whole current row and metadata"
        );
        assert!(planned.ledger.records[key].reconciled);
    }

    /// A validated fixture refuses with the closure-proof mismatch's RollbackConflict (a typed
    /// correction, or the rollback when `plan` is None); the input ledger and rows are preserved.
    fn c4_refuses(ledger: &SyncLedger, rows: &Snapshot, key: &str, plan: Option<ReconcilePlan>) {
        ledger.validate().unwrap();
        let (ledger_before, rows_before) = (ledger.clone(), rows.clone());
        let result = match plan {
            Some(plan) => reconcile(ledger, rows, &correction_schema(), key, plan),
            None => inverse(ledger, rows, &correction_schema(), key, false),
        };
        match result {
            Err(error) => assert_eq!(error, CommandError::RollbackConflict),
            Ok(_) => panic!("the ineligible shape refuses"),
        }
        assert!(
            *ledger == ledger_before && *rows == rows_before,
            "the input ledger and rows are preserved"
        );
    }

    // EO14 (RED for C4): an accepted external change to an untouched field (`other`, 150
    // authoritative) already in B is preserved exactly in eff0, in every pending image and in the
    // current row and metadata. The uncorrected declared `drop` is promoted by C1a into eff0.
    #[test]
    fn c4_eo14_untouched_external_change_is_preserved() {
        use crate::command::EntityOperation;
        let (ledger, rows, key, id, keys) =
            c4_fixture(EntityOperation::Update, true, &[("pending", 200)]);
        let plan = c4_typed(&id, "name", "backend", 150);
        let planned = reconcile(&ledger, &rows, &correction_schema(), &key, plan)
            .expect("C4 re-bases the pending writer");
        let external = ("external", 150, true);
        let drop = ("prior", 100, true);
        let pending = c4_image(&id, [("pending", 200, false), external, drop]);
        c4_rebased(
            &ledger,
            &planned,
            &keys[0],
            &id,
            c4_image(&id, [("backend", 150, true), external, drop]),
            pending.clone(),
        );
        c4_current(&planned, &key, &id, pending);
        c4_others(&ledger, &planned, &key, &keys);
    }

    // EO15 (characterization): an external change to a correction-only field A did not declare
    // makes B(other) differ from A.after(other), so the action refuses (E6).
    #[test]
    fn c4_eo15_external_correction_only_field_refuses() {
        use crate::command::EntityOperation;
        let (ledger, rows, key, id, _) =
            c4_fixture(EntityOperation::Update, true, &[("pending", 200)]);
        c4_refuses(
            &ledger,
            &rows,
            &key,
            Some(c4_typed(&id, "other", "backend", 150)),
        );
    }

    // EO15 positive twin (RED for C4): the same correction-only field, unchanged since A, is
    // eligible; the uncorrected declared fields are promoted by C1a into eff0.
    #[test]
    fn c4_eo15_twin_unchanged_correction_only_field_rebases() {
        use crate::command::EntityOperation;
        let (ledger, rows, key, id, keys) =
            c4_fixture(EntityOperation::Update, false, &[("pending", 200)]);
        let plan = c4_typed(&id, "other", "backend", 150);
        let planned = reconcile(&ledger, &rows, &correction_schema(), &key, plan)
            .expect("C4 re-bases the pending writer");
        let other = ("backend", 150, true);
        let drop = ("prior", 100, true);
        let pending = c4_image(&id, [("pending", 200, false), other, drop]);
        c4_rebased(
            &ledger,
            &planned,
            &keys[0],
            &id,
            c4_image(&id, [("optimistic", 100, true), other, drop]),
            pending.clone(),
        );
        c4_current(&planned, &key, &id, pending);
        c4_others(&ledger, &planned, &key, &keys);
    }

    // E5 positive twin (RED for C4): an unbroken two-record chain is re-based whole; the break
    // variants below differ from this fixture in exactly one cell or flag.
    #[test]
    fn c4_e5_twin_unbroken_chain_rebases() {
        use crate::command::EntityOperation;
        let (ledger, rows, key, id, keys) = c4_fixture(
            EntityOperation::Update,
            false,
            &[("first", 200), ("second", 300)],
        );
        let plan = c4_typed(&id, "name", "backend", 150);
        let planned = reconcile(&ledger, &rows, &correction_schema(), &key, plan)
            .expect("C4 re-bases both pending writers");
        let other = ("untouched", 20, true);
        let drop = ("prior", 100, true);
        let first = c4_image(&id, [("first", 200, false), other, drop]);
        let second = c4_image(&id, [("second", 300, false), other, drop]);
        c4_rebased(
            &ledger,
            &planned,
            &keys[0],
            &id,
            c4_image(&id, [("backend", 150, true), other, drop]),
            first.clone(),
        );
        c4_rebased(&ledger, &planned, &keys[1], &id, first, second.clone());
        c4_current(&planned, &key, &id, second);
        c4_others(&ledger, &planned, &key, &keys);
    }

    /// E6 stamp-only (characterization): B keeps A.after's owned value, but its FieldStamp differs
    /// in one part. B is L.before, so the E5 chain and the current row still agree with L.after
    /// and only E6 can refuse.
    fn c4_e6_stamp(field: &str, change: impl Fn(&mut super::super::entity::FieldStamp)) {
        use crate::command::EntityOperation;
        let (mut ledger, rows, key, id, keys) =
            c4_fixture(EntityOperation::Update, false, &[("pending", 200)]);
        let later = ledger.records.get_mut(&keys[0]).unwrap();
        change(
            later.metadata_undo[0]
                .before
                .as_mut()
                .unwrap()
                .fields
                .get_mut(field)
                .unwrap(),
        );
        // Pins: the B and A.after rows are equal, and the metadata differ only in this stamp.
        let (later, accepted) = (&ledger.records[&keys[0]], &ledger.records[&key]);
        assert_eq!(later.undo[0].before, accepted.undo[0].after);
        let base = later.metadata_undo[0].before.as_ref().unwrap();
        let owned = accepted.metadata_undo[0].after.as_ref().unwrap();
        assert_ne!(base.fields[field], owned.fields[field]);
        let mut aligned = base.clone();
        aligned
            .fields
            .insert(field.into(), owned.fields[field].clone());
        assert_eq!(&aligned, owned, "only the one stamp differs");
        assert_eq!(rows["parents"].get(&id), later.undo[0].after.as_ref());
        assert_eq!(
            ledger
                .entity
                .as_ref()
                .unwrap()
                .metadata
                .get(&("parents".to_string(), id.clone())),
            later.metadata_undo[0].after.as_ref(),
            "current equals L.after"
        );
        c4_refuses(
            &ledger,
            &rows,
            &key,
            Some(c4_typed(&id, "name", "backend", 150)),
        );
    }

    // E6: the corrected owned `name` keeps A.after's value with a different timestamp.
    #[test]
    fn c4_e6_owned_stamp_timestamp_differs_refuses() {
        c4_e6_stamp("name", |stamp| stamp.timestamp_ms = 99);
    }

    // E6: the declared uncorrected `drop` keeps A.after's value and timestamp; only the flag differs.
    #[test]
    fn c4_e6_owned_stamp_flag_differs_refuses() {
        c4_e6_stamp("drop", |stamp| stamp.authoritative = true);
    }

    /// The E5 twin fixture with one break applied (characterization: each refuses).
    fn c4_broken(mut brk: impl FnMut(&mut SyncLedger, &mut Snapshot, &str, &[String])) {
        use crate::command::EntityOperation;
        let (mut ledger, mut rows, key, id, keys) = c4_fixture(
            EntityOperation::Update,
            false,
            &[("first", 200), ("second", 300)],
        );
        brk(&mut ledger, &mut rows, &id, &keys);
        c4_refuses(
            &ledger,
            &rows,
            &key,
            Some(c4_typed(&id, "name", "backend", 150)),
        );
    }

    // Chain break (characterization): L1.after differs from L2.before in a row cell (E5).
    #[test]
    fn c4_chain_break_refuses() {
        c4_broken(|ledger, _, _, keys| {
            ledger.records.get_mut(&keys[1]).unwrap().undo[0]
                .before
                .as_mut()
                .unwrap()
                .fields
                .insert("other".into(), Value::String("broken".into()));
        });
    }

    // Metadata flag-only chain break (characterization): L2.before(name) differs from
    // L1.after(name) only in the authoritative flag (E5 compares whole metadata).
    #[test]
    fn c4_chain_metadata_flag_break_refuses() {
        c4_broken(|ledger, _, _, keys| {
            ledger.records.get_mut(&keys[1]).unwrap().metadata_undo[0]
                .before
                .as_mut()
                .unwrap()
                .fields
                .get_mut("name")
                .unwrap()
                .authoritative = true;
        });
    }

    // Current row differs from L_last.after (characterization, E5).
    #[test]
    fn c4_current_row_mismatch_refuses() {
        c4_broken(|_, rows, id, _| {
            rows.get_mut("parents")
                .unwrap()
                .get_mut(id)
                .unwrap()
                .fields
                .insert("other".into(), Value::String("drift".into()));
        });
    }

    // Current metadata differs from L_last.after only in a flag (characterization, E5).
    #[test]
    fn c4_current_metadata_flag_mismatch_refuses() {
        c4_broken(|ledger, _, id, _| {
            ledger
                .entity
                .as_mut()
                .unwrap()
                .metadata
                .get_mut(&("parents".to_string(), id.to_string()))
                .unwrap()
                .fields
                .get_mut("name")
                .unwrap()
                .authoritative = true;
        });
    }

    // Current tombstone metadata on W (characterization, E4).
    #[test]
    fn c4_current_tombstone_refuses() {
        use super::super::entity::FieldStamp;
        c4_broken(|ledger, _, id, _| {
            ledger
                .entity
                .as_mut()
                .unwrap()
                .metadata
                .get_mut(&("parents".to_string(), id.to_string()))
                .unwrap()
                .tombstone = Some(FieldStamp {
                timestamp_ms: 350,
                authoritative: false,
            });
        });
    }

    /// The Create K correction plan: the identity mapping its created set requires, plus the
    /// typed correction of the created row.
    fn c4_create_plan(id: &str) -> ReconcilePlan {
        let mut plan = c4_typed(id, "name", "backend", 150);
        plan.mappings = BTreeMap::from([(id.to_string(), id.to_string())]);
        plan
    }

    /// The ordinary path accepts the same validated fixture without the synthetic mismatch, so
    /// the paired refusal comes from that mismatch (non-vacuity control on unchanged code).
    fn c4_ordinary(ledger: &SyncLedger, rows: &Snapshot, key: &str, plan: Option<ReconcilePlan>) {
        let result = match plan {
            Some(plan) => reconcile(ledger, rows, &correction_schema(), key, plan),
            None => inverse(ledger, rows, &correction_schema(), key, false),
        };
        assert!(
            result.is_ok(),
            "control: the ordinary path proves the closure"
        );
    }

    // Create K with a synthetic current mismatch (characterization, shape and E4).
    #[test]
    fn c4_create_k_synthetic_mismatch_refuses() {
        use crate::command::EntityOperation;
        let (ledger, mut rows, key, id, keys) = c4_fixture(EntityOperation::Create, false, &[]);
        assert!(keys.is_empty());
        c4_ordinary(&ledger, &rows, &key, Some(c4_create_plan(&id)));
        rows.get_mut("parents")
            .unwrap()
            .get_mut(&id)
            .unwrap()
            .fields
            .insert("other".into(), Value::String("drift".into()));
        c4_refuses(&ledger, &rows, &key, Some(c4_create_plan(&id)));
    }

    // Create K with a dependent L in its closure and a synthetic current mismatch
    // (characterization: a closure of more than one record).
    #[test]
    fn c4_create_k_dependent_closure_refuses() {
        use crate::command::EntityOperation;
        let (ledger, mut rows, key, id, keys) =
            c4_fixture(EntityOperation::Create, false, &[("pending", 200)]);
        assert_eq!(
            closure(&ledger, &key).unwrap(),
            BTreeSet::from([key.clone(), keys[0].clone()]),
            "setup: L is in closure(K)"
        );
        c4_ordinary(&ledger, &rows, &key, Some(c4_create_plan(&id)));
        rows.get_mut("parents")
            .unwrap()
            .get_mut(&id)
            .unwrap()
            .fields
            .insert("other".into(), Value::String("drift".into()));
        c4_refuses(&ledger, &rows, &key, Some(c4_create_plan(&id)));
    }

    // A processed rejected Delete P with a synthetic current row (characterization: a delete in K).
    #[test]
    fn c4_rejected_delete_p_refuses() {
        use crate::command::EntityOperation;
        let (mut ledger, mut rows, key, id) = correction_fixture(EntityOperation::Delete);
        let record = ledger.records.get_mut(&key).unwrap();
        record.state = SyncState::Rejected;
        record.outcome = Some(RemoteOutcome::Rejected("no".into()));
        record.reconciled = true;
        ledger.validate().unwrap();
        c4_ordinary(&ledger, &rows, &key, None);
        rows.get_mut("parents")
            .unwrap()
            .insert(id.clone(), row(&id, "external"));
        c4_refuses(&ledger, &rows, &key, None);
    }

    // Chain break on the rollback path (characterization): the same E5 break, P rejected and
    // processed instead of A Corrected.
    #[test]
    fn c4_rollback_chain_break_refuses() {
        use crate::command::EntityOperation;
        let (mut ledger, rows, key, _, keys) = c4_fixture(
            EntityOperation::Update,
            false,
            &[("first", 200), ("second", 300)],
        );
        let record = ledger.records.get_mut(&key).unwrap();
        record.state = SyncState::Rejected;
        record.outcome = Some(RemoteOutcome::Rejected("no".into()));
        record.reconciled = true;
        ledger.records.get_mut(&keys[1]).unwrap().undo[0]
            .before
            .as_mut()
            .unwrap()
            .fields
            .insert("other".into(), Value::String("broken".into()));
        c4_refuses(&ledger, &rows, &key, None);
    }

    // SPEC-0037 R4 counter exactness of the C4 fallback: one direct `fallback::rebase_in` call on the
    // rollback path (no correction, so no resolve_entity work) with the counters reset around it.
    // metadata_entries_visited terms: K's two images (2), each examined L record's two images (2
    // each), current W (1), one RowMetadata comparison per E5 window whose rows are equal and
    // one for current against L_last.after when their rows are equal (the image comparison
    // short-circuits on the row), plus exactly the metadata-undo entries `touches` examined for
    // every other live record, plus one metadata first-image visit on success.
    fn c4_rollback_counted(ledger: &SyncLedger, rows: &Snapshot, key: &str) -> (bool, u64) {
        use crate::storage::counters;
        let mut candidate = ledger.clone();
        let mut txn = LedgerTxn::new(&mut candidate, false);
        counters::reset();
        let rebased = fallback::rebase_in(
            &mut txn,
            rows,
            &correction_schema(),
            key,
            fallback::Path::Rollback,
            &Hooks::new(),
        );
        if rebased.is_none() {
            assert!(
                txn.record_keys().is_empty(),
                "C4 refusal captures no record first image"
            );
            assert!(
                txn.metadata_keys().is_empty(),
                "C4 refusal captures no metadata first image"
            );
        }
        (
            rebased.is_some(),
            counters::snapshot().metadata_entries_visited,
        )
    }

    fn c4_processed_rejection(ledger: &mut SyncLedger, key: &str) {
        let record = ledger.records.get_mut(key).unwrap();
        record.state = SyncState::Rejected;
        record.outcome = Some(RemoteOutcome::Rejected("no".into()));
        record.reconciled = true;
    }

    /// Moves a pending later record onto another row, so it is live but does not touch W; W's
    /// current state becomes K.after again. The result is validated.
    fn c4_retarget(ledger: &mut SyncLedger, rows: &mut Snapshot, key: &str, later: &str, id: &str) {
        let other = "server:other";
        let record = ledger.records.get_mut(later).unwrap();
        record.undo[0].id = other.into();
        let change = &mut record.undo[0];
        for row in [&mut change.before, &mut change.after] {
            row.as_mut().unwrap().id = other.into();
        }
        record.metadata_undo[0].record_id = other.into();
        if let Json::Object(args) = &mut record.args {
            args.insert("recordId".into(), Json::String(other.into()));
        }
        record.frozen = Some(record.envelope().canonical().unwrap());
        let (other_row, other_meta) = (
            record.undo[0].after.clone().unwrap(),
            record.metadata_undo[0].after.clone().unwrap(),
        );
        let accepted = &ledger.records[key];
        let (row, meta) = (
            accepted.undo[0].after.clone().unwrap(),
            accepted.metadata_undo[0].after.clone().unwrap(),
        );
        let entity = ledger.entity.as_mut().unwrap();
        entity
            .metadata
            .insert(("parents".to_string(), other.to_string()), other_meta);
        entity
            .metadata
            .insert(("parents".to_string(), id.to_string()), meta);
        let parents = rows.get_mut("parents").unwrap();
        parents.insert(other.into(), other_row);
        parents.insert(id.into(), row);
        ledger.validate().unwrap();
    }

    // Row-undo matches (RED on the up-front count): both L records touch W through their row
    // undo, so `touches` examines no metadata. 2 (K) + 0 + 4 (L1, L2) + 1 (current) + 1 (E5
    // window) + 1 (current against L2.after) + 1 (rollback first image) = 10.
    #[test]
    fn c4_counter_row_undo_match_examines_no_touch_metadata() {
        use crate::command::EntityOperation;
        let (mut ledger, rows, key, _, _) = c4_fixture(
            EntityOperation::Update,
            false,
            &[("first", 200), ("second", 300)],
        );
        c4_processed_rejection(&mut ledger, &key);
        ledger.validate().unwrap();
        assert_eq!(c4_rollback_counted(&ledger, &rows, &key), (true, 10));
    }

    // A metadata match before the end (RED on the up-front count): a live record whose row undo
    // is elsewhere but whose metadata undo lists W first and another row second. `touches`
    // stops after the first entry; the record then fails the single-pair shape before its images
    // are examined, so the fallback refuses. 2 (K) + 1 = 3. No writer produces this shape, but
    // the ledger validator accepts it (asserted below), so the count is exercised on a valid
    // ledger.
    #[test]
    fn c4_counter_metadata_match_stops_before_the_end() {
        use crate::command::EntityOperation;
        let (mut ledger, mut rows, key, id, keys) =
            c4_fixture(EntityOperation::Update, false, &[("other", 200)]);
        c4_processed_rejection(&mut ledger, &key);
        c4_retarget(&mut ledger, &mut rows, &key, &keys[0], &id);
        let record = ledger.records.get_mut(&keys[0]).unwrap();
        let mut on_w = record.metadata_undo[0].clone();
        on_w.record_id = id.clone();
        record.metadata_undo.insert(0, on_w);
        assert_eq!(
            ledger.validate(),
            Ok(()),
            "the validator accepts the unpaired shape"
        );
        assert_eq!(c4_rollback_counted(&ledger, &rows, &key), (false, 3));
    }

    // No match (characterization, equal before and after the fix): a live record on another row
    // has its single metadata-undo entry examined; L is empty. 2 (K) + 1 + 1 (current)
    // + 1 (rollback first image) = 5.
    #[test]
    fn c4_counter_no_match_examines_every_touch_metadata_entry() {
        use crate::command::EntityOperation;
        let (mut ledger, mut rows, key, id, keys) =
            c4_fixture(EntityOperation::Update, false, &[("other", 200)]);
        c4_processed_rejection(&mut ledger, &key);
        c4_retarget(&mut ledger, &mut rows, &key, &keys[0], &id);
        assert_eq!(c4_rollback_counted(&ledger, &rows, &key), (true, 5));
    }
}
