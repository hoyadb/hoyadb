//! SPEC-0038 C4: the proven entity fallback at a closure-proof mismatch (O2 slice).
//!
//! Pure planning over the validated ledger and snapshot, entered only after every ordinary check
//! and only at the closure-proof mismatch. Any failed condition returns `None`, so the caller
//! keeps the original `RollbackConflict` with nothing changed.
use super::super::ledger_txn::LedgerTxn;
use super::super::{
    EntityOperation,
    entity::{EntityChange, FieldStamp, RowMetadata},
    records::{SyncRecord, SyncState},
};
use super::{Hooks, Snapshot};
use crate::{model::SchemaDefinition, storage::Row};
use std::collections::BTreeSet;

/// The C4 path: a typed Update correction of an Applied or Corrected A, or the rollback of a
/// processed rejection P (never cancel).
pub(super) enum Path<'a> {
    Correction(&'a [EntityChange]),
    Rollback,
}

/// A whole image: the row and its metadata.
type Image = (Row, RowMetadata);

fn clone_image(image: &Image) -> Image {
    (super::clone_row(&image.0), image.1.clone())
}

/// Builds eff0 and recomputes L (rule G2), or `None` when a condition E1-E6 or G2 fails.
/// Returns the ledger with L's recomputed images and W's metadata, and the rows with W's row;
/// K's own transition is the caller's.
pub(super) fn rebase_in(
    ledger: &mut LedgerTxn<'_>,
    rows: &Snapshot,
    schema: &SchemaDefinition,
    key: &str,
    path: Path<'_>,
    hooks: &Hooks,
) -> Option<Snapshot> {
    // Shape: an existing-row single entity Update record, alone in its closure.
    let k = ledger.records.get(key)?;
    let descriptor = k.entity.as_ref()?;
    if descriptor.op != EntityOperation::Update
        || !k.created.is_empty()
        || k.undo.len() != 1
        || k.metadata_undo.len() != 1
        || super::closure(ledger, key).ok()? != BTreeSet::from([key.to_owned()])
    {
        return None;
    }
    let (table, id) = (&k.undo[0].table, &k.undo[0].id);
    if table != &descriptor.table || hooks.contains_key(table) {
        return None;
    }
    let (k_before, k_after) = images(k, table, id)?;
    let mut owned = declared(k);
    match path {
        Path::Correction(changes) => {
            for change in changes {
                if change.op != EntityOperation::Update
                    || &change.table != table
                    || &change.record_id != id
                {
                    return None;
                }
                owned.extend(change.changes.keys().cloned());
                owned.extend(change.removed_fields.iter().cloned());
            }
        }
        Path::Rollback => {
            if k.state != SyncState::Rejected || !k.reconciled || k.rolled_back {
                return None;
            }
        }
    }
    // L: the live records after K that touch W, in order (E1-E4).
    let live = ledger.live_key_refs();
    let mut later = vec![];
    for record in ledger.records.values() {
        if record.key == key || !live.contains(record.key.as_str()) || !touches(record, table, id) {
            continue;
        }
        let entity = record.entity.as_ref()?; // E1: a live named command refuses.
        if record.order < k.order // E2
            || !matches!(
                record.state,
                SyncState::Queued | SyncState::Sent | SyncState::Acknowledged
            )
            || record.outcome.is_some() // E3
            || entity.op != EntityOperation::Update // E4
            || !record.created.is_empty()
        {
            return None;
        }
        later.push(record);
    }
    later.sort_by_key(|record| record.order);
    let chain = later
        .iter()
        .map(|record| images(record, table, id))
        .collect::<Option<Vec<_>>>()?;
    // E4: W exists now with metadata and no tombstone.
    crate::storage::count_work!(metadata_entries_visited, 1);
    let current = (
        super::clone_row(rows.get(table)?.get(id)?),
        ledger
            .entity
            .as_ref()?
            .metadata
            .get(&(table.clone(), id.clone()))?
            .clone(),
    );
    if current.1.tombstone.is_some() {
        return None;
    }
    // E5: whole images chain exactly, flags included, and current W equals L_last.after.
    if chain.windows(2).any(|pair| pair[0].1 != pair[1].0)
        || chain.last().is_some_and(|(_, after)| *after != current)
    {
        return None;
    }
    // E6: on every owned field the baseline B equals K.after in value and whole FieldStamp.
    let base = chain.first().map_or(&current, |(before, _)| before);
    if owned.iter().any(|name| {
        base.0.fields.get(name) != k_after.0.fields.get(name)
            || base.1.fields.get(name) != k_after.1.fields.get(name)
    }) {
        return None;
    }
    // eff0: B with only the owned fields changed.
    let mut eff = clone_image(base);
    match path {
        Path::Correction(changes) => {
            let mut corrected = BTreeSet::new();
            for change in changes {
                let resolved =
                    super::corrected_image(schema, Some(&eff.0), eff.1.clone(), change).ok()?;
                eff = (resolved.row?, resolved.metadata);
                corrected.extend(change.changes.keys().cloned());
                corrected.extend(change.removed_fields.iter().cloned());
            }
            // C1a value-aware promotion of A's uncorrected declared fields, into eff0 only.
            let (row, metadata) = &mut eff;
            for name in declared(k).difference(&corrected) {
                if let (Some(stamp), Some(cell)) =
                    (k_after.1.fields.get(name), metadata.fields.get_mut(name))
                    && row.fields.get(name) == k_after.0.fields.get(name)
                    && cell.timestamp_ms == stamp.timestamp_ms
                {
                    cell.authoritative = true;
                }
            }
        }
        Path::Rollback => {
            for name in &owned {
                copy(&mut eff, &k_before, name);
            }
        }
    }
    // Recompute L last (rule G2): each declared field must carry a strictly newer client stamp
    // than eff(i-1); a field without a stamp counts as time 0 for this comparison only.
    let mut rebased = Vec::with_capacity(chain.len());
    for (record, (_, after)) in later.iter().zip(&chain) {
        let before = clone_image(&eff);
        for name in declared(record) {
            let stamp = after.1.fields.get(&name)?;
            let time = eff
                .1
                .fields
                .get(&name)
                .map_or(0, |stamp| stamp.timestamp_ms);
            if stamp.timestamp_ms <= time {
                return None;
            }
            copy(&mut eff, after, &name);
            eff.1.fields.insert(
                name,
                FieldStamp {
                    timestamp_ms: stamp.timestamp_ms,
                    authoritative: false,
                },
            );
        }
        rebased.push((record.key.clone(), before, clone_image(&eff)));
    }
    // All E/G refusals precede the first ledger mutation.
    let identity = (table.clone(), id.clone());
    let (row, metadata) = eff;
    let mut result = super::clone_snapshot(rows);
    result.get_mut(&identity.0)?.insert(identity.1.clone(), row);
    for (key, before, after) in rebased {
        let record = ledger.record_mut(&key).expect("validated C4 member");
        record.undo[0].before = Some(before.0);
        record.undo[0].after = Some(after.0);
        record.metadata_undo[0].before = Some(before.1);
        record.metadata_undo[0].after = Some(after.1);
    }
    ledger
        .put_metadata(identity, metadata)
        .expect("validated C4 entity ledger");
    Some(result)
}

