//! SPEC-0022 / SPEC-0023 / SPEC-0026: SQLite storage and schema lifecycle.
//! Operations execute synchronously and return ready Send futures. No async runtime,
//! shared connection access or async runtime is introduced. Encryption uses supplied raw keys.
mod encryption;
pub use encryption::{EncryptionKey, SqliteOpenOptions, StorageProtection};
#[cfg(all(feature = "ledger-convert-tool", not(target_arch = "wasm32")))]
#[doc(hidden)]
pub mod convert;
mod lifecycle;
mod projection;
mod query;
mod sync;
#[cfg(all(feature = "test-hooks", not(target_arch = "wasm32")))]
#[doc(hidden)]
pub mod test_hooks;
use hoyadb_core::storage::{
    ColumnType, CommittedChange, Direction, FiniteFloat, Mutation, Query, Row, Schema, Storage,
    StorageError, TransactionalStorage, TypedStorage, TypedTransactionalStorage, ValidatedQuery,
    Value, resolve_field, validate_row, validate_schema,
};
use rusqlite::{
    Connection, OpenFlags, params, params_from_iter,
    types::{Value as SqlValue, ValueRef},
};
use std::collections::BTreeMap;
use std::future::{Future, ready};
use std::path::Path;
use std::time::Duration;

// An exact, private version-1 format. No consumer-controlled SQL identifiers.
const SCHEMA: [(&str, &str); 4] = [
    (
        "__hoya_meta",
        "CREATE TABLE __hoya_meta(version INTEGER NOT NULL)",
    ),
    (
        "__hoya_tables",
        "CREATE TABLE __hoya_tables(name TEXT PRIMARY KEY NOT NULL)",
    ),
    (
        "__hoya_rows",
        "CREATE TABLE __hoya_rows(table_name TEXT NOT NULL,id TEXT NOT NULL,PRIMARY KEY(table_name,id))",
    ),
    (
        "__hoya_fields",
        "CREATE TABLE __hoya_fields(table_name TEXT NOT NULL,row_id TEXT NOT NULL,name TEXT NOT NULL,kind INTEGER NOT NULL,text_value TEXT,int_value INTEGER,PRIMARY KEY(table_name,row_id,name))",
    ),
];

pub struct SqliteStorage {
    connection: Connection,
    protection: StorageProtection,
}

fn backend(error: impl std::fmt::Display) -> StorageError {
    StorageError::BackendFailure {
        message: error.to_string(),
    }
}

impl SqliteStorage {
    pub fn open_with_options(
        path: impl AsRef<Path>,
        options: SqliteOpenOptions,
    ) -> Result<Self, StorageError> {
        let key = options
            .encryption_key
            .ok_or_else(|| backend("an application encryption key is required"))?;
        Self::open_inner(path.as_ref(), Some(key)).map_err(|error| match error {
            StorageError::StoreInUse => StorageError::StoreInUse,
            _ => backend("protected storage open failed"),
        })
    }

    pub fn storage_protection(&self) -> StorageProtection {
        self.protection
    }

    /// SPEC-0037 R2.3: lets a fixture holding this store corrupt, read and restore
    /// on its one live connection. Runs one statement and returns its changed
    /// row count. Absent from normal builds.
    #[cfg(all(feature = "test-hooks", not(target_arch = "wasm32")))]
    #[doc(hidden)]
    pub fn execute_test_sql(&self, sql: &str) -> Result<usize, StorageError> {
        self.connection.execute(sql, []).map_err(backend)
    }

    /// Protected default: this convenience signature has no application key.
    /// Rejects before inspecting a path or touching the filesystem (API-0010).
    pub fn open(_path: impl AsRef<Path>) -> Result<Self, StorageError> {
        Err(backend("an application encryption key is required"))
    }

    /// Explicit plaintext control for historical synthetic fixtures only.
    /// This entry does not meet the protected storage security contract.
    /// Opens a literal local path, or the installed `opfs-sahpool` VFS on WASM.
    /// Initializes only an empty store; rejects unknown formats without reset.
    pub fn open_unprotected(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        Self::open_inner(path.as_ref(), None)
    }

