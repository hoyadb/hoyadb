//! SPEC-0027 / TC-0029 R14: fixed native fixture over the production Rust model.
//! All pointers are call-scoped; no foreign callback runs under a registry lock.
use hoyadb_core::{
    model::{
        Database, ObservationValue, ReactiveDatabase, SchemaDefinition, Subscription,
        TableDefinition,
    },
    storage::{
        Column, ColumnType, CommittedChange, ModelCatalog, Mutation, ObservationStorage, Operator,
        ProjectedRow, Query, Row, Schema, SchemaPlan, SchemaStorage, Storage, StorageError,
        TransactionalStorage, TypedStorage, TypedTransactionalStorage, Value,
    },
};
use hoyadb_storage_memory::MemoryStorage;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ffi::{c_char, c_void},
    future::{Future, ready},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll, Waker},
};

#[repr(C)]
pub struct RowView {
    id: *const u8,
    id_len: usize,
    status: *const u8,
    status_len: usize,
    notes: *const u8,
    notes_len: usize,
    total_cents: i64,
}

type RowsVisitor = unsafe extern "C" fn(*mut c_void, *const RowView, usize);

type EventVisitor = unsafe extern "C" fn(*mut c_void, u64, u8, usize, *const RowView, usize, u32);
// TC-0031 Q05: only this synthetic native fixture can arm a projected read
// failure. All ordinary writes, queries and lifecycle operations delegate.
struct FixtureStorage {
    inner: MemoryStorage,
    fail_next_project: Arc<AtomicBool>,
}

impl Storage for FixtureStorage {
    fn create_table(
        &mut self,
        name: &str,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.create_table(name)
    }

    fn insert(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.insert(table, row)
    }

    fn get(
        &self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<Option<Row>, StorageError>> + Send {
        self.inner.get(table, id)
    }

    fn replace(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.replace(table, row)
    }

    fn remove(
        &mut self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send {
        self.inner.remove(table, id)
    }
}

impl TransactionalStorage for FixtureStorage {
    fn apply_batch(
        &mut self,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.apply_batch(mutations)
    }
}

impl TypedStorage for FixtureStorage {
    fn query(
        &self,
        schema: &Schema,
        query: &Query,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.inner.query(schema, query)
    }
}

impl TypedTransactionalStorage for FixtureStorage {
    fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send {
        self.inner.apply_typed_batch(schema, mutations)
    }
}

impl SchemaStorage for FixtureStorage {
    fn catalog(&self) -> impl Future<Output = Result<ModelCatalog, StorageError>> + Send {
        self.inner.catalog()
    }

    fn scan(&self, table: &str) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.inner.scan(table)
    }

    fn apply_schema_plan(
        &mut self,
        plan: SchemaPlan,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.apply_schema_plan(plan)
    }
}

impl ObservationStorage for FixtureStorage {
    fn project(
        &self,
        schema: &Schema,
        query: &Query,
        fields: &[String],
    ) -> impl Future<Output = Result<Vec<ProjectedRow>, StorageError>> + Send {
        ready(if self.fail_next_project.swap(false, Ordering::SeqCst) {
            Err(StorageError::BackendFailure {
                message: "synthetic native fixture projected read failure".into(),
            })
        } else {
            run(self.inner.project(schema, query, fields))
        })
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hoya_reactive_fail_next_project(handle: u64) -> i32 {
    super::status(|| {
        with_store(handle, |entry| {
            entry.fail_next_project.store(true, Ordering::SeqCst);
            Ok(())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn hoya_reactive_refresh(handle: u64) -> i32 {
    super::status(|| {
        with_store(handle, |entry| {
            run(entry.db.refresh_notifications());
            Ok(())
        })
    })
}

struct Entry {
    db: ReactiveDatabase<FixtureStorage>,
    fail_next_project: Arc<AtomicBool>,
    active: HashSet<u64>,
    draining: bool,
}

struct Registry {
    next: u64,
    stores: HashMap<u64, Entry>,
}

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        Mutex::new(Registry {
            next: 1,
            stores: HashMap::new(),
        })
    })
}

fn with_registry<T>(operation: impl FnOnce(&mut Registry) -> Result<T, i32>) -> Result<T, i32> {
    let mut entries = registry().lock().map_err(|_| 3)?;
    operation(&mut entries)
}

fn with_store<T>(
    handle: u64,
    operation: impl FnOnce(&mut Entry) -> Result<T, i32>,
) -> Result<T, i32> {
    with_registry(|entries| operation(entries.stores.get_mut(&handle).ok_or(1)?))
}

fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("synchronous memory fixture unexpectedly suspended"),
    }
}

fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([(
            "orders".into(),
            TableDefinition {
                columns: BTreeMap::from([
                    (
                        "status".into(),
                        Column {
                            kind: ColumnType::String,
                            optional: false,
                        },
                    ),
                    (
                        "notes".into(),
                        Column {
                            kind: ColumnType::String,
                            optional: false,
                        },
                    ),
                    (
                        "total_cents".into(),
                        Column {
                            kind: ColumnType::Integer,
                            optional: false,
                        },
                    ),
                ]),
                relations: BTreeMap::new(),
            },
        )]),
    }
}

fn fields(status: String, notes: String, total: i64) -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("status".into(), Value::String(status)),
        ("notes".into(), Value::String(notes)),
        ("total_cents".into(), Value::Integer(total)),
    ])
}

