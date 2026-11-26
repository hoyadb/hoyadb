//! SPEC-0027 / TC-0029 R13: fixed worker fixture over the production reactive owner.
use super::*;
use hoyadb_core::{
    model::{Database, ObservationValue, ReactiveDatabase, Subscription, Write},
    storage::*,
};
use std::{
    future::ready,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

// Fault injection is confined to this experimental test bridge, never adapters.
struct BridgeStorage {
    inner: SqliteStorage,
    fail_project: Arc<AtomicBool>,
    fail_query: Arc<AtomicBool>,
}

impl Storage for BridgeStorage {
    fn create_table(
        &mut self,
        name: &str,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(run(self.inner.create_table(name)))
    }

    fn insert(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(run(self.inner.insert(table, row)))
    }

    fn get(
        &self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<Option<Row>, StorageError>> + Send {
        ready(run(self.inner.get(table, id)))
    }

    fn replace(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(run(self.inner.replace(table, row)))
    }

    fn remove(
        &mut self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send {
        ready(run(self.inner.remove(table, id)))
    }
}

impl TransactionalStorage for BridgeStorage {
    fn apply_batch(
        &mut self,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(run(self.inner.apply_batch(mutations)))
    }
}

impl TypedStorage for BridgeStorage {
    fn query(
        &self,
        schema: &Schema,
        query: &Query,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        ready(if self.fail_query.swap(false, Ordering::SeqCst) {
            Err(StorageError::BackendFailure {
                message: "fixture full-row read failure".into(),
            })
        } else {
            run(self.inner.query(schema, query))
        })
    }
}

impl TypedTransactionalStorage for BridgeStorage {
    fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send {
        ready(run(self.inner.apply_typed_batch(schema, mutations)))
    }
}

impl SchemaStorage for BridgeStorage {
    fn catalog(&self) -> impl Future<Output = Result<ModelCatalog, StorageError>> + Send {
        ready(run(self.inner.catalog()))
    }

    fn scan(&self, table: &str) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        ready(run(self.inner.scan(table)))
    }

    fn apply_schema_plan(
        &mut self,
        plan: SchemaPlan,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(run(self.inner.apply_schema_plan(plan)))
    }
}

impl ObservationStorage for BridgeStorage {
    fn project(
        &self,
        schema: &Schema,
        query: &Query,
        fields: &[String],
    ) -> impl Future<Output = Result<Vec<ProjectedRow>, StorageError>> + Send {
        ready(if self.fail_project.swap(false, Ordering::SeqCst) {
            Err(StorageError::BackendFailure {
                message: "fixture projected read failure".into(),
            })
        } else {
            run(self.inner.project(schema, query, fields))
        })
    }
}

fn local<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("synchronous reactive fixture suspended"),
    }
}

fn row_json(row: Row) -> JsValue {
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
    object(&[("id", row.id.into()), ("fields", fields.into())])
}

fn rows_json(rows: Vec<Row>) -> JsValue {
    rows.into_iter().map(row_json).collect::<Array>().into()
}

#[wasm_bindgen]
pub struct ReactiveStore {
    db: ReactiveDatabase<BridgeStorage>,
    fail_project: Arc<AtomicBool>,
    fail_query: Arc<AtomicBool>,
}

#[wasm_bindgen]
impl ReactiveStore {
    #[wasm_bindgen(constructor)]
    pub fn new(name: &str) -> Result<Self, JsValue> {
        let fail_project = Arc::new(AtomicBool::new(false));
        let fail_query = Arc::new(AtomicBool::new(false));
        let storage = BridgeStorage {
            inner: SqliteStorage::open_unprotected(name).map_err(error)?,
            fail_project: fail_project.clone(),
            fail_query: fail_query.clone(),
        };
        let db = local(Database::open(
            storage,
            super::model_evidence::reactive_schema(),
            vec![],
        ))
        .map_err(error)?
        .into_reactive();
        Ok(Self {
            db,
            fail_project,
            fail_query,
        })
    }

