//! SPEC-0023 / TC-0023: test-only bridge; no production JS API or thread transfer.
use hoyadb_core::storage::{
    FiniteFloat, Mutation, Row, Storage, StorageError, TransactionalStorage, Value,
};
use hoyadb_storage_sqlite::SqliteStorage;
use js_sys::{Array, Object, Reflect};
use sqlite_wasm_rs::vfs::VfsFilesManager;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::future::Future;
use std::task::{Context, Poll, Waker};
use wasm_bindgen::prelude::*;

fn object(fields: &[(&str, JsValue)]) -> JsValue {
    let object = Object::new();
    for (key, value) in fields {
        Reflect::set(&object, &JsValue::from_str(key), value).expect("owned plain object");
    }
    object.into()
}

mod error_boundary;

fn backend(_message: impl ToString) -> JsValue {
    object(&[("kind", "BackendFailure".into())])
}

fn error(error: StorageError) -> JsValue {
    let fields = error_boundary::fields(error)
        .into_iter()
        .map(|(name, value)| (name, JsValue::from_str(&value)))
        .collect::<Vec<_>>();
    object(&fields)
}

fn run<F: Future + Send>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("SQLite must return an already ready Send future"),
    }
}

#[wasm_bindgen]
pub async fn initialize(namespace: &str) -> Result<(), JsValue> {
    if !namespace.starts_with("hoyadb-spec23-")
        || namespace.len() != 50
        || !namespace[14..]
            .bytes()
            .all(|b| b.is_ascii_hexdigit() || b == b'-')
    {
        return Err(backend("invalid test-owned namespace"));
    }
    if !js_sys::global().is_instance_of::<web_sys::DedicatedWorkerGlobalScope>() {
        return Err(backend("dedicated worker required"));
    }
    // A regression assertion: no named OPFS VFS exists before installation.
    if SqliteStorage::open_unprotected("before-init.sqlite").is_ok() {
        return Err(backend("unexpected adapter before OPFS installation"));
    }
    let cfg = sqlite_wasm_vfs::sahpool::OpfsSAHPoolCfgBuilder::new()
        .directory(namespace)
        .clear_on_init(false)
        .initial_capacity(24)
        .build();
    let pool = sqlite_wasm_vfs::sahpool::install::<sqlite_wasm_rs::WasmOsCallback>(&cfg, true)
        .await
        .map_err(backend)?;
    POOL.with(|slot| *slot.borrow_mut() = Some(pool));
    Ok(())
}

// SPEC-0024 / TC-0024: connection disposal alone does not release pooled SAHs.
thread_local! {
    static POOL: RefCell<Option<sqlite_wasm_vfs::sahpool::OpfsSAHPoolUtil>> = const { RefCell::new(None) };
}

#[wasm_bindgen]
pub fn shutdown() -> Result<(), JsValue> {
    POOL.with(|slot| {
        let pool = slot.borrow();
        pool.as_ref()
            .ok_or_else(|| backend("VFS not initialized"))?
            .pause()
            .map_err(backend)
    })
}

#[wasm_bindgen]
pub struct TypedRow {
    row: Row,
}

#[wasm_bindgen]
impl TypedRow {
    #[wasm_bindgen(constructor)]
    pub fn new(id: &str) -> Self {
        Self {
            row: Row {
                id: id.into(),
                fields: BTreeMap::new(),
            },
        }
    }

    pub fn string(&mut self, name: &str, value: &str) -> Result<(), JsValue> {
        self.add(name, Value::String(value.into()))
    }

    pub fn integer(&mut self, name: &str, decimal: &str) -> Result<(), JsValue> {
        self.add(
            name,
            Value::Integer(decimal.parse::<i64>().map_err(backend)?),
        )
    }

    pub fn float(&mut self, name: &str, value: f64) -> Result<(), JsValue> {
        self.add(
            name,
            Value::Float(FiniteFloat::new(value).ok_or_else(|| backend("finite float required"))?),
        )
    }

    pub fn timestamp(&mut self, name: &str, decimal: &str) -> Result<(), JsValue> {
        self.add(
            name,
            Value::Timestamp(decimal.parse::<i64>().map_err(backend)?),
        )
    }

    pub fn reference(&mut self, name: &str, value: &str) -> Result<(), JsValue> {
        self.add(name, Value::Reference(value.into()))
    }

    pub fn null(&mut self, name: &str) -> Result<(), JsValue> {
        self.add(name, Value::Null)
    }

    pub fn boolean(&mut self, name: &str, value: bool) -> Result<(), JsValue> {
        self.add(name, Value::Boolean(value))
    }
}

impl TypedRow {
    fn add(&mut self, name: &str, value: Value) -> Result<(), JsValue> {
        if self.row.fields.contains_key(name) {
            return Err(backend("duplicate field"));
        }
        self.row.fields.insert(name.into(), value);
        Ok(())
    }
}

#[wasm_bindgen]
pub struct Batch {
    mutations: Vec<Mutation>,
}

