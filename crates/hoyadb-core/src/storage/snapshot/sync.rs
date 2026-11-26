//! API-0007: memory recovery commits publish all of a plan or none of it.
use super::SnapshotStorage;
use crate::storage::*;

impl SnapshotStorage {
    pub(super) fn sync_rows(&self) -> Result<Option<Vec<Row>>, StorageError> {
        let Some(rows) = self.tables.get(SYNC_TABLE) else {
            return Ok(None);
        };
        let model = self
            .tables
            .get(MODEL_TABLE)
            .ok_or_else(|| schema_conflict("orphan command recovery"))?;
        let state = model_state_from_rows(&model.values().cloned().collect::<Vec<_>>())?;
        let rows = rows.values().cloned().collect::<Vec<_>>();
        validate_sync_inventory(&rows, &state)?;
        Ok(Some(rows))
    }
}

impl SyncStorage for SnapshotStorage {
    fn load_sync(&self) -> Result<Option<Vec<Row>>, StorageError> {
        self.model_catalog()?;
        self.sync_rows()
    }

    /// An owned value: no other owner can reach these rows (SPEC-0037 R3.0).
    fn ownership(&self) -> StoreOwnership {
        StoreOwnership::Exclusive
    }

    /// Every read is an in-memory lookup that completes on its first poll.
    fn reads_ready(&self) -> bool {
        true
    }

    fn apply_sync_delta(&mut self, delta: SyncDelta) -> Result<Vec<CommittedChange>, StorageError> {
        let schema = validate_sync_delta(&delta)?;
        let state = self
            .tables
            .get(MODEL_TABLE)
            .and_then(|rows| rows.get("state"))
            .ok_or_else(|| schema_conflict("command model changed before commit"))?;
        if model_state_from_rows(std::slice::from_ref(state))? != *delta.expected_model() {
            return Err(schema_conflict("command model changed before commit"));
        }
        for (id, expected) in delta.expected_packets() {
            let actual = self.tables.get(SYNC_TABLE).and_then(|rows| rows.get(id));
            if actual != expected.as_ref() {
                return Err(schema_conflict("command recovery changed before commit"));
            }
        }
        for expected in delta.guards() {
            let rows =
                self.tables
                    .get(&expected.table)
                    .ok_or_else(|| StorageError::TableNotFound {
                        table: expected.table.clone(),
                    })?;
            let actual = rows.get(&expected.id);
            if let Some(row) = actual {
                scoped_bookkeeping(row, delta.expected_model().version)?;
            }
            if actual != expected.expected.as_ref() {
                return Err(schema_conflict(GUARD_CONFLICT));
            }
        }
        let parts = delta.into_parts();
        // Strict open validated the base; the proof and exact touched-image
        // checks preserve it without reading unrelated rows or packets.
        let mut guard = super::UndoGuard::new(self);
        let changes = super::typed::typed_batch(&mut guard, &schema, parts.mutations)?;
        guard.replace(MODEL_TABLE, model_state_row(&parts.next_model)?)?;
        if !guard.store().tables.contains_key(SYNC_TABLE) {
            guard.clear_table(SYNC_TABLE);
        }
        for id in parts.deletes {
            guard.remove(SYNC_TABLE, &id)?;
        }
        for row in parts.upserts {
            if guard.store().tables[SYNC_TABLE].contains_key(&row.id) {
                guard.replace(SYNC_TABLE, row)?;
            } else {
                guard.insert(SYNC_TABLE, row)?;
            }
        }
        guard.commit();
        Ok(changes)
    }

    fn apply_sync_plan(&mut self, plan: SyncPlan) -> Result<Vec<CommittedChange>, StorageError> {
        let schema = validate_sync_plan(&plan)?;
        if self.model_catalog()?.state.as_ref() != Some(&plan.expected_model) {
            return Err(schema_conflict("command model changed before commit"));
        }
        let actual = self.sync_rows()?;
        check_sync_rows(actual.as_deref(), plan.expected_sync.as_deref())?;
        validate_reset_inventory(
            &plan,
            schema.tables.keys().flat_map(|table| {
                self.tables[table]
                    .keys()
                    .map(move |id| (table.clone(), id.clone()))
            }),
        )?;
        for guard in &plan.guards {
            let actual = self
                .tables
                .get(&guard.table)
                .and_then(|rows| rows.get(&guard.id));
            if actual != guard.expected.as_ref() {
                return Err(schema_conflict(GUARD_CONFLICT));
            }
        }
        // SPEC-0037 R7: in place under one guard; any error or unwind restores
        // every touched row, the model row and the moved-out sync packets.
        let mut guard = super::UndoGuard::new(self);
        let changes = super::typed::typed_batch(&mut guard, &schema, plan.mutations)?;
        guard.replace(MODEL_TABLE, model_state_row(&plan.next_model)?)?;
        guard.clear_table(SYNC_TABLE);
        for row in plan.next_sync {
            guard.insert(SYNC_TABLE, row)?;
        }
        guard.store().model_catalog()?;
        guard.commit();
        Ok(changes)
    }
}

fn scoped_bookkeeping(row: &Row, version: u32) -> Result<(), StorageError> {
    use crate::model::{RESERVED_PREFIX, ROW_VERSION};
    let backend = |message: &str| StorageError::BackendFailure {
        message: message.into(),
    };
    if row
        .fields
        .keys()
        .any(|name| name.starts_with(RESERVED_PREFIX) && name != ROW_VERSION)
    {
        return Err(backend("unknown reserved record bookkeeping"));
    }
    if row.fields.get(ROW_VERSION) != Some(&Value::Integer(i64::from(version))) {
        return Err(backend("managed row version differs from schema"));
    }
    Ok(())
}