    pub fn observe(&mut self, mode: &str, columns: Array) -> Result<String, JsValue> {
        let query =
            self.db
                .query("orders")
                .where_("status", Operator::Eq, Value::String("pending".into()));
        let token = match mode {
            "rows" => local(query.observe(&mut self.db)),
            "count" => local(query.observe_count(&mut self.db)),
            "columns" => {
                let columns = columns
                    .iter()
                    .map(|v| {
                        v.as_string()
                            .ok_or_else(|| backend("column string required"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                local(query.observe_with_columns(&mut self.db, columns))
            }
            _ => return Err(backend("unknown observation mode")),
        }
        .map_err(error)?;
        Ok(token.0.to_string())
    }

    pub fn unsubscribe(&mut self, token: &str) -> Result<bool, JsValue> {
        Ok(self
            .db
            .unsubscribe(Subscription(token.parse::<u64>().map_err(backend)?)))
    }

    pub fn insert(&mut self, table: &str, input: &TypedRow) -> Result<JsValue, JsValue> {
        local(self.db.insert(table, input.row.fields.clone()))
            .map(row_json)
            .map_err(error)
    }

    pub fn replace(&mut self, table: &str, input: &TypedRow) -> Result<JsValue, JsValue> {
        local(
            self.db
                .replace(table, &input.row.id, input.row.fields.clone()),
        )
        .map(row_json)
        .map_err(error)
    }

    pub fn remove(&mut self, table: &str, id: &str) -> Result<bool, JsValue> {
        local(self.db.remove(table, id)).map_err(error)
    }

    pub fn batch(&mut self, input: &Batch) -> Result<(), JsValue> {
        let writes = input
            .mutations
            .iter()
            .map(|m| match m {
                Mutation::Insert {
                    table,
                    row,
                } => Write::Insert {
                    table: table.clone(),
                    fields: row.fields.clone(),
                },
                Mutation::Replace {
                    table,
                    row,
                } => Write::Replace {
                    table: table.clone(),
                    id: row.id.clone(),
                    fields: row.fields.clone(),
                },
                Mutation::Remove {
                    table,
                    id,
                } => Write::Remove {
                    table: table.clone(),
                    id: id.clone(),
                },
            })
            .collect();
        local(self.db.apply_batch(writes))
            .map(|_| ())
            .map_err(error)
    }

    pub fn fetch(&self) -> Result<JsValue, JsValue> {
        local(
            self.db
                .query("orders")
                .where_("status", Operator::Eq, Value::String("pending".into()))
                .fetch(&self.db),
        )
        .map(rows_json)
        .map_err(error)
    }

    pub fn drain(&mut self, now_ms: &str) -> Result<Array, JsValue> {
        let now = now_ms.parse::<u64>().map_err(backend)?;
        let events = Array::new();
        for event in self.db.drain_notifications(now) {
            let (kind, value) = match event.value {
                Ok(ObservationValue::Rows(rows)) => ("rows", rows_json(rows)),
                Ok(ObservationValue::Count(count)) => ("count", JsValue::from_f64(count as f64)),
                Err(e) => ("error", error(e)),
            };
            events.push(&object(&[
                ("subscription", event.subscription.0.to_string().into()),
                ("kind", kind.into()),
                ("value", value),
            ]));
        }
        Ok(events)
    }

    pub fn refresh(&mut self) {
        local(self.db.refresh_notifications());
    }

    pub fn fail_next_project(&self) {
        self.fail_project.store(true, Ordering::SeqCst);
    }

    pub fn fail_next_query(&self) {
        self.fail_query.store(true, Ordering::SeqCst);
    }

    pub fn stats(&self) -> JsValue {
        let s = self.db.observation_stats();
        object(&[
            ("groups", (s.groups as f64).into()),
            ("observers", (s.observers as f64).into()),
            ("projected_queries", (s.projected_queries as f64).into()),
            ("candidate_updates", (s.candidate_updates as f64).into()),
            ("materialized_rows", (s.materialized_rows as f64).into()),
        ])
    }
}

/// SPEC-0028 / TC-0031 Q01: actual oversized operands cannot allocate SAH files.
#[wasm_bindgen]
pub fn review_large_sets() -> Result<JsValue, JsValue> {
    let mut storage = fresh();
    run(storage.create_table("items")).map_err(error)?;
    run(storage.insert(
        "items",
        Row {
            id: "needle".into(),
            fields: BTreeMap::from([("value".into(), Value::Integer(42))]),
        },
    ))
    .map_err(error)?;
    let schema = Schema {
        tables: BTreeMap::from([(
            "items".into(),
            TableSchema {
                columns: BTreeMap::from([(
                    "value".into(),
                    Column {
                        kind: ColumnType::Integer,
                        optional: false,
                    },
                )]),
                relations: BTreeMap::new(),
            },
        )]),
    };
    let mut values: Vec<_> = (0..33_001)
        .map(|i| Value::String(format!("unrelated-{i}")))
        .collect();
    values.push(Value::String("needle".into()));
    let query = Query {
        table: "items".into(),
        predicates: vec![Predicate {
            field: "id".into(),
            operator: Operator::In,
            values,
        }],
        order: None,
        limit: None,
    };
    let files = || {
        POOL.with(|slot| {
            let mut list = slot
                .borrow()
                .as_ref()
                .unwrap()
                .names()
                .expect("active owned pool");
            list.sort();
            list
        })
    };
    let before = files();
    let selected = run(storage.query(&schema, &query));
    let projected = run(storage.project(&schema, &query, &[]));
    let after = files();
    let (rows, query_error) = match selected {
        Ok(rows) => (
            rows.into_iter().map(row_json).collect::<Array>().into(),
            JsValue::NULL,
        ),
        Err(e) => (JsValue::NULL, error(e)),
    };
    let (keys, project_error) = match projected {
        Ok(keys) => (
            keys.into_iter()
                .map(|r| JsValue::from(r.id))
                .collect::<Array>()
                .into(),
            JsValue::NULL,
        ),
        Err(e) => (JsValue::NULL, error(e)),
    };
    drop(storage);
    cleanup_conformance();
    Ok(object(&[
        ("operands", 33_002.0.into()),
        ("rows", rows),
        ("keys", keys),
        ("queryError", query_error),
        ("projectError", project_error),
        (
            "poolBefore",
            before
                .into_iter()
                .map(JsValue::from)
                .collect::<Array>()
                .into(),
        ),
        (
            "poolAfter",
            after
                .into_iter()
                .map(JsValue::from)
                .collect::<Array>()
                .into(),
        ),
    ]))
}