#[wasm_bindgen]
impl Batch {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            mutations: Vec::new(),
        }
    }

    pub fn insert(&mut self, table: &str, row: &TypedRow) {
        self.mutations.push(Mutation::Insert {
            table: table.into(),
            row: row.row.clone(),
        });
    }

    pub fn replace(&mut self, table: &str, row: &TypedRow) {
        self.mutations.push(Mutation::Replace {
            table: table.into(),
            row: row.row.clone(),
        });
    }

    pub fn remove(&mut self, table: &str, id: &str) {
        self.mutations.push(Mutation::Remove {
            table: table.into(),
            id: id.into(),
        });
    }
}

impl Default for Batch {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
pub struct BrowserStore {
    storage: SqliteStorage,
}

#[wasm_bindgen]
impl BrowserStore {
    #[wasm_bindgen(constructor)]
    pub fn new(name: &str) -> Result<Self, JsValue> {
        Ok(Self {
            storage: SqliteStorage::open_unprotected(name).map_err(error)?,
        })
    }

    pub fn create_table(&mut self, table: &str) -> Result<(), JsValue> {
        run(self.storage.create_table(table)).map_err(error)
    }

    pub fn insert(&mut self, table: &str, row: &TypedRow) -> Result<(), JsValue> {
        run(self.storage.insert(table, row.row.clone())).map_err(error)
    }

    pub fn replace(&mut self, table: &str, row: &TypedRow) -> Result<(), JsValue> {
        run(self.storage.replace(table, row.row.clone())).map_err(error)
    }

    pub fn remove(&mut self, table: &str, id: &str) -> Result<bool, JsValue> {
        run(self.storage.remove(table, id)).map_err(error)
    }

    pub fn batch(&mut self, batch: &Batch) -> Result<(), JsValue> {
        run(self.storage.apply_batch(batch.mutations.clone())).map_err(error)
    }