    fn open_inner(path: &Path, key: Option<EncryptionKey>) -> Result<Self, StorageError> {
        if path.as_os_str().is_empty()
            || path == Path::new(":memory:")
            || path.as_os_str().as_encoded_bytes().starts_with(b"file:")
            || path.as_os_str().as_encoded_bytes().contains(&0)
        {
            return Err(backend("a local database file path is required"));
        }
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE;
        #[cfg(not(target_arch = "wasm32"))]
        let mut connection = Connection::open_with_flags(path, flags).map_err(backend)?;
        // The backend registers a memory VFS by default. Naming OPFS explicitly
        // prevents an uninitialized browser caller from silently using memory.
        #[cfg(target_arch = "wasm32")]
        let mut connection =
            Connection::open_with_flags_and_vfs(path, flags, "opfs-sahpool").map_err(backend)?;
        // Keying is the first SQL on this connection. Upstream pragma helpers
        // query pragma_pragma_list, so direct fixed SQL is required here.
        let protection = match key {
            Some(key) => encryption::configure(&connection, key)?,
            None => StorageProtection {
                encrypted: false,
                cipher: "SQLite",
                version: "unprotected",
            },
        };
        connection
            .execute_batch("PRAGMA temp_store=MEMORY")
            .map_err(backend)?;
        connection.busy_timeout(Duration::ZERO).map_err(backend)?;
        // SPEC-0037 R1: take the exclusive lease before any page is read, so an
        // owned store is reported in use even when the supplied key is wrong.
        #[cfg(not(target_arch = "wasm32"))]
        connection
            .execute_batch("PRAGMA locking_mode=EXCLUSIVE; BEGIN EXCLUSIVE; COMMIT;")
            .map_err(|error| match error.sqlite_error_code() {
                Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked) => {
                    StorageError::StoreInUse
                }
                _ => backend(error),
            })?;
        #[cfg(all(feature = "test-hooks", not(target_arch = "wasm32")))]
        test_hooks::register(&connection, path).map_err(backend)?;
        let empty = connection
            .query_row(
                "SELECT NOT EXISTS(SELECT 1 FROM sqlite_schema WHERE name NOT GLOB 'sqlite_*')",
                [],
                |row| row.get::<_, bool>(0),
            )
            .map_err(backend)?;
        if empty {
            let transaction = connection.transaction().map_err(backend)?;
            for (_, sql) in SCHEMA {
                transaction.execute_batch(sql).map_err(backend)?;
            }
            transaction
                .execute("INSERT INTO __hoya_meta(version) VALUES (?1)", [1_i64])
                .map_err(backend)?;
            transaction.commit().map_err(backend)?;
        } else {
            validate_private_format(&connection)?;
        }
        // Private oversized-query operands must never spill a temp file into
        // the browser VFS pool. This does not change main-store durability.
        connection
            .execute_batch("PRAGMA temp_store=MEMORY")
            .map_err(backend)?;
        // Additive engine indexes preserve the validated v1 table DDL and rows.
        let transaction = connection.transaction().map_err(backend)?;
        transaction.execute_batch("CREATE INDEX IF NOT EXISTS __hoya_fields_numeric ON __hoya_fields(table_name,name,kind,int_value,row_id); CREATE INDEX IF NOT EXISTS __hoya_fields_text ON __hoya_fields(table_name,name,kind,text_value,row_id)").map_err(backend)?;
        transaction.commit().map_err(backend)?;
        Ok(Self {
            connection,
            protection,
        })
    }

    fn transaction<T>(
        &mut self,
        operation: impl FnOnce(&Connection) -> Result<T, StorageError>,
    ) -> Result<T, StorageError> {
        let transaction = self.connection.transaction().map_err(backend)?;
        let result = operation(&transaction)?;
        transaction.commit().map_err(backend)?;
        Ok(result)
    }
}

/// SPEC-0037 R4: adds one finished statement's physical work to the thread's
/// counters; nothing without the non-default `work-counters` feature.
fn measure(statement: &rusqlite::Statement<'_>) {
    #[cfg(feature = "work-counters")]
    {
        use hoyadb_core::storage::counters::add;
        use rusqlite::StatementStatus;
        let read = |status| u64::try_from(statement.get_status(status)).unwrap_or(0);
        add(
            |c| &c.sqlite_fullscan_steps,
            read(StatementStatus::FullscanStep),
        );
        add(|c| &c.sqlite_sort, read(StatementStatus::Sort));
        add(|c| &c.sqlite_vm_steps, read(StatementStatus::VmStep));
    }
    #[cfg(not(feature = "work-counters"))]
    let _ = statement;
}

