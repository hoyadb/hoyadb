//! SPEC-0021: experimental whole-store SQLite commits, never a production adapter.
use hoyadb_cross_platform_spike::{Order, Store};
use rusqlite::{Connection, OpenFlags, params};
use std::collections::HashSet;
use std::ops::{Deref, DerefMut};
use std::path::Path;

#[derive(Default)]
pub(crate) struct NativeStore {
    core: Store,
    connection: Option<Connection>,
}

impl Deref for NativeStore {
    type Target = Store;

    fn deref(&self) -> &Store {
        &self.core
    }
}

impl DerefMut for NativeStore {
    fn deref_mut(&mut self) -> &mut Store {
        &mut self.core
    }
}

impl NativeStore {
    pub(crate) fn open(path: &str) -> Result<Self, ()> {
        if path.is_empty() || path == ":memory:" || path.starts_with("file:") {
            return Err(());
        }
        let exists = Path::new(path).try_exists().map_err(|_| ())?;
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .map_err(|_| ())?;
        if !exists {
            connection
                .execute_batch(
                    "CREATE TABLE hoya_spike_rows (
                id TEXT PRIMARY KEY NOT NULL, status TEXT NOT NULL,
                total_cents INTEGER NOT NULL, paid INTEGER NOT NULL CHECK(paid IN (0,1)))",
                )
                .map_err(|_| ())?;
        }
        let mut core = Store::default();
        {
            let mut statement = connection
                .prepare("SELECT id,status,total_cents,paid FROM hoya_spike_rows")
                .map_err(|_| ())?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })
                .map_err(|_| ())?;
            let mut ids = HashSet::new();
            for row in rows {
                let (id, status, total_cents, paid) = row.map_err(|_| ())?;
                if !matches!(paid, 0 | 1) || !ids.insert(id.clone()) {
                    return Err(());
                }
                core.insert(Order {
                    id,
                    status,
                    total_cents,
                    paid: paid == 1,
                });
            }
        }
        Ok(Self {
            core,
            connection: Some(connection),
        })
    }

    fn update(&mut self, operation: impl FnOnce(&mut Store) -> Result<(), i32>) -> Result<(), i32> {
        if let Some(connection) = self.connection.as_mut() {
            let mut candidate = self.core.clone();
            operation(&mut candidate)?;
            let transaction = connection.transaction().map_err(|_| 4)?;
            transaction
                .execute("DELETE FROM hoya_spike_rows", [])
                .map_err(|_| 4)?;
            {
                let mut statement = transaction.prepare("INSERT INTO hoya_spike_rows (id,status,total_cents,paid) VALUES (?1,?2,?3,?4)").map_err(|_|4)?;
                for row in candidate.committed_rows() {
                    statement
                        .execute(params![
                            row.id,
                            row.status,
                            row.total_cents,
                            i64::from(row.paid)
                        ])
                        .map_err(|_| 4)?;
                }
            }
            transaction.commit().map_err(|_| 4)?;
            self.core = candidate;
            Ok(())
        } else {
            operation(&mut self.core)
        }
    }

    pub(crate) fn write_insert(&mut self, row: Order) -> Result<(), i32> {
        self.update(|core| {
            core.insert(row);
            Ok(())
        })
    }

    pub(crate) fn finish(&mut self, token: u64, commit: bool) -> Result<(), i32> {
        if !commit {
            return self.core.finish_scope(token, false).then_some(()).ok_or(1);
        }
        self.update(|core| core.finish_scope(token, true).then_some(()).ok_or(1))
    }
}