    pub fn get(&self, table: &str, id: &str) -> Result<JsValue, JsValue> {
        let Some(row) = run(self.storage.get(table, id)).map_err(error)? else {
            return Ok(JsValue::NULL);
        };
        let fields = Array::new();
        for (name, value) in row.fields {
            let (kind, value) = match value {
                Value::String(v) => ("String", JsValue::from_str(&v)),
                Value::Integer(v) => ("Integer", JsValue::from_str(&v.to_string())),
                Value::Boolean(v) => ("Boolean", JsValue::from_bool(v)),
                Value::Float(v) => ("Float", JsValue::from_f64(v.get())),
                Value::Timestamp(v) => ("Timestamp", JsValue::from_str(&v.to_string())),
                Value::Reference(v) => ("Reference", JsValue::from_str(&v)),
                Value::Null => ("Null", JsValue::NULL),
            };
            fields.push(&object(&[
                ("name", name.into()),
                ("kind", kind.into()),
                ("value", value),
            ]));
        }
        Ok(object(&[("id", row.id.into()), ("fields", fields.into())]))
    }
}

// The original assertions run unchanged except the explicit M03 instance exclusion
// and a single-poll WASM executor. Native thread-parking executor stays intact.
thread_local! { static NEXT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }

fn fresh() -> SqliteStorage {
    NEXT.with(|next| {
        let n = next.get();
        next.set(n + 1);
        fixture_open(format!("conformance-{n}.sqlite")).unwrap()
    })
}

// Each shared case closes its storage before returning. Delete only harness-owned
// conformance files so the bounded SAH pool is not exhausted by independent cases.
fn cleanup_conformance() {
    POOL.with(|slot| {
        let pool = slot.borrow();
        let pool = pool.as_ref().expect("initialized fixture pool");
        for name in pool.names().expect("active owned pool") {
            let literal = name.trim_start_matches('/');
            if literal.starts_with("conformance-") && literal.ends_with(".sqlite") {
                pool.remove(&name).expect("closed conformance fixture");
            }
        }
    });
}

#[path = "../../../tests/conformance/storage.rs"]
mod conformance;
#[wasm_bindgen]
pub fn conformance() -> Array {
    let cases: [(&str, fn()); 15] = [
        ("M01", conformance::m01_typed_roundtrip),
        ("M02", conformance::m02_reads_are_owned_snapshots),
        (
            "M04",
            conformance::m04_duplicate_insert_preserves_existing_row,
        ),
        (
            "M05",
            conformance::m05_unknown_table_rejects_every_operation,
        ),
        ("M06", conformance::m06_missing_read_and_replacement),
        ("M07", conformance::m07_replacement_replaces_all_fields),
        (
            "M08",
            conformance::m08_removal_reports_existing_and_missing_rows,
        ),
        (
            "M09",
            conformance::m09_creating_existing_table_preserves_rows,
        ),
        ("T01", conformance::t01_mixed_cross_table_batch_commits),
        (
            "T02",
            conformance::t02_batch_observes_prior_mutations_in_order,
        ),
        (
            "T03",
            conformance::t03_duplicate_rolls_back_all_tables_and_later_mutations,
        ),
        (
            "T04",
            conformance::t04_missing_replacement_restores_removed_row,
        ),
        (
            "T05",
            conformance::t05_unknown_table_rolls_back_for_each_mutation_kind,
        ),
        ("T06", conformance::t06_first_typed_error_is_preserved),
        (
            "T07",
            conformance::t07_empty_and_missing_removal_batches_preserve_rows,
        ),
    ];
    let passed = Array::new();
    for (name, test) in cases {
        test();
        cleanup_conformance();
        passed.push(&name.into());
    }
    let mut storage = fresh();
    for table in ["first", "second"] {
        run(storage.create_table(table)).unwrap();
    }
    let a = Row {
        id: "same".into(),
        fields: BTreeMap::new(),
    };
    let mut b = a.clone();
    b.fields.insert("b".into(), Value::Boolean(true));
    run(storage.insert("first", a.clone())).unwrap();
    run(storage.insert("second", b.clone())).unwrap();
    assert_eq!(run(storage.get("first", "same")), Ok(Some(a)));
    assert_eq!(run(storage.get("second", "same")), Ok(Some(b)));
    passed.push(&"M03 logical tables only; simultaneous instances excluded".into());
    drop(storage);
    cleanup_conformance();
    passed
}

#[wasm_bindgen]
pub fn fixture(name: &str, kind: &str, create: bool) -> Result<JsValue, JsValue> {
    if !matches!(
        (name, kind),
        ("unrelated.sqlite", "unrelated") | ("version.sqlite", "version")
    ) {
        return Err(backend("invalid fixture"));
    }
    if create && kind == "version" {
        drop(SqliteStorage::open_unprotected(name).map_err(error)?);
    }
    let connection = rusqlite::Connection::open_with_flags_and_vfs(
        name,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_CREATE,
        "opfs-sahpool",
    )
    .map_err(backend)?;
    if create {
        if kind == "unrelated" {
            connection.execute_batch("CREATE TABLE sentinel(value TEXT); INSERT INTO sentinel VALUES ('preserve 🌿');").map_err(backend)?;
        } else {
            connection
                .execute("UPDATE __hoya_meta SET version=2", [])
                .map_err(backend)?;
        }
    }
    let schema = Array::new();
    let mut statement = connection
        .prepare("SELECT name,sql FROM sqlite_schema ORDER BY name")
        .map_err(backend)?;
    let mut rows = statement.query([]).map_err(backend)?;
    while let Some(row) = rows.next().map_err(backend)? {
        let pair = Array::new();
        for column in 0..2 {
            pair.push(
                &row.get::<_, Option<String>>(column)
                    .map_err(backend)?
                    .map(JsValue::from)
                    .unwrap_or(JsValue::NULL),
            );
        }
        schema.push(&pair);
    }
    let data: String = if kind == "unrelated" {
        connection
            .query_row("SELECT value FROM sentinel", [], |r| r.get(0))
            .map_err(backend)?
    } else {
        connection
            .query_row("SELECT version FROM __hoya_meta", [], |r| {
                r.get::<_, i64>(0)
            })
            .map_err(backend)?
            .to_string()
    };
    Ok(object(&[("schema", schema.into()), ("data", data.into())]))
}

#[path = "../../../tests/conformance/typed_storage.rs"]
mod typed_conformance_cases;
/// TC-0025 P01–P10, the same assertions used by memory and native SQLite.
#[wasm_bindgen]
pub fn typed_conformance() -> Array {
    let cases: [(&str, fn()); 10] = [
        ("P01", typed_conformance_cases::p01_all_primitives_roundtrip),
        (
            "P02",
            typed_conformance_cases::p02_schema_and_row_validation,
        ),
        (
            "P03",
            typed_conformance_cases::p03_every_operator_every_primitive,
        ),
        (
            "P04",
            typed_conformance_cases::p04_optional_null_comparison_and_order,
        ),
        ("P05", typed_conformance_cases::p05_filter_order_limit_ties),
        (
            "P06",
            typed_conformance_cases::p06_direct_to_one_missing_and_cycles,
        ),
        (
            "P07",
            typed_conformance_cases::p07_invalid_queries_before_execution,
        ),
        (
            "P08",
            typed_conformance_cases::p08_committed_changes_order_owned_noops,
        ),
        (
            "P09",
            typed_conformance_cases::p09_rollback_and_invalid_returned_rows,
        ),
        (
            "P10",
            typed_conformance_cases::p10_all_write_paths_keep_queries_coherent,
        ),
    ];
    let passed = Array::new();
    for (name, test) in cases {
        test();
        cleanup_conformance();
        passed.push(&name.into());
    }
    passed
}

mod typed_evidence;
pub use typed_evidence::{typed_index_evidence, typed_physical};

mod model_evidence;
pub use model_evidence::{
    model_conformance, model_fixture_read, model_fixture_write, model_physical,
    reactive_conformance,
};

mod reactive_bridge;
pub use reactive_bridge::{ReactiveStore, review_large_sets};
mod command_bridge;
pub use command_bridge::CommandFixture;

mod command_conformance;
pub use command_conformance::command_conformance;
mod entity_conformance;
pub use entity_conformance::entity_conformance;

mod security_conformance;
use security_conformance::fixture_open;
pub use security_conformance::security_conformance;