/// Checks names, declared types, keys, collation and constraints through the
/// exact DDL emitted by this unreleased private format, then its version row.
/// Read-only: shared by strict open and the offline converter (SPEC-0037 R11).
fn validate_private_format(connection: &Connection) -> Result<(), StorageError> {
    let tables = connection.prepare("SELECT name,sql FROM sqlite_schema WHERE type='table' AND name NOT GLOB 'sqlite_*' ORDER BY name")
        .map_err(backend)?.query_map([],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?)))
        .map_err(backend)?.collect::<Result<BTreeMap<_,_>,_>>().map_err(backend)?;
    if tables.len() != SCHEMA.len()
        || SCHEMA
            .iter()
            .any(|(name, sql)| tables.get(*name).map(String::as_str) != Some(*sql))
    {
        return Err(backend("unrelated or incompatible SQLite storage format"));
    }
    let versions = connection
        .prepare("SELECT version FROM __hoya_meta")
        .map_err(backend)?
        .query_map([], |row| row.get::<_, i64>(0))
        .map_err(backend)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(backend)?;
    if versions != [1] {
        return Err(backend("unsupported storage format version"));
    }
    Ok(())
}

/// SPEC-0037 R4: counts the rows and fields one write statement changed, as
/// SQLite reports them; nothing without the non-default `work-counters` feature.
pub(crate) fn written(changes: usize) -> usize {
    #[cfg(feature = "work-counters")]
    hoyadb_core::storage::counters::add(
        |c| &c.sqlite_rows_written,
        u64::try_from(changes).unwrap_or(u64::MAX),
    );
    changes
}

/// One-value query whose statement work is measured.
fn query_one<T: rusqlite::types::FromSql>(
    connection: &Connection,
    sql: &str,
    parameters: impl rusqlite::Params,
) -> Result<T, StorageError> {
    let mut statement = connection.prepare(sql).map_err(backend)?;
    let value = statement
        .query_row(parameters, |row| row.get(0))
        .map_err(backend)?;
    measure(&statement);
    Ok(value)
}

fn require_table(connection: &Connection, table: &str) -> Result<(), StorageError> {
    let exists: bool = query_one(
        connection,
        "SELECT EXISTS(SELECT 1 FROM __hoya_tables WHERE name=?1)",
        [table],
    )?;
    if exists {
        Ok(())
    } else {
        Err(StorageError::TableNotFound {
            table: table.into(),
        })
    }
}

fn require_query_tables(
    connection: &Connection,
    schema: &Schema,
    query: &Query,
) -> Result<(), StorageError> {
    require_table(connection, &query.table)?;
    for field in query
        .predicates
        .iter()
        .map(|predicate| &predicate.field)
        .chain(query.order.iter().map(|order| &order.field))
    {
        let resolved = resolve_field(schema, &query.table, field)?;
        require_table(connection, &resolved.table)?;
    }
    Ok(())
}

fn row_exists(connection: &Connection, table: &str, id: &str) -> Result<bool, StorageError> {
    query_one(
        connection,
        "SELECT EXISTS(SELECT 1 FROM __hoya_rows WHERE table_name=?1 AND id=?2)",
        params![table, id],
    )
}

fn write_fields(connection: &Connection, table: &str, row: &Row) -> Result<(), StorageError> {
    let mut statement=connection.prepare("INSERT INTO __hoya_fields(table_name,row_id,name,kind,text_value,int_value) VALUES (?1,?2,?3,?4,?5,?6)").map_err(backend)?;
    for (name, value) in &row.fields {
        let (kind, text, integer) = encode_value(value);
        written(
            statement
                .execute(params![table, row.id, name, kind, text, integer])
                .map_err(backend)?,
        );
    }
    Ok(())
}