/// A record's single row and metadata change on W, both images present and without a tombstone
/// (E4), as (before, after).
fn images(record: &SyncRecord, table: &str, id: &str) -> Option<(Image, Image)> {
    let ([undo], [meta]) = (record.undo.as_slice(), record.metadata_undo.as_slice()) else {
        return None;
    };
    crate::storage::count_work!(metadata_entries_visited, 2);
    if undo.table != table || undo.id != id || meta.table != table || meta.record_id != id {
        return None;
    }
    let image = |row: &Option<Row>, metadata: &Option<RowMetadata>| match (row, metadata) {
        (Some(row), Some(metadata)) if metadata.tombstone.is_none() => {
            Some((super::clone_row(row), metadata.clone()))
        }
        _ => None,
    };
    Some((
        image(&undo.before, &meta.before)?,
        image(&undo.after, &meta.after)?,
    ))
}

/// Whether a record's recovery images touch W. Counts each metadata-undo entry actually
/// examined: none after a row-undo match, and none after the first metadata match.
fn touches(record: &SyncRecord, table: &str, id: &str) -> bool {
    record
        .undo
        .iter()
        .any(|change| change.table == table && change.id == id)
        || record.metadata_undo.iter().any(|change| {
            crate::storage::count_work!(metadata_entries_visited, 1);
            change.table == table && change.record_id == id
        })
}

