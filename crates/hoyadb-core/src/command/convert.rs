//! SPEC-0037 R11: pure, explicit offline ledger conversion for native tooling.
//!
//! Nothing here opens a store. The facade, SDKs and `CommandDatabase` never call
//! this module; only the feature-gated SQLite tooling entry does.
use super::records::{SyncLedger, entity_header_row, identity_metadata_row, metadata_row};
use crate::storage::{ModelState, Row, StorageError, schema_conflict, validate_sync_model};

/// Counts logical replacement packets, not SQL deletes plus inserts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConversionReport {
    pub from: u8,
    pub to: u8,
    pub metadata_entries: usize,
    /// Every metadata packet plus the entity header; excludes all other packets.
    pub packets_rewritten: usize,
}

/// Fully validated source and replacement packets for explicit offline tooling.
///
/// Fields are private so storage tooling cannot forge a conversion. This value
/// authorizes no write by itself: the caller must retain its storage lease and
/// compare the complete source rows and model again inside its transaction.
pub struct LedgerConversion {
    source_rows: Vec<Row>,
    rows: Vec<Row>,
    model: ModelState,
    report: ConversionReport,
}

impl LedgerConversion {
    /// The exact source packets, in the order they were supplied.
    pub fn source_rows(&self) -> &[Row] {
        &self.source_rows
    }

    /// The replacement packets, ordered by id.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn model(&self) -> &ModelState {
        &self.model
    }

    pub fn report(&self) -> &ConversionReport {
        &self.report
    }
}

/// Re-encodes an existing entity ledger to exactly format 2 or 3.
///
/// This pure helper does not open stores or inspect application rows. It fully
/// decodes and validates the recovery packets against their recorded model
/// before looking at the target. Every packet other than `entity:state` and the
/// `entity:metadata:*` packets is carried over byte for byte; the header gains or
/// loses only its format marker and the metadata packets change only their ids.
/// Output packets are ordered by id.
pub fn reencode(
    rows: &[Row],
    model: &ModelState,
    to: u8,
) -> Result<LedgerConversion, StorageError> {
    let ledger = SyncLedger::from_rows(rows)?;
    validate_sync_model(&ledger, model)?;
    let entity = ledger
        .entity
        .as_ref()
        .ok_or_else(|| schema_conflict("conversion requires entity comparison state"))?;
    if !matches!(to, 2 | 3) {
        return Err(schema_conflict("unsupported target ledger format"));
    }
    if entity.ledger_format == to {
        return Err(schema_conflict("ledger already uses the requested format"));
    }
    let mut target = entity.clone();
    target.ledger_format = to;
    let mut converted: Vec<Row> = rows
        .iter()
        .filter(|row| row.id != "entity:state" && !row.id.starts_with("entity:metadata:"))
        .cloned()
        .collect();
    converted.push(entity_header_row(&target)?);
    for (ordinal, ((table, id), metadata)) in target.metadata.iter().enumerate() {
        converted.push(if to == 3 {
            identity_metadata_row(table, id, metadata)?
        } else {
            metadata_row(ordinal as u64, table, id, metadata)?
        });
    }
    converted.sort_by(|left, right| left.id.cmp(&right.id));
    // The replacement set must itself decode to the same ledger in the new format.
    let decoded = SyncLedger::from_rows(&converted)?;
    let mut expected = ledger.clone();
    expected.entity = Some(target);
    if decoded != expected {
        return Err(schema_conflict("converted ledger does not round trip"));
    }
    let metadata_entries = entity.metadata.len();
    Ok(LedgerConversion {
        source_rows: rows.to_vec(),
        rows: converted,
        model: model.clone(),
        report: ConversionReport {
            from: entity.ledger_format,
            to,
            metadata_entries,
            packets_rewritten: metadata_entries + 1,
        },
    })
}

#[cfg(test)]
mod tests;