fn insert(connection: &Connection, table: &str, row: Row) -> Result<(), StorageError> {
    require_table(connection, table)?;
    if row_exists(connection, table, &row.id)? {
        return Err(StorageError::RowAlreadyExists {
            table: table.into(),
            id: row.id,
        });
    }
    written(
        connection
            .execute(
                "INSERT INTO __hoya_rows(table_name,id) VALUES (?1,?2)",
                params![table, row.id],
            )
            .map_err(backend)?,
    );
    write_fields(connection, table, &row)
}

fn replace(connection: &Connection, table: &str, row: Row) -> Result<(), StorageError> {
    require_table(connection, table)?;
    if !row_exists(connection, table, &row.id)? {
        return Err(StorageError::RowNotFound {
            table: table.into(),
            id: row.id,
        });
    }
    written(
        connection
            .execute(
                "DELETE FROM __hoya_fields WHERE table_name=?1 AND row_id=?2",
                params![table, row.id],
            )
            .map_err(backend)?,
    );
    write_fields(connection, table, &row)
}

fn remove(connection: &Connection, table: &str, id: &str) -> Result<bool, StorageError> {
    require_table(connection, table)?;
    written(
        connection
            .execute(
                "DELETE FROM __hoya_fields WHERE table_name=?1 AND row_id=?2",
                params![table, id],
            )
            .map_err(backend)?,
    );
    Ok(written(
        connection
            .execute(
                "DELETE FROM __hoya_rows WHERE table_name=?1 AND id=?2",
                params![table, id],
            )
            .map_err(backend)?,
    ) != 0)
}

fn get(connection: &Connection, table: &str, id: &str) -> Result<Option<Row>, StorageError> {
    require_table(connection, table)?;
    if !row_exists(connection, table, id)? {
        return Ok(None);
    }
    let mut statement=connection.prepare("SELECT name,kind,text_value,int_value FROM __hoya_fields WHERE table_name=?1 AND row_id=?2").map_err(backend)?;
    let mut cursor = statement.query(params![table, id]).map_err(backend)?;
    let mut fields = BTreeMap::new();
    while let Some(row) = cursor.next().map_err(backend)? {
        let name = match row.get_ref(0).map_err(backend)? {
            ValueRef::Text(value) => std::str::from_utf8(value).map_err(backend)?.to_owned(),
            _ => return Err(backend("invalid stored field name")),
        };
        let value = decode_value(
            row.get_ref(1).map_err(backend)?,
            row.get_ref(2).map_err(backend)?,
            row.get_ref(3).map_err(backend)?,
        )?;
        if fields.insert(name, value).is_some() {
            return Err(backend("duplicate stored field"));
        }
    }
    drop(cursor);
    measure(&statement);
    #[cfg(feature = "work-counters")]
    hoyadb_core::storage::counters::add(|c| &c.sqlite_rows_read, 1);
    Ok(Some(Row {
        id: id.into(),
        fields,
    }))
}

impl Storage for SqliteStorage {
    fn create_table(
        &mut self,
        name: &str,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(
            self.connection
                .execute(
                    "INSERT INTO __hoya_tables(name) VALUES (?1) ON CONFLICT(name) DO NOTHING",
                    [name],
                )
                .map(|_| ())
                .map_err(backend),
        )
    }

    fn insert(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(self.transaction(|connection| insert(connection, table, row)))
    }