/// The fields an entity record declared: its changes and its removals.
fn declared(record: &SyncRecord) -> BTreeSet<String> {
    let mut fields = record
        .entity
        .as_ref()
        .map(|entity| entity.removed_fields.clone())
        .unwrap_or_default();
    if let super::super::Json::Object(args) = &record.args
        && let Some(super::super::Json::Object(changes)) = args.get("changes")
    {
        fields.extend(changes.keys().cloned());
    }
    fields
}

/// Copies one field from `source` into `target` exactly: value or absence, stamp or none.
fn copy(target: &mut Image, source: &Image, name: &str) {
    match source.0.fields.get(name) {
        Some(value) => target.0.fields.insert(name.to_owned(), value.clone()),
        None => target.0.fields.remove(name),
    };
    match source.1.fields.get(name) {
        Some(stamp) => target.1.fields.insert(name.to_owned(), stamp.clone()),
        None => target.1.fields.remove(name),
    };
}

#[cfg(test)]
mod tests {
    use super::super::super::{Json, entity::MetadataChange};
    use super::*;
    use crate::storage::{CommittedChange, counters};

    /// A bare live record with these row-undo and metadata-undo identities on `parents`.
    fn record(undo: &[&str], metadata: &[&str]) -> SyncRecord {
        SyncRecord {
            entity: None,
            metadata_undo: metadata
                .iter()
                .map(|id| MetadataChange {
                    table: "parents".into(),
                    record_id: (*id).into(),
                    before: None,
                    after: None,
                })
                .collect(),
            key: "k".into(),
            command: "write-v1".into(),
            args: Json::Null,
            context: Json::Null,
            schema_version: 1,
            order: 1,
            reference_paths: vec![],
            dependencies: BTreeSet::new(),
            created: Default::default(),
            mappings: Default::default(),
            undo: undo
                .iter()
                .map(|id| CommittedChange {
                    table: "parents".into(),
                    id: (*id).into(),
                    before: None,
                    after: None,
                })
                .collect(),
            state: SyncState::Sent,
            frozen: None,
            attempts: 1,
            due_ms: 0,
            outcome: None,
            rolled_back: false,
            reconciled: false,
        }
    }

    #[test]
    fn si_w03_c4_image_pair_counts_both_rows() {
        let mut record = record(&["w"], &["w"]);
        let row = Row {
            id: "w".into(),
            fields: Default::default(),
        };
        record.undo[0].before = Some(row.clone());
        record.undo[0].after = Some(row);
        record.metadata_undo[0].before = Some(RowMetadata::default());
        record.metadata_undo[0].after = Some(RowMetadata::default());
        counters::reset();
        assert!(images(&record, "parents", "w").is_some());
        assert_eq!(counters::snapshot().rows_cloned, 2);
    }

    /// `touches` and the metadata entries it actually examined (SPEC-0037 R4).
    fn visited(record: &SyncRecord) -> (bool, u64) {
        counters::reset();
        let touched = touches(record, "parents", "w");
        (touched, counters::snapshot().metadata_entries_visited)
    }

    // A row-undo match short-circuits: no metadata entry is examined.
    #[test]
    fn touches_row_undo_match_examines_no_metadata() {
        assert_eq!(visited(&record(&["w"], &["w", "x"])), (true, 0));
    }

    // A metadata match stops at the first match, before the end of the list.
    #[test]
    fn touches_metadata_match_stops_at_first_match() {
        assert_eq!(visited(&record(&["x"], &["x", "w", "y"])), (true, 2));
    }

    // No match scans every metadata entry (characterization: unchanged by the fix).
    #[test]
    fn touches_no_match_examines_every_metadata_entry() {
        assert_eq!(visited(&record(&["x"], &["x", "y"])), (false, 2));
    }
}