fn views(rows: &[Row]) -> Result<Vec<RowView>, i32> {
    rows.iter()
        .map(|row| {
            let Some(Value::String(status)) = row.fields.get("status") else {
                return Err(4);
            };
            let Some(Value::String(notes)) = row.fields.get("notes") else {
                return Err(4);
            };
            let Some(Value::Integer(total)) = row.fields.get("total_cents") else {
                return Err(4);
            };
            if row.fields.len() != 3 {
                return Err(4);
            }
            Ok(RowView {
                id: row.id.as_ptr(),
                id_len: row.id.len(),
                status: status.as_ptr(),
                status_len: status.len(),
                notes: notes.as_ptr(),
                notes_len: notes.len(),
                total_cents: *total,
            })
        })
        .collect()
}

fn error_code(error: StorageError) -> u32 {
    match error {
        StorageError::TableNotFound {
            ..
        } => 1,
        StorageError::RowAlreadyExists {
            ..
        } => 2,
        StorageError::RowNotFound {
            ..
        } => 3,
        StorageError::InvalidSchema {
            ..
        } => 4,
        StorageError::InvalidQuery {
            ..
        } => 5,
        StorageError::InvalidRow {
            ..
        } => 6,
        StorageError::BackendFailure {
            ..
        }
        | StorageError::StoreInUse => 7,
        StorageError::SchemaConflict {
            ..
        } => 8,
        StorageError::MigrationFailure {
            ..
        } => 9,
    }
}

struct DrainGuard(u64);

