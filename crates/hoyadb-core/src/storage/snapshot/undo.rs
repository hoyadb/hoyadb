//! SPEC-0037 R7: in-place memory batches with a touched-entry undo log, so a
//! batch copies no untouched row. Errors and unwinding panics restore every
//! touched row, index entry and table through `Drop`; no `unsafe` is used.
use super::{SnapshotStorage, TableIndexes, fault_point, insert_entries, remove_entries};
use crate::storage::{Row, StorageError, count_work};
use std::collections::{BTreeMap, BTreeSet};

enum Undo {
    /// `after` is recorded too: a primitive interrupted after its map write may
    /// leave index entries of the attempted image.
    Row {
        table: String,
        id: String,
        before: Option<Row>,
        after: Option<Row>,
        /// None means the table index was absent; otherwise these touched
        /// field maps did not exist. Preserve even previously empty maps.
        absent_index_fields: Option<Vec<String>>,
    },
    /// A whole-table replacement keeps the moved-out rows and index, uncopied.
    Table {
        name: String,
        previous: Option<(BTreeMap<String, Row>, Option<TableIndexes>)>,
    },
}

pub(super) struct UndoGuard<'a> {
    store: &'a mut SnapshotStorage,
    log: Vec<Undo>,
}

fn table_not_found(table: &str) -> StorageError {
    StorageError::TableNotFound {
        table: table.into(),
    }
}

/// Capture only the shape a touched row can change, never any value/id set or
/// unrelated field. All allocation happens before the corresponding map write.
fn absent_index_fields(
    indexes: &BTreeMap<String, TableIndexes>,
    table: &str,
    rows: &[&Row],
) -> Option<Vec<String>> {
    let index = indexes.get(table)?;
    Some(
        std::iter::once("id")
            .chain(
                rows.iter()
                    .flat_map(|row| row.fields.keys().map(String::as_str)),
            )
            .filter(|name| !index.contains_key(*name))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(str::to_owned)
            .collect(),
    )
}

impl<'a> UndoGuard<'a> {
    pub(super) fn new(store: &'a mut SnapshotStorage) -> Self {
        Self {
            store,
            log: Vec::new(),
        }
    }

    pub(super) fn store(&self) -> &SnapshotStorage {
        self.store
    }

    /// Keeps every change; dropping a guard without commit undoes them.
    pub(super) fn commit(mut self) {
        fault_point("before-commit");
        self.log.clear();
    }

    pub(super) fn insert(&mut self, table: &str, row: Row) -> Result<(), StorageError> {
        let rows = self
            .store
            .tables
            .get_mut(table)
            .ok_or_else(|| table_not_found(table))?;
        if rows.contains_key(&row.id) {
            return Err(StorageError::RowAlreadyExists {
                table: table.into(),
                id: row.id,
            });
        }
        let absent_index_fields = absent_index_fields(&self.store.indexes, table, &[&row]);
        count_work!(rows_cloned, 1);
        self.log.push(Undo::Row {
            table: table.into(),
            id: row.id.clone(),
            before: None,
            after: Some(row.clone()),
            absent_index_fields,
        });
        rows.insert(row.id.clone(), row);
        fault_point("map-written");
        if let Some(Undo::Row {
            after: Some(after),
            ..
        }) = self.log.last()
        {
            insert_entries(&mut self.store.indexes, table, after);
        }
        Ok(())
    }

    pub(super) fn replace(&mut self, table: &str, row: Row) -> Result<(), StorageError> {
        let rows = self
            .store
            .tables
            .get_mut(table)
            .ok_or_else(|| table_not_found(table))?;
        let stored = rows
            .get_mut(&row.id)
            .ok_or_else(|| StorageError::RowNotFound {
                table: table.into(),
                id: row.id.clone(),
            })?;
        let absent_index_fields = absent_index_fields(&self.store.indexes, table, &[stored, &row]);
        count_work!(rows_cloned, 1);
        let (name, id, after) = (table.to_owned(), row.id.clone(), row.clone());
        // Nothing between the map write and the push can panic.
        self.log.reserve(1);
        let before = std::mem::replace(stored, row);
        self.log.push(Undo::Row {
            table: name,
            id,
            before: Some(before),
            after: Some(after),
            absent_index_fields,
        });
        fault_point("map-written");
        if let Some(Undo::Row {
            before: Some(before),
            after: Some(after),
            ..
        }) = self.log.last()
        {
            remove_entries(&mut self.store.indexes, table, before);
            fault_point("index-removed");
            insert_entries(&mut self.store.indexes, table, after);
        }
        Ok(())
    }

    pub(super) fn remove(&mut self, table: &str, id: &str) -> Result<bool, StorageError> {
        let rows = self
            .store
            .tables
            .get_mut(table)
            .ok_or_else(|| table_not_found(table))?;
        let (name, key) = (table.to_owned(), id.to_owned());
        let Some(before) = rows.get(id) else {
            return Ok(false);
        };
        let absent_index_fields = absent_index_fields(&self.store.indexes, table, &[before]);
        self.log.reserve(1);
        let Some(before) = rows.remove(id) else {
            return Ok(false);
        };
        self.log.push(Undo::Row {
            table: name,
            id: key,
            before: Some(before),
            after: None,
            absent_index_fields,
        });
        fault_point("map-written");
        if let Some(Undo::Row {
            before: Some(before),
            ..
        }) = self.log.last()
        {
            remove_entries(&mut self.store.indexes, table, before);
        }
        Ok(true)
    }

    /// Replaces a table with an empty one; the old rows and index are moved.
    pub(super) fn clear_table(&mut self, name: &str) {
        let key = name.to_owned();
        self.log.reserve(1);
        let rows = self.store.tables.insert(key.clone(), BTreeMap::new());
        let index = self.store.indexes.remove(name);
        self.log.push(Undo::Table {
            name: key,
            previous: rows.map(|rows| (rows, index)),
        });
        fault_point("table-created");
    }
}

impl Drop for UndoGuard<'_> {
    fn drop(&mut self) {
        let store = &mut *self.store;
        let (tables, indexes) = (&mut store.tables, &mut store.indexes);
        while let Some(entry) = self.log.pop() {
            match entry {
                Undo::Row {
                    table,
                    id,
                    before,
                    after,
                    absent_index_fields,
                } => {
                    let Some(rows) = tables.get_mut(&table) else {
                        continue;
                    };
                    // Idempotent: removes whatever a partial primitive indexed.
                    if let Some(current) = rows.remove(&id) {
                        remove_entries(indexes, &table, &current);
                    }
                    if let Some(after) = &after {
                        remove_entries(indexes, &table, after);
                    }
                    if let Some(before) = before {
                        insert_entries(indexes, &table, &before);
                        rows.insert(id, before);
                    }
                    match absent_index_fields {
                        None => {
                            indexes.remove(&table);
                        }
                        Some(fields) => {
                            if let Some(index) = indexes.get_mut(&table) {
                                for field in fields {
                                    index.remove(&field);
                                }
                            }
                        }
                    }
                }
                Undo::Table {
                    name,
                    previous,
                } => match previous {
                    Some((rows, index)) => {
                        tables.insert(name.clone(), rows);
                        match index {
                            Some(index) => {
                                indexes.insert(name, index);
                            }
                            None => {
                                indexes.remove(&name);
                            }
                        }
                    }
                    None => {
                        tables.remove(&name);
                        indexes.remove(&name);
                    }
                },
            }
        }
    }
}