    fn get(
        &self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<Option<Row>, StorageError>> + Send {
        ready(get(&self.connection, table, id))
    }

    fn replace(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(self.transaction(|connection| replace(connection, table, row)))
    }

    fn remove(
        &mut self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send {
        ready(self.transaction(|connection| remove(connection, table, id)))
    }
}

impl TransactionalStorage for SqliteStorage {
    fn apply_batch(
        &mut self,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(self.transaction(|connection| {
            for mutation in mutations {
                match mutation {
                    Mutation::Insert {
                        table,
                        row,
                    } => insert(connection, &table, row)?,
                    Mutation::Replace {
                        table,
                        row,
                    } => replace(connection, &table, row)?,
                    Mutation::Remove {
                        table,
                        id,
                    } => {
                        remove(connection, &table, &id)?;
                    }
                }
            }
            Ok(())
        }))
    }
}

impl SqliteStorage {
    /// Actual SQLite planner output for an already validated portable query.
    /// This diagnostic is adapter-specific, not a portable optimizer guarantee.
    pub fn query_plan(&self, schema: &Schema, query: &Query) -> Result<Vec<String>, StorageError> {
        let plan = query::build(schema, query)?;
        require_query_tables(&self.connection, schema, query)?;
        plan.execute(&self.connection, || {
            self.connection
                .prepare(&format!("EXPLAIN QUERY PLAN {}", plan.sql))
                .map_err(backend)?
                .query_map(params_from_iter(plan.parameters.iter()), |row| {
                    row.get::<_, String>(3)
                })
                .map_err(backend)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(backend)
        })
    }

    fn query_rows(&self, schema: &Schema, query: &Query) -> Result<Vec<Row>, StorageError> {
        let plan = query::build(schema, query)?;
        require_query_tables(&self.connection, schema, query)?;
        let ids = plan.execute(&self.connection, || {
            let mut statement = self.connection.prepare(&plan.sql).map_err(backend)?;
            let ids = statement
                .query_map(params_from_iter(plan.parameters.iter()), |row| {
                    row.get::<_, String>(0)
                })
                .map_err(backend)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(backend)?;
            measure(&statement);
            Ok(ids)
        })?;
        ids.into_iter()
            .map(|id| {
                let row = get(&self.connection, &query.table, &id)?
                    .ok_or_else(|| backend("selected row disappeared"))?;
                validate_row(schema, &query.table, &row)?;
                Ok(row)
            })
            .collect()
    }
}

impl TypedStorage for SqliteStorage {
    fn query(
        &self,
        schema: &Schema,
        query: &Query,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        ready(self.query_rows(schema, query))
    }

    fn query_validated(
        &self,
        proof: ValidatedQuery<'_>,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        let (schema, query) = (proof.schema(), proof.query());
        ready(match self.bounded_rows(schema, query) {
            Ok(Some(rows)) => Ok(rows),
            Ok(None) => self.query_rows(schema, query),
            Err(error) => Err(error),
        })
    }
}

// TC-0041 SI-Q01: statements issued by the bounded walk (unit tests only).
#[cfg(test)]
thread_local! {
    static WALK: std::cell::Cell<(u32, u32)> = const { std::cell::Cell::new((0, 0)) };
}

fn walk_statement(_next_value: bool) {
    #[cfg(test)]
    WALK.with(|walk| {
        let (next, equal) = walk.get();
        walk.set(if _next_value {
            (next + 1, equal)
        } else {
            (next, equal + 1)
        });
    });
}

impl SqliteStorage {
    /// SPEC-0037 R8.1: a finite, predicate-free limit ordered by a direct
    /// non-optional column of a validated Exclusive owner reads ids from the
    /// existing field index only. ASC is one range; DESC walks distinct values
    /// with one-row seeks and bounded equality ranges, so no statement sorts
    /// and no tie group is fetched whole. `None` means today's query applies.
    fn bounded_rows(
        &self,
        schema: &Schema,
        query: &Query,
    ) -> Result<Option<Vec<Row>>, StorageError> {
        let (Some(limit), Some(order), true) = (
            query.limit,
            query.order.as_ref(),
            query.predicates.is_empty(),
        ) else {
            return Ok(None);
        };
        validate_schema(schema)?;
        hoyadb_core::storage::validate_query(schema, query)?;
        let field = resolve_field(schema, &query.table, &order.field)?;
        if field.foreign_key.is_some() || field.name == "id" || field.column.optional {
            return Ok(None);
        }
        if limit == 0 {
            return Ok(Some(Vec::new()));
        }
        let (kind, payload) = match field.column.kind {
            ColumnType::String => (1, "text_value"),
            ColumnType::Integer => (2, "int_value"),
            ColumnType::Boolean => (3, "int_value"),
            ColumnType::Float => (4, "int_value"),
            ColumnType::Timestamp => (5, "int_value"),
            ColumnType::Reference => (6, "text_value"),
        };
        let table = query.table.as_str();
        let name = field.name.as_str();
        let bound = i64::try_from(limit).unwrap_or(i64::MAX);
        let fixed = "FROM __hoya_fields WHERE table_name=?1 AND name=?2 AND kind=?3";
        let read_ids = |sql: &str, parameters: &[&dyn rusqlite::ToSql]| {
            let mut statement = self.connection.prepare(sql).map_err(backend)?;
            let ids = statement
                .query_map(parameters, |row| row.get::<_, String>(0))
                .map_err(backend)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(backend)?;
            measure(&statement);
            Ok::<_, StorageError>(ids)
        };
        let ids = match order.direction {
            Direction::Asc => read_ids(
                &format!("SELECT row_id {fixed} ORDER BY {payload} ASC, row_id ASC LIMIT ?4"),
                &[&table, &name, &kind, &bound],
            )?,
            Direction::Desc => {
                let mut ids = Vec::new();
                let mut previous: Option<SqlValue> = None;
                while ids.len() < limit {
                    walk_statement(true);
                    let mut statement = self
                        .connection
                        .prepare(&if previous.is_some() {
                            format!("SELECT {payload} {fixed} AND {payload} < ?4 ORDER BY {payload} DESC LIMIT 1")
                        } else {
                            format!("SELECT {payload} {fixed} ORDER BY {payload} DESC LIMIT 1")
                        })
                        .map_err(backend)?;
                    let next = match &previous {
                        Some(value) => statement
                            .query_row(params![table, name, kind, value], |row| {
                                row.get::<_, SqlValue>(0)
                            }),
                        None => statement
                            .query_row(params![table, name, kind], |row| row.get::<_, SqlValue>(0)),
                    };
                    let next = match next {
                        Ok(value) => Some(value),
                        Err(rusqlite::Error::QueryReturnedNoRows) => None,
                        Err(error) => return Err(backend(error)),
                    };
                    measure(&statement);
                    let Some(value) = next else {
                        break;
                    };
                    walk_statement(false);
                    let remaining = i64::try_from(limit - ids.len()).unwrap_or(i64::MAX);
                    ids.extend(read_ids(
                        &format!(
                            "SELECT row_id {fixed} AND {payload} = ?4 ORDER BY row_id ASC LIMIT ?5"
                        ),
                        &[&table, &name, &kind, &value, &remaining],
                    )?);
                    previous = Some(value);
                }
                ids
            }
        };
        ids.into_iter()
            .map(|id| {
                let row = get(&self.connection, table, &id)?
                    .ok_or_else(|| backend("selected row disappeared"))?;
                validate_row(schema, table, &row)?;
                Ok(row)
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    }
}

fn typed_batch(
    connection: &Connection,
    schema: &Schema,
    mutations: Vec<Mutation>,
) -> Result<Vec<CommittedChange>, StorageError> {
    let mut changes = Vec::new();
    for mutation in mutations {
        let (table, id, after, is_insert) = match &mutation {
            Mutation::Insert {
                table,
                row,
            } => (table, row.id.as_str(), Some(row), true),
            Mutation::Replace {
                table,
                row,
            } => (table, row.id.as_str(), Some(row), false),
            Mutation::Remove {
                table,
                id,
            } => (table, id.as_str(), None, false),
        };
        if !schema.tables.contains_key(table) {
            return Err(StorageError::TableNotFound {
                table: table.clone(),
            });
        }
        require_table(connection, table)?;
        if let Some(row) = after {
            validate_row(schema, table, row)?;
        }
        // Insertion needs no prior snapshot: preserve the raw duplicate error.
        let before = if is_insert {
            None
        } else {
            get(connection, table, id)?
        };
        if let Some(row) = &before {
            validate_row(schema, table, row)?;
        }
        let change = CommittedChange {
            table: table.clone(),
            id: id.into(),
            before,
            after: after.cloned(),
        };
        match mutation {
            Mutation::Insert {
                table,
                row,
            } => insert(connection, &table, row)?,
            Mutation::Replace {
                table,
                row,
            } => replace(connection, &table, row)?,
            Mutation::Remove {
                table,
                id,
            } => {
                remove(connection, &table, &id)?;
            }
        }
        if change.before != change.after {
            changes.push(change);
        }
    }
    Ok(changes)
}

// [covers: REQ-0040, TC-0025] Snapshots remain private until COMMIT succeeds.
impl TypedTransactionalStorage for SqliteStorage {
    fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send {
        ready(validate_schema(schema).and_then(|()| {
            self.transaction(|connection| typed_batch(connection, schema, mutations))
        }))
    }
}

fn decode_value(
    kind: ValueRef<'_>,
    text: ValueRef<'_>,
    number: ValueRef<'_>,
) -> Result<Value, StorageError> {
    Ok(match (kind, text, number) {
        (ValueRef::Integer(1), ValueRef::Text(value), ValueRef::Null) => {
            Value::String(std::str::from_utf8(value).map_err(backend)?.to_owned())
        }
        (ValueRef::Integer(2), ValueRef::Null, ValueRef::Integer(value)) => Value::Integer(value),
        (ValueRef::Integer(3), ValueRef::Null, ValueRef::Integer(value @ (0 | 1))) => {
            Value::Boolean(value == 1)
        }
        (ValueRef::Integer(4), ValueRef::Null, ValueRef::Real(value)) => {
            Value::Float(FiniteFloat::new(value).ok_or_else(|| backend("nonfinite stored float"))?)
        }
        (ValueRef::Integer(4), ValueRef::Null, ValueRef::Integer(value)) => {
            let number = value as f64;
            // i64 backcasts saturate at the extrema, hiding rounded i64::MAX.
            if number as i128 != i128::from(value) {
                return Err(backend("stored float integer is not exactly representable"));
            }
            Value::Float(FiniteFloat::new(number).ok_or_else(|| backend("nonfinite stored float"))?)
        }
        (ValueRef::Integer(5), ValueRef::Null, ValueRef::Integer(value)) => Value::Timestamp(value),
        (ValueRef::Integer(6), ValueRef::Text(value), ValueRef::Null) => {
            Value::Reference(std::str::from_utf8(value).map_err(backend)?.to_owned())
        }
        (ValueRef::Integer(7), ValueRef::Null, ValueRef::Null) => Value::Null,
        _ => return Err(backend("invalid stored field discriminator or payload")),
    })
}

fn encode_value(value: &Value) -> (i64, Option<&str>, SqlValue) {
    match value {
        Value::String(value) => (1, Some(value.as_str()), SqlValue::Null),
        Value::Integer(value) => (2, None, SqlValue::Integer(*value)),
        Value::Boolean(value) => (3, None, SqlValue::Integer(i64::from(*value))),
        Value::Float(value) => (4, None, SqlValue::Real(value.get())),
        Value::Timestamp(value) => (5, None, SqlValue::Integer(*value)),
        Value::Reference(value) => (6, Some(value.as_str()), SqlValue::Null),
        Value::Null => (7, None, SqlValue::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll, Waker};

    fn run<F: Future>(future: F) -> F::Output {
        match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("SQLite operation suspended"),
        }
    }

    fn row(id: &str) -> Row {
        Row {
            id: id.into(),
            fields: BTreeMap::from([
                ("a".into(), Value::String("保留\0'; DROP TABLE x;--".into())),
                ("b".into(), Value::Integer(i64::MIN)),
                ("c".into(), Value::Boolean(true)),
            ]),
        }
    }

    fn backend_failure<T>(result: Result<T, StorageError>) {
        assert!(matches!(result, Err(StorageError::BackendFailure { .. })));
    }

    fn count(connection: &Connection, sql: &str) -> i64 {
        connection.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    /// TC-0041 SI-Q01: the DESC walk issues at most d+1 next-value queries, d
    /// equality ranges and k exact-id row gets for d distinct values visited.
    #[test]
    fn si_q01_desc_walk_statement_bounds() {
        use hoyadb_core::{command::*, model::*, storage::counters};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "hoya-walk-{}-{}.sqlite",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let schema = SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([(
                "items".into(),
                TableDefinition {
                    columns: BTreeMap::from([(
                        "rank".into(),
                        hoyadb_core::storage::Column {
                            kind: ColumnType::Integer,
                            optional: false,
                        },
                    )]),
                    ..Default::default()
                },
            )]),
        };
        let mut db = run(CommandDatabase::open(
            SqliteStorage::open_unprotected(&path).unwrap(),
            schema,
            vec![],
            [0x77; 16],
            SyncOptions::default(),
        ))
        .unwrap();
        for (rank, count) in [(500, 3), (400, 1), (300, 4)] {
            for _ in 0..count {
                run(db.insert(
                    "items",
                    BTreeMap::from([("rank".into(), Value::Integer(rank))]),
                ))
                .unwrap();
            }
        }
        // (k, distinct values visited d, next-value queries, equality ranges)
        for (limit, d, next, equal) in [(5, 3, 3, 3), (4, 2, 2, 2), (20, 3, 4, 3), (1, 1, 1, 1)] {
            WALK.with(|walk| walk.set((0, 0)));
            counters::reset();
            let rows = run(db.fetch(
                &db.query("items")
                    .order_by("rank", Direction::Desc)
                    .limit(limit),
            ))
            .unwrap();
            let (queried_next, queried_equal) = WALK.with(|walk| walk.get());
            assert_eq!((queried_next, queried_equal), (next, equal), "k={limit}");
            assert!(queried_next <= d + 1 && queried_equal <= d, "k={limit}");
            assert_eq!(rows.len(), limit.min(8));
            assert_eq!(counters::snapshot().sqlite_rows_read, rows.len() as u64);
        }
        drop(db);
        std::fs::remove_file(path).unwrap();
    }

    /// TC-0041 SI-K01 (q05 moved): a deferred foreign key fails the engine's
    /// own COMMIT, so the adapter's transaction must roll back what it wrote.
    #[test]
    fn si_k01_real_commit_failure_recovers_without_publication() {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "hoya-commit-fault-{}-{}.sqlite",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut store = SqliteStorage::open_unprotected(&path).unwrap();
        run(store.create_table("orders")).unwrap();
        run(store.insert("orders", row("seed"))).unwrap();
        store.connection.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE commit_guard_parent(id INTEGER PRIMARY KEY); CREATE TABLE commit_guard_child(parent INTEGER REFERENCES commit_guard_parent(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER commit_fault AFTER INSERT ON __hoya_fields WHEN NEW.table_name='orders' BEGIN INSERT INTO commit_guard_child(parent) VALUES(1); END;").unwrap();
        // The violating statement succeeds; only the COMMIT itself fails.
        store
            .connection
            .execute_batch("BEGIN; INSERT INTO commit_guard_child(parent) VALUES(1);")
            .unwrap();
        let commit = store.connection.execute_batch("COMMIT").unwrap_err();
        assert_eq!(
            commit.sqlite_extended_error_code(),
            Some(rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY)
        );
        store.connection.execute_batch("ROLLBACK").unwrap();
        let mut changed = row("seed");
        changed.fields.insert("extra".into(), Value::Boolean(false));
        backend_failure(run(store.replace("orders", changed)));
        backend_failure(run(store.apply_batch(vec![
            Mutation::Insert {
                table: "orders".into(),
                row: row("valid-prefix"),
            },
            Mutation::Insert {
                table: "orders".into(),
                row: row("commit-failure"),
            },
        ])));
        assert!(store.connection.is_autocommit());
        assert_eq!(
            count(&store.connection, "SELECT COUNT(*) FROM commit_guard_child"),
            0
        );
        assert_eq!(run(store.get("orders", "seed")), Ok(Some(row("seed"))));
        assert_eq!(run(store.get("orders", "valid-prefix")), Ok(None));
        assert_eq!(run(store.get("orders", "commit-failure")), Ok(None));
        assert_eq!(
            count(
                &store.connection,
                "SELECT COUNT(*) FROM __hoya_rows WHERE table_name='orders'"
            ),
            1
        );
        store.connection.execute_batch("DROP TRIGGER commit_fault; DROP TABLE commit_guard_child; DROP TABLE commit_guard_parent; PRAGMA foreign_keys=OFF;").unwrap();
        let integrity: String = store
            .connection
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .unwrap();
        assert_eq!(integrity, "ok");
        assert_eq!(
            count(
                &store.connection,
                "SELECT COUNT(*) FROM sqlite_schema WHERE type='index' AND name IN ('__hoya_fields_numeric','__hoya_fields_text')"
            ),
            2
        );
        run(store.insert("orders", row("recovered"))).unwrap();
        assert_eq!(
            run(store.get("orders", "recovered")),
            Ok(Some(row("recovered")))
        );
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
}