impl Drop for DrainGuard {
    fn drop(&mut self) {
        if let Ok(mut entries) = registry().lock()
            && let Some(store) = entries.stores.get_mut(&self.0)
        {
            store.draining = false;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hoya_reactive_open() -> u64 {
    super::boundary(0, || {
        let fail_next_project = Arc::new(AtomicBool::new(false));
        let storage = FixtureStorage {
            inner: MemoryStorage::default(),
            fail_next_project: fail_next_project.clone(),
        };
        let Ok(db) = run(Database::open(storage, schema(), vec![])) else {
            return 0;
        };
        with_registry(|entries| {
            let next = entries.next.checked_add(1).ok_or(4)?;
            let handle = entries.next;
            entries.stores.insert(
                handle,
                Entry {
                    db: db.into_reactive(),
                    fail_next_project,
                    active: HashSet::new(),
                    draining: false,
                },
            );
            entries.next = next;
            Ok(handle)
        })
        .unwrap_or(0)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn hoya_reactive_close(handle: u64) -> i32 {
    super::status(|| with_registry(|entries| entries.stores.remove(&handle).map(|_| ()).ok_or(1)))
}

/// # Safety
/// Required pointers must satisfy the readable NUL-terminated UTF-8 contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoya_reactive_insert(
    handle: u64,
    status: *const c_char,
    notes: *const c_char,
    total: i64,
) -> i32 {
    super::status(|| {
        // SAFETY: caller guarantees readable inputs; input copies and validates UTF-8.
        let (status, notes) = unsafe { (super::input(status)?, super::input(notes)?) };
        with_store(handle, |entry| {
            run(entry.db.insert("orders", fields(status, notes, total)))
                .map(|_| ())
                .map_err(|_| 4)
        })
    })
}

/// # Safety
/// Required pointers must satisfy the readable NUL-terminated UTF-8 contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoya_reactive_replace(
    handle: u64,
    id: *const c_char,
    status: *const c_char,
    notes: *const c_char,
    total: i64,
) -> i32 {
    super::status(|| {
        // SAFETY: caller guarantees readable inputs; no borrowed bytes are retained.
        let (id, status, notes) = unsafe {
            (
                super::input(id)?,
                super::input(status)?,
                super::input(notes)?,
            )
        };
        with_store(handle, |entry| {
            run(entry
                .db
                .replace("orders", &id, fields(status, notes, total)))
            .map(|_| ())
            .map_err(|_| 4)
        })
    })
}

/// # Safety
/// The ID must point to readable NUL-terminated UTF-8 for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoya_reactive_remove(handle: u64, id: *const c_char) -> i32 {
    super::status(|| {
        // SAFETY: caller guarantees readable input; input returns an owned copy.
        let id = unsafe { super::input(id)? };
        with_store(handle, |entry| {
            run(entry.db.remove("orders", &id))
                .map(|_| ())
                .map_err(|_| 4)
        })
    })
}

/// # Safety
/// Input/context/visitor must remain valid for this call. Visitor cannot unwind
/// or retain borrowed row pointers; copy all needed data before it returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoya_reactive_query(
    handle: u64,
    status: *const c_char,
    visitor: Option<RowsVisitor>,
    context: *mut c_void,
) -> i32 {
    super::status(|| {
        let visitor = visitor.ok_or(2)?;
        // SAFETY: caller guarantees readable query input.
        let status = unsafe { super::input(status)? };
        let rows = with_store(handle, |entry| {
            let query =
                entry
                    .db
                    .query("orders")
                    .where_("status", Operator::Eq, Value::String(status));
            run(entry.db.fetch(&query)).map_err(|_| 4)
        })?;
        let row_views = views(&rows)?;
        // SAFETY: rows and their views own valid callback-scoped bytes. The
        // registry lock and owner borrow ended above, permitting re-entry.
        unsafe { visitor(context, row_views.as_ptr(), row_views.len()) };
        Ok(())
    })
}

/// # Safety
/// Status must point to readable NUL-terminated UTF-8 for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoya_reactive_observe(
    handle: u64,
    status: *const c_char,
    mode: u8,
) -> u64 {
    super::boundary(0, || {
        if mode > 2 {
            return 0;
        }
        // SAFETY: caller guarantees readable query input.
        let Ok(status) = (unsafe { super::input(status) }) else {
            return 0;
        };
        with_store(handle, |entry| {
            let query =
                entry
                    .db
                    .query("orders")
                    .where_("status", Operator::Eq, Value::String(status));
            let subscription = match mode {
                0 => run(query.observe(&mut entry.db)),
                1 => run(query.observe_count(&mut entry.db)),
                _ => run(query.observe_with_columns(&mut entry.db, vec!["total_cents".into()])),
            }
            .map_err(|_| 4)?;
            entry.active.insert(subscription.0);
            Ok(subscription.0)
        })
        .unwrap_or(0)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn hoya_reactive_unsubscribe(handle: u64, subscription: u64) -> i32 {
    super::status(|| {
        with_store(handle, |entry| {
            if entry.db.unsubscribe(Subscription(subscription)) {
                entry.active.remove(&subscription);
                Ok(())
            } else {
                Err(1)
            }
        })
    })
}

/// # Safety
/// Visitor/context must remain valid through this call. No foreign unwind or
/// retained callback pointers/row views are permitted. Copies may be retained.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoya_reactive_drain(
    handle: u64,
    now_ms: u64,
    visitor: Option<EventVisitor>,
    context: *mut c_void,
) -> i32 {
    super::status(|| {
        let visitor = visitor.ok_or(2)?;
        let events = with_store(handle, |entry| {
            if entry.draining {
                return Err(5);
            }
            entry.draining = true;
            Ok(entry.db.drain_notifications(now_ms))
        })?;
        let _guard = DrainGuard(handle);
        for event in events {
            let token = event.subscription.0;
            let active = with_registry(|entries| {
                Ok(entries
                    .stores
                    .get(&handle)
                    .is_some_and(|entry| entry.active.contains(&token)))
            })?;
            if !active {
                continue;
            }
            match event.value {
                Ok(ObservationValue::Rows(rows)) => {
                    let row_views = views(&rows)?;
                    // SAFETY: copied snapshot and views remain alive through this
                    // callback only. No registry lock/owner borrow is held.
                    unsafe {
                        visitor(
                            context,
                            token,
                            0,
                            rows.len(),
                            row_views.as_ptr(),
                            row_views.len(),
                            0,
                        )
                    };
                }
                Ok(ObservationValue::Count(count)) => {
                    // SAFETY: no row pointers; caller owns callback/context.
                    unsafe { visitor(context, token, 1, count, std::ptr::null(), 0, 0) };
                }
                Err(error) => {
                    // SAFETY: no row pointers; preserve the typed error kind.
                    unsafe {
                        visitor(context, token, 2, 0, std::ptr::null(), 0, error_code(error))
                    };
                }
            }
        }
        Ok(())
    })
}
