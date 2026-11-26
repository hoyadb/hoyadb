//! TC-0026: public API assertions, reused on memory/native/real browser.
use super::fresh;
use hoyadb_core::{model::*, storage::*};
use std::{
    collections::BTreeMap,
    future::Future,
    task::{Context, Poll, Waker},
};

pub fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("fixture adapter unexpectedly suspended"),
    }
}

pub fn column(kind: ColumnType, optional: bool) -> Column {
    Column {
        kind,
        optional,
    }
}

pub fn fields(values: impl IntoIterator<Item = (&'static str, Value)>) -> BTreeMap<String, Value> {
    values
        .into_iter()
        .map(|(name, value)| (name.into(), value))
        .collect()
}

pub fn schema(version: u32) -> SchemaDefinition {
    let mut tables = BTreeMap::new();
    tables.insert(
        "customers".into(),
        TableDefinition {
            columns: BTreeMap::from([("country".into(), column(ColumnType::String, false))]),
            relations: BTreeMap::from([(
                "orders".into(),
                RelationDefinition::HasMany {
                    table: "orders".into(),
                    foreign_key: "customer_id".into(),
                },
            )]),
        },
    );
    tables.insert(
        "orders".into(),
        TableDefinition {
            columns: BTreeMap::from([
                ("status".into(), column(ColumnType::String, false)),
                ("total_cents".into(), column(ColumnType::Integer, false)),
                ("placed_at".into(), column(ColumnType::Timestamp, false)),
                ("price".into(), column(ColumnType::Float, true)),
                ("active".into(), column(ColumnType::Boolean, true)),
                ("notes".into(), column(ColumnType::String, true)),
            ]),
            relations: BTreeMap::from([
                (
                    "customer".into(),
                    RelationDefinition::ToOne {
                        table: "customers".into(),
                        foreign_key: "customer_id".into(),
                        optional: true,
                    },
                ),
                (
                    "items".into(),
                    RelationDefinition::HasMany {
                        table: "order_items".into(),
                        foreign_key: "order_id".into(),
                    },
                ),
                (
                    "products".into(),
                    RelationDefinition::ManyToMany {
                        table: "products".into(),
                        through: "order_products".into(),
                        source_relation: "order".into(),
                        target_relation: "product".into(),
                    },
                ),
            ]),
        },
    );
    tables.insert(
        "products".into(),
        TableDefinition {
            columns: BTreeMap::from([("name".into(), column(ColumnType::String, false))]),
            relations: BTreeMap::from([(
                "orders".into(),
                RelationDefinition::ManyToMany {
                    table: "orders".into(),
                    through: "order_products".into(),
                    source_relation: "product".into(),
                    target_relation: "order".into(),
                },
            )]),
        },
    );
    tables.insert(
        "order_items".into(),
        TableDefinition {
            columns: BTreeMap::from([("quantity".into(), column(ColumnType::Integer, false))]),
            relations: BTreeMap::from([(
                "order".into(),
                RelationDefinition::ToOne {
                    table: "orders".into(),
                    foreign_key: "order_id".into(),
                    optional: false,
                },
            )]),
        },
    );
    tables.insert(
        "order_notes".into(),
        TableDefinition {
            columns: BTreeMap::from([("body".into(), column(ColumnType::String, false))]),
            relations: BTreeMap::new(),
        },
    );
    tables.insert(
        "order_products".into(),
        TableDefinition {
            columns: BTreeMap::new(),
            relations: BTreeMap::from([
                (
                    "order".into(),
                    RelationDefinition::ToOne {
                        table: "orders".into(),
                        foreign_key: "order_id".into(),
                        optional: false,
                    },
                ),
                (
                    "product".into(),
                    RelationDefinition::ToOne {
                        table: "products".into(),
                        foreign_key: "product_id".into(),
                        optional: false,
                    },
                ),
            ]),
        },
    );
    SchemaDefinition {
        version,
        tables,
    }
}

pub fn order(total: i64) -> BTreeMap<String, Value> {
    fields([
        ("status", Value::String("pending".into())),
        ("total_cents", Value::Integer(total)),
        ("placed_at", Value::Timestamp(total)),
    ])
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn d01_declared_schema_typed_records_and_bookkeeping() {
    let definition = schema(1);
    let mut db = run(Database::open(fresh(), definition.clone(), vec![])).unwrap();
    let mut input = order(i64::MAX);
    input.extend(fields([
        ("price", Value::Float(FiniteFloat::new(f64::MAX).unwrap())),
        ("active", Value::Boolean(false)),
        ("notes", Value::Null),
        ("customer_id", Value::Reference("dangling\0🌿".into())),
    ]));
    let created = run(db.insert("orders", input.clone())).unwrap();
    assert_eq!(created.id, "local:1");
    assert_eq!(created.fields, input);
    assert_eq!(
        run(db.query("orders").fetch(&db)),
        Ok(vec![created.clone()])
    );
    let mut changed = run(db.query("orders").fetch(&db)).unwrap();
    changed[0].fields.clear();
    assert_eq!(
        run(db.query("orders").fetch(&db)),
        Ok(vec![created.clone()])
    );
    let store = db.into_storage();
    let catalog = run(store.catalog()).unwrap();
    assert_eq!(catalog.state.as_ref().unwrap().version, 1);
    assert_eq!(catalog.state.unwrap().next_id, 2);
    let raw = run(store.scan("orders")).unwrap();
    let mut expected = created;
    expected
        .fields
        .insert(ROW_VERSION.into(), Value::Integer(1));
    assert_eq!(raw, vec![expected]);
    assert_eq!(
        run(store.catalog()).unwrap().tables,
        definition.tables.keys().cloned().collect::<Vec<_>>()
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn d02_invalid_declarations_and_reserved_inputs_leave_state_unchanged() {
    let mut bad = schema(0);
    for name in ["__hoya_queue", "id", "bad.path", ""] {
        bad.tables.insert(name.into(), TableDefinition::default());
        let (error, store) = match run(Database::open_recoverable(fresh(), bad.clone(), vec![])) {
            Err(error) => error,
            Ok(_) => panic!("invalid declaration opened"),
        };
        assert!(matches!(error, StorageError::InvalidSchema { .. }));
        assert_eq!(
            run(store.catalog()).unwrap(),
            ModelCatalog {
                state: None,
                tables: vec![]
            }
        );
        bad = schema(1);
    }
    let mut db = run(Database::open(fresh(), schema(1), vec![])).unwrap();
    for name in ["__hoya_schema_version", "__hoya_custom", "id"] {
        let mut input = order(7);
        input.insert(name.into(), Value::Integer(1));
        assert!(matches!(
            run(db.insert("orders", input)),
            Err(StorageError::InvalidRow { .. })
        ));
    }
    assert!(
        matches!(run(db.query("invoices").fetch(&db)),Err(StorageError::TableNotFound{table}) if table=="invoices")
    );
    assert!(run(db.query(MODEL_TABLE).fetch(&db)).is_err());
    assert!(matches!(
        run(db.insert("invoices", BTreeMap::new())),
        Err(StorageError::TableNotFound { .. })
    ));
    let store = db.into_storage();
    assert_eq!(run(store.scan("orders")), Ok(vec![]));
    assert_eq!(run(store.catalog()).unwrap().state.unwrap().next_id, 1);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn d03_generated_ids_public_mutations_and_counter_rollback() {
    let mut db = run(Database::open(fresh(), schema(1), vec![])).unwrap();
    let failure = run(db.apply_batch(vec![
        Write::Insert {
            table: "orders".into(),
            fields: order(1),
        },
        Write::Replace {
            table: "orders".into(),
            id: "missing".into(),
            fields: order(2),
        },
    ]));
    assert!(
        matches!(failure,Err(StorageError::RowNotFound{table,id}) if table=="orders"&&id=="missing")
    );
    let first = run(db.insert("orders", order(1))).unwrap();
    assert_eq!(first.id, "local:1");
    let replaced = run(db.replace("orders", &first.id, order(2))).unwrap();
    assert_eq!(replaced.fields, order(2));
    assert_eq!(run(db.remove("orders", &first.id)), Ok(true));
    assert_eq!(run(db.remove("orders", &first.id)), Ok(false));
    let store = db.into_storage();
    assert_eq!(run(store.catalog()).unwrap().state.unwrap().next_id, 2);
    let mut db = run(Database::open(store, schema(1), vec![])).unwrap();
    assert_eq!(run(db.insert("orders", order(3))).unwrap().id, "local:2");
    let mut ids = ["fixed", "", "fixed"].into_iter();
    let mut db = db.with_id_generator(move || ids.next().unwrap().into());
    assert_eq!(run(db.insert("orders", order(4))).unwrap().id, "fixed");
    assert!(matches!(
        run(db.insert("orders", order(5))),
        Err(StorageError::InvalidRow { .. })
    ));
    assert!(matches!(
        run(db.insert("orders", order(6))),
        Err(StorageError::RowAlreadyExists { .. })
    ));
    let store = db.into_storage();
    assert_eq!(run(store.catalog()).unwrap().state.unwrap().next_id, 3);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn d07_identical_reopen_and_schema_drift() {
    let mut db = run(Database::open(fresh(), schema(1), vec![])).unwrap();
    let row = run(db.insert("orders", order(7))).unwrap();
    let store = db.into_storage();
    let original = run(store.catalog()).unwrap();
    let mut changed = schema(1);
    changed
        .tables
        .get_mut("orders")
        .unwrap()
        .columns
        .get_mut("notes")
        .unwrap()
        .optional = false;
    let (error, store) = match run(Database::open_recoverable(store, changed, vec![])) {
        Err(error) => error,
        Ok(_) => panic!("schema drift opened"),
    };
    assert!(matches!(error, StorageError::SchemaConflict { .. }));
    assert_eq!(run(store.catalog()).unwrap(), original);
    let db = run(Database::open(store, schema(1), vec![])).unwrap();
    assert_eq!(run(db.query("orders").fetch(&db)), Ok(vec![row]));
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn d08_initialization_and_compatible_legacy_adoption() {
    let db = run(Database::open(fresh(), schema(4), vec![])).unwrap();
    assert_eq!(db.schema().version, 4);
    drop(db);
    let mut store = fresh();
    run(store.create_table("orders")).unwrap();
    let legacy = Row {
        id: "local:1".into(),
        fields: order(7),
    };
    run(store.insert("orders", legacy.clone())).unwrap();
    let (error, store) = match run(Database::open_recoverable(store, schema(2), vec![])) {
        Err(error) => error,
        Ok(_) => panic!("ambiguous legacy version adopted"),
    };
    assert!(matches!(error, StorageError::SchemaConflict { .. }));
    assert_eq!(run(store.scan("orders")), Ok(vec![legacy.clone()]));
    let mut db = run(Database::open(store, schema(1), vec![])).unwrap();
    assert_eq!(run(db.query("orders").fetch(&db)), Ok(vec![legacy]));
    assert_eq!(run(db.insert("orders", order(8))).unwrap().id, "local:2");
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn d04_public_operators_all_kinds_order_and_limit() {
    for (kind, values) in [
        (
            ColumnType::String,
            vec![
                Value::String("A".into()),
                Value::String("b".into()),
                Value::String("é\0".into()),
            ],
        ),
        (
            ColumnType::Integer,
            vec![
                Value::Integer(i64::MIN),
                Value::Integer(0),
                Value::Integer(i64::MAX),
            ],
        ),
        (
            ColumnType::Float,
            vec![
                Value::Float(FiniteFloat::new(-f64::MAX).unwrap()),
                Value::Float(FiniteFloat::new(f64::from_bits(1)).unwrap()),
                Value::Float(FiniteFloat::new(f64::MAX).unwrap()),
            ],
        ),
        (
            ColumnType::Boolean,
            vec![
                Value::Boolean(false),
                Value::Boolean(true),
                Value::Boolean(true),
            ],
        ),
        (
            ColumnType::Timestamp,
            vec![
                Value::Timestamp(i64::MIN),
                Value::Timestamp(0),
                Value::Timestamp(i64::MAX),
            ],
        ),
        (
            ColumnType::Reference,
            vec![
                Value::Reference("A".into()),
                Value::Reference("b".into()),
                Value::Reference("é\0".into()),
            ],
        ),
    ] {
        let boolean = kind == ColumnType::Boolean;
        let declared = SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([(
                "values".into(),
                TableDefinition {
                    columns: BTreeMap::from([("value".into(), column(kind.clone(), true))]),
                    relations: BTreeMap::new(),
                },
            )]),
        };
        let mut db = run(Database::open(fresh(), declared, vec![])).unwrap();
        let mut rows = Vec::new();
        for value in &values {
            rows.push(run(db.insert("values", fields([("value", value.clone())]))).unwrap());
        }
        for (operator, indices) in [
            (
                Operator::Eq,
                if boolean {
                    vec![1, 2]
                } else {
                    vec![1]
                },
            ),
            (
                Operator::Ne,
                if boolean {
                    vec![0]
                } else {
                    vec![0, 2]
                },
            ),
            (Operator::Lt, vec![0]),
            (
                Operator::Le,
                if boolean {
                    vec![0, 1, 2]
                } else {
                    vec![0, 1]
                },
            ),
            (
                Operator::Gt,
                if boolean {
                    vec![]
                } else {
                    vec![2]
                },
            ),
            (Operator::Ge, vec![1, 2]),
        ] {
            let expected = indices
                .into_iter()
                .map(|i| rows[i].clone())
                .collect::<Vec<_>>();
            assert_eq!(
                run(db
                    .query("values")
                    .where_("value", operator, values[1].clone())
                    .fetch(&db)),
                Ok(expected)
            );
        }
        let expected = if boolean {
            rows.clone()
        } else {
            vec![rows[0].clone(), rows[2].clone()]
        };
        assert_eq!(
            run(db
                .query("values")
                .where_in(
                    "value",
                    vec![values[0].clone(), values[2].clone(), values[2].clone()]
                )
                .fetch(&db)),
            Ok(expected)
        );
        assert_eq!(
            run(db.query("values").where_in("value", vec![]).fetch(&db)),
            Ok(vec![])
        );
        assert_eq!(
            run(db
                .query("values")
                .where_("id", Operator::Eq, Value::String(rows[1].id.clone()))
                .fetch(&db)),
            Ok(vec![rows[1].clone()])
        );
        assert_eq!(
            run(db
                .query("values")
                .order_by("value", Direction::Desc)
                .limit(1)
                .fetch(&db)),
            Ok(vec![
                rows[if boolean {
                    1
                } else {
                    2
                }]
                .clone()
            ])
        );
        assert_eq!(
            run(db
                .query("values")
                .order_by("value", Direction::Asc)
                .limit(0)
                .fetch(&db)),
            Ok(vec![])
        );
        assert!(matches!(
            run(db
                .query("values")
                .where_("missing", Operator::Eq, values[0].clone())
                .limit(0)
                .fetch(&db)),
            Err(StorageError::InvalidQuery { .. })
        ));
        let wrong = if kind == ColumnType::String {
            Value::Integer(1)
        } else {
            Value::String("bad".into())
        };
        assert!(matches!(
            run(db
                .query("values")
                .where_("value", Operator::Eq, wrong)
                .fetch(&db)),
            Err(StorageError::InvalidQuery { .. })
        ));
        let omitted = run(db.insert("values", BTreeMap::new())).unwrap();
        let null = run(db.insert("values", fields([("value", Value::Null)]))).unwrap();
        assert_eq!(
            run(db
                .query("values")
                .where_("value", Operator::Eq, Value::Null)
                .fetch(&db)),
            Ok(vec![omitted.clone(), null.clone()])
        );
        assert_eq!(
            run(db
                .query("values")
                .where_("value", Operator::Ne, Value::Null)
                .fetch(&db)),
            Ok(rows)
        );
        assert_eq!(
            run(db
                .query("values")
                .order_by("value", Direction::Asc)
                .limit(2)
                .fetch(&db)),
            Ok(vec![omitted, null])
        );
    }
}

struct ReadCount<S> {
    inner: S,
    reads: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    scans: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl<S> ReadCount<S> {
    fn read(&self) {
        self.reads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

impl<S: SchemaStorage> Storage for ReadCount<S> {
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
        self.read();
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

impl<S: SchemaStorage> TransactionalStorage for ReadCount<S> {
    fn apply_batch(
        &mut self,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.apply_batch(mutations)
    }
}

impl<S: SchemaStorage> TypedStorage for ReadCount<S> {
    fn query(
        &self,
        schema: &Schema,
        query: &Query,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.read();
        self.inner.query(schema, query)
    }
}

impl<S: SchemaStorage> TypedTransactionalStorage for ReadCount<S> {
    fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send {
        self.inner.apply_typed_batch(schema, mutations)
    }
}

impl<S: SchemaStorage> SchemaStorage for ReadCount<S> {
    fn catalog(&self) -> impl Future<Output = Result<ModelCatalog, StorageError>> + Send {
        self.read();
        self.inner.catalog()
    }

    fn scan(&self, table: &str) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.read();
        self.scans
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.inner.scan(table)
    }

    fn apply_schema_plan(
        &mut self,
        plan: SchemaPlan,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.inner.apply_schema_plan(plan)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn d05_lazy_to_one_has_many_and_engine_owned_fetch() {
    let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let scans = reads.clone();
    let store = ReadCount {
        inner: fresh(),
        reads: reads.clone(),
        scans: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    };
    let scan_count = store.scans.clone();
    let mut db = run(Database::open(store, schema(1), vec![])).unwrap();
    let fr = run(db.insert(
        "customers",
        fields([("country", Value::String("FR".into()))]),
    ))
    .unwrap();
    let de = run(db.insert(
        "customers",
        fields([("country", Value::String("DE".into()))]),
    ))
    .unwrap();
    let mut input = order(4200);
    input.insert("customer_id".into(), Value::Reference(fr.id.clone()));
    let order_row = run(db.insert("orders", input)).unwrap();
    let before = reads.load(std::sync::atomic::Ordering::Relaxed);
    let lazy = db.related("orders", &order_row.id, "customer");
    let other = db.related("customers", &de.id, "orders");
    assert_eq!(reads.load(std::sync::atomic::Ordering::Relaxed), before);
    let mut input = order(4200);
    input.insert("customer_id".into(), Value::Reference(de.id.clone()));
    let updated = run(db.replace("orders", &order_row.id, input)).unwrap();
    assert_eq!(run(lazy.fetch(&db)), Ok(vec![de.clone()]));
    assert_eq!(run(other.fetch(&db)), Ok(vec![updated.clone()]));
    assert_eq!(
        run(db
            .query("orders")
            .where_("customer.country", Operator::Eq, Value::String("DE".into()))
            .fetch(&db)),
        Ok(vec![updated])
    );
    assert!(matches!(
        run(db
            .query("orders")
            .where_("items.quantity", Operator::Gt, Value::Integer(5))
            .fetch(&db)),
        Err(StorageError::InvalidQuery { .. })
    ));
    assert_eq!(
        run(db.related("orders", "missing", "customer").fetch(&db)),
        Ok(vec![])
    );
    assert!(matches!(
        run(db
            .related("orders", "missing", "unknown")
            .limit(0)
            .fetch(&db)),
        Err(StorageError::InvalidQuery { .. })
    ));
    assert_eq!(scan_count.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert!(scans.load(std::sync::atomic::Ordering::Relaxed) > before);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn d06_many_to_many_managed_links_and_both_directions() {
    let mut db = run(Database::open(fresh(), schema(1), vec![])).unwrap();
    let order_row = run(db.insert("orders", order(4200))).unwrap();
    let p1 = run(db.insert("products", fields([("name", Value::String("A".into()))]))).unwrap();
    let p2 = run(db.insert("products", fields([("name", Value::String("B".into()))]))).unwrap();
    let lazy = db.related("orders", &order_row.id, "products");
    let link = run(db.link("orders", &order_row.id, "products", &p1.id, BTreeMap::new())).unwrap();
    assert_eq!(
        run(db.link("orders", &order_row.id, "products", &p1.id, BTreeMap::new())),
        Ok(link)
    );
    run(db.link("orders", &order_row.id, "products", &p2.id, BTreeMap::new())).unwrap();
    assert_eq!(run(lazy.fetch(&db)), Ok(vec![p1.clone(), p2.clone()]));
    assert_eq!(
        run(db.related("products", &p1.id, "orders").fetch(&db)),
        Ok(vec![order_row.clone()])
    );
    assert_eq!(
        run(db
            .related("orders", &order_row.id, "products")
            .where_("name", Operator::Ne, Value::String("absent".into()))
            .order_by("name", Direction::Desc)
            .limit(1)
            .fetch(&db)),
        Ok(vec![p2.clone()])
    );
    run(db.insert(
        "order_products",
        fields([
            ("order_id", Value::Reference(order_row.id.clone())),
            ("product_id", Value::Reference(p1.id.clone())),
        ]),
    ))
    .unwrap();
    assert_eq!(run(lazy.fetch(&db)), Ok(vec![p1.clone(), p2.clone()]));
    assert_eq!(
        run(db.unlink("products", &p1.id, "orders", &order_row.id)),
        Ok(2)
    );
    assert_eq!(run(lazy.fetch(&db)), Ok(vec![p2]));
    assert_eq!(
        run(db.unlink("orders", &order_row.id, "products", &p1.id)),
        Ok(0)
    );
    assert!(
        matches!(run(db.link("orders","missing","products",&p1.id,BTreeMap::new())),Err(StorageError::RowNotFound{table,id}) if table=="orders"&&id=="missing")
    );
    assert!(run(db.link("orders", &order_row.id, "customer", &p1.id, BTreeMap::new())).is_err());
}

pub fn chain() -> (SchemaDefinition, Vec<Migration>) {
    let mut two = schema(2);
    let table = two.tables.get_mut("orders").unwrap();
    let total = table.columns.remove("total_cents").unwrap();
    table.columns.insert("gross_cents".into(), total);
    table
        .columns
        .insert("discount".into(), column(ColumnType::Integer, false));
    two.tables
        .insert("temporary".into(), TableDefinition::default());
    two.tables
        .get_mut("order_notes")
        .unwrap()
        .columns
        .insert("order_id".into(), column(ColumnType::Reference, true));
    let note_relations = BTreeMap::from([(
        "order".into(),
        RelationDefinition::ToOne {
            table: "orders".into(),
            foreign_key: "order_id".into(),
            optional: true,
        },
    )]);
    two.tables.get_mut("order_notes").unwrap().relations = note_relations.clone();
    let first = Migration {
        from_version: 1,
        to: two.clone(),
        steps: vec![
            MigrationStep::AddTable {
                table: "temporary".into(),
                definition: TableDefinition::default(),
            },
            MigrationStep::AddColumn {
                table: "orders".into(),
                name: "discount".into(),
                column: column(ColumnType::Integer, false),
                default: Some(Value::Integer(0)),
            },
            MigrationStep::RenameColumn {
                table: "orders".into(),
                from: "total_cents".into(),
                to: "gross_cents".into(),
            },
            MigrationStep::Backfill {
                table: "orders".into(),
                column: "discount".into(),
                value: Value::Integer(7),
            },
            MigrationStep::AddColumn {
                table: "order_notes".into(),
                name: "order_id".into(),
                column: column(ColumnType::Reference, true),
                default: None,
            },
            MigrationStep::SetRelations {
                table: "order_notes".into(),
                relations: note_relations,
            },
        ],
    };
    let mut three = two;
    three.version = 3;
    three.tables.remove("temporary");
    three
        .tables
        .get_mut("orders")
        .unwrap()
        .columns
        .remove("price");
    three
        .tables
        .get_mut("orders")
        .unwrap()
        .columns
        .remove("active");
    let note_relations = BTreeMap::from([(
        "owner".into(),
        RelationDefinition::ToOne {
            table: "orders".into(),
            foreign_key: "order_id".into(),
            optional: true,
        },
    )]);
    three.tables.get_mut("order_notes").unwrap().relations = note_relations.clone();
    let second = Migration {
        from_version: 2,
        to: three.clone(),
        steps: vec![
            MigrationStep::DropTable {
                table: "temporary".into(),
            },
            MigrationStep::DropColumn {
                table: "orders".into(),
                name: "price".into(),
            },
            MigrationStep::DropColumn {
                table: "orders".into(),
                name: "active".into(),
            },
            MigrationStep::Backfill {
                table: "orders".into(),
                column: "notes".into(),
                value: Value::String("migrated".into()),
            },
            MigrationStep::SetRelations {
                table: "order_notes".into(),
                relations: note_relations,
            },
        ],
    };
    (three, vec![second, first])
}

pub fn snapshot<S: SchemaStorage>(
    store: &S,
) -> (ModelCatalog, BTreeMap<String, Vec<Row>>, Option<Row>) {
    let catalog = run(store.catalog()).unwrap();
    let mut tables = BTreeMap::new();
    for name in &catalog.tables {
        tables.insert(name.clone(), run(store.scan(name)).unwrap());
    }
    let metadata = run(store.get(MODEL_TABLE, "state")).unwrap();
    (catalog, tables, metadata)
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn d09_complete_authored_chain_applies_all_primitives() {
    let mut db = run(Database::open(fresh(), schema(1), vec![])).unwrap();
    let mut original = Vec::new();
    for total in [500, 1000, 4200] {
        let mut input = order(total);
        input.insert(
            "price".into(),
            Value::Float(FiniteFloat::new(1.25).unwrap()),
        );
        input.insert("active".into(), Value::Boolean(false));
        original.push(run(db.insert("orders", input)).unwrap());
    }
    let store = db.into_storage();
    let (target, migrations) = chain();
    let db = run(Database::open(store, target.clone(), migrations)).unwrap();
    let expected = original
        .into_iter()
        .map(|mut row| {
            let total = row.fields.remove("total_cents").unwrap();
            row.fields.insert("gross_cents".into(), total);
            row.fields.insert("discount".into(), Value::Integer(7));
            row.fields
                .insert("notes".into(), Value::String("migrated".into()));
            row.fields.remove("price");
            row.fields.remove("active");
            row
        })
        .collect::<Vec<_>>();
    assert_eq!(run(db.query("orders").fetch(&db)), Ok(expected));
    assert_eq!(db.schema().canonical(), target.canonical());
    let store = db.into_storage();
    let after = snapshot(&store);
    assert_eq!(after.0.state.unwrap().next_id, 4);
    assert!(!after.0.tables.contains(&"temporary".into()));
    for rows in after.1.values() {
        for row in rows {
            assert_eq!(row.fields.get(ROW_VERSION), Some(&Value::Integer(3)));
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn d10_failed_migration_and_endpoint_return_original_owned_engine() {
    for failure in 0..5 {
        let mut db = run(Database::open(fresh(), schema(1), vec![])).unwrap();
        run(db.insert("orders", order(4200))).unwrap();
        let store = db.into_storage();
        let original = snapshot(&store);
        let (target, mut migrations) = chain();
        match failure {
            0 => {
                migrations[0].steps = vec![MigrationStep::Backfill {
                    table: "orders".into(),
                    column: "discount".into(),
                    value: Value::String("wrong".into()),
                }]
            }
            1 => {
                migrations[0].steps = vec![MigrationStep::DropColumn {
                    table: "orders".into(),
                    name: "missing".into(),
                }]
            }
            2 => migrations[0].steps.clear(),
            3 => migrations.push(migrations[1].clone()),
            _ => migrations[0].from_version = 0,
        }
        let (error, store) = match run(Database::open_recoverable(store, target, migrations)) {
            Err(value) => value,
            Ok(_) => panic!("invalid authored migration succeeded"),
        };
        assert!(
            matches!(error, StorageError::MigrationFailure { .. }),
            "{error:?}"
        );
        assert_eq!(snapshot(&store), original);
        assert!(run(Database::open(store, schema(1), vec![])).is_ok());
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn d11_missing_complete_chain_resets_without_running_prefix() {
    let mut db = run(Database::open(fresh(), schema(1), vec![])).unwrap();
    for total in [500, 1000, 4200] {
        run(db.insert("orders", order(total))).unwrap();
    }
    let store = db.into_storage();
    let (_, mut migrations) = chain();
    migrations.remove(0);
    let mut db = run(Database::open(store, schema(4), migrations)).unwrap();
    assert_eq!(run(db.query("orders").fetch(&db)), Ok(vec![]));
    assert_eq!(db.schema().version, 4);
    assert_eq!(run(db.insert("orders", order(9))).unwrap().id, "local:4");
    let store = db.into_storage();
    let catalog = run(store.catalog()).unwrap();
    assert_eq!(catalog.state.unwrap().version, 4);
    assert!(!catalog.tables.contains(&"temporary".into()));
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn d12_downgrade_unknown_queue_and_future_metadata_reject() {
    let mut db = run(Database::open(fresh(), schema(3), vec![])).unwrap();
    run(db.insert("orders", order(7))).unwrap();
    let store = db.into_storage();
    let original = snapshot(&store);
    let (error, mut store) = match run(Database::open_recoverable(store, schema(2), vec![])) {
        Err(value) => value,
        Ok(_) => panic!("downgrade opened"),
    };
    assert!(matches!(error, StorageError::SchemaConflict { .. }));
    assert_eq!(snapshot(&store), original);
    run(store.create_table("__hoya_queue")).unwrap();
    let queue = Row {
        id: "pending".into(),
        fields: fields([("payload", Value::String("preserve".into()))]),
    };
    run(store.insert("__hoya_queue", queue.clone())).unwrap();
    let (error, store) = match run(Database::open_recoverable(store, schema(4), vec![])) {
        Err(value) => value,
        Ok(_) => panic!("unknown bookkeeping was reset"),
    };
    assert!(matches!(error, StorageError::BackendFailure { .. }));
    assert_eq!(run(store.get("__hoya_queue", "pending")), Ok(Some(queue)));
    assert_eq!(run(store.scan("orders")).unwrap(), original.1["orders"]);
    assert_eq!(run(store.get(MODEL_TABLE, "state")).unwrap(), original.2);
    drop(store);
    for (name, value) in [
        ("format", Value::Integer(2)),
        ("definition", Value::String("corrupt".into())),
        ("version", Value::Integer(9)),
        ("next_id", Value::Integer(0)),
    ] {
        let mut db = run(Database::open(fresh(), schema(1), vec![])).unwrap();
        run(db.insert("orders", order(7))).unwrap();
        let mut store = db.into_storage();
        let original = run(store.scan("orders")).unwrap();
        let mut metadata = run(store.get(MODEL_TABLE, "state")).unwrap().unwrap();
        metadata.fields.insert(name.into(), value);
        run(store.replace(MODEL_TABLE, metadata.clone())).unwrap();
        let (error, store) = match run(Database::open_recoverable(store, schema(4), vec![])) {
            Err(value) => value,
            Ok(_) => panic!("corrupt metadata was reset"),
        };
        assert!(matches!(error, StorageError::BackendFailure { .. }));
        assert_eq!(run(store.scan("orders")), Ok(original));
        assert_eq!(run(store.get(MODEL_TABLE, "state")), Ok(Some(metadata)));
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn d13_definition_codec_and_bound_unicode_names_roundtrip() {
    let mut definition = schema(1);
    definition
        .tables
        .get_mut("orders")
        .unwrap()
        .columns
        .insert("引用\0:5🌿".into(), column(ColumnType::String, true));
    let encoded = definition.canonical().unwrap();
    assert_eq!(
        SchemaDefinition::from_canonical(&encoded)
            .unwrap()
            .canonical(),
        Ok(encoded.clone())
    );
    let mut db = run(Database::open(fresh(), definition.clone(), vec![])).unwrap();
    let mut input = order(7);
    input.insert(
        "引用\0:5🌿".into(),
        Value::String("'; DROP TABLE x;--\0".into()),
    );
    let created = run(db.insert("orders", input)).unwrap();
    let store = db.into_storage();
    let db = run(Database::open(store, definition, vec![])).unwrap();
    assert_eq!(
        run(db
            .query("orders")
            .where_(
                "引用\0:5🌿",
                Operator::Eq,
                Value::String("'; DROP TABLE x;--\0".into())
            )
            .fetch(&db)),
        Ok(vec![created])
    );
    for bad in [
        format!("{encoded}0:"),
        "99999999999999999999999999999999999:x".into(),
        encoded[..encoded.len() - 1].into(),
    ] {
        assert!(SchemaDefinition::from_canonical(&bad).is_err());
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn d14_public_boundary_partitions_preserve_managed_state() {
    for failure in 0..4 {
        let mut bad = schema(1);
        match failure {
            0 => {
                bad.tables.get_mut("orders").unwrap().relations.insert(
                    "products".into(),
                    RelationDefinition::ManyToMany {
                        table: "products".into(),
                        through: "order_products".into(),
                        source_relation: "missing".into(),
                        target_relation: "product".into(),
                    },
                );
            }
            1 => {
                bad.tables
                    .get_mut("orders")
                    .unwrap()
                    .columns
                    .insert("customer_id".into(), column(ColumnType::String, true));
            }
            2 => {
                bad.tables.get_mut("orders").unwrap().relations.insert(
                    "customer".into(),
                    RelationDefinition::ToOne {
                        table: "missing".into(),
                        foreign_key: "customer_id".into(),
                        optional: true,
                    },
                );
            }
            _ => {
                bad.tables.get_mut("orders").unwrap().relations.insert(
                    "buyer".into(),
                    RelationDefinition::ToOne {
                        table: "products".into(),
                        foreign_key: "customer_id".into(),
                        optional: true,
                    },
                );
            }
        }
        assert!(matches!(
            bad.validate(),
            Err(StorageError::InvalidSchema { .. })
        ));
    }
    {
        let mut db = run(Database::open(fresh(), schema(1), vec![])).unwrap();
        let order_row = run(db.insert("orders", order(7))).unwrap();
        for value in [
            None,
            Some(Value::Null),
            Some(Value::Reference("dangling".into())),
        ] {
            let mut fields = order(7);
            if let Some(value) = value {
                fields.insert("customer_id".into(), value);
            }
            run(db.replace("orders", &order_row.id, fields)).unwrap();
            assert_eq!(
                run(db.related("orders", &order_row.id, "customer").fetch(&db)),
                Ok(vec![])
            );
        }
        assert!(matches!(
            run(db
                .query("orders")
                .order_by(ROW_VERSION, Direction::Asc)
                .limit(0)
                .fetch(&db)),
            Err(StorageError::InvalidQuery { .. })
        ));
        assert!(matches!(
            run(db
                .related("orders", "missing", "customer")
                .where_("unknown", Operator::Eq, Value::String("bad".into()))
                .limit(0)
                .fetch(&db)),
            Err(StorageError::InvalidQuery { .. })
        ));
    }
    {
        let mut definition = schema(1);
        definition
            .tables
            .get_mut("order_products")
            .unwrap()
            .columns
            .insert("label".into(), column(ColumnType::String, true));
        let mut db = run(Database::open(fresh(), definition, vec![])).unwrap();
        let order_row = run(db.insert("orders", order(7))).unwrap();
        let product =
            run(db.insert("products", fields([("name", Value::String("A".into()))]))).unwrap();
        let payload = fields([("label", Value::String("tag".into()))]);
        let linked = run(db.link(
            "orders",
            &order_row.id,
            "products",
            &product.id,
            payload.clone(),
        ))
        .unwrap();
        assert_eq!(
            run(db.link("orders", &order_row.id, "products", &product.id, payload)),
            Ok(linked)
        );
        assert!(matches!(
            run(db.link(
                "orders",
                &order_row.id,
                "products",
                &product.id,
                fields([("label", Value::String("other".into()))])
            )),
            Err(StorageError::InvalidRow { .. })
        ));
        assert!(matches!(
            run(db.link(
                "orders",
                &order_row.id,
                "products",
                &product.id,
                fields([("order_id", Value::Reference(order_row.id.clone()))])
            )),
            Err(StorageError::InvalidRow { .. })
        ));
        assert!(
            matches!(run(db.link("orders",&order_row.id,"products","missing",BTreeMap::new())),Err(StorageError::RowNotFound{table,id}) if table=="products"&&id=="missing")
        );
        run(db.insert(
            "order_products",
            fields([
                ("order_id", Value::Reference(order_row.id.clone())),
                ("product_id", Value::Reference("dangling".into())),
            ]),
        ))
        .unwrap();
        assert_eq!(
            run(db.related("orders", &order_row.id, "products").fetch(&db)),
            Ok(vec![product])
        );
    }
    {
        let db = run(Database::open(fresh(), schema(1), vec![])).unwrap();
        let mut store = db.into_storage();
        let mut metadata = run(store.get(MODEL_TABLE, "state")).unwrap().unwrap();
        metadata
            .fields
            .insert("next_id".into(), Value::Integer(i64::MAX));
        run(store.replace(MODEL_TABLE, metadata)).unwrap();
        let mut db = run(Database::open(store, schema(1), vec![])).unwrap();
        assert!(matches!(
            run(db.insert("orders", order(7))),
            Err(StorageError::SchemaConflict { .. })
        ));
        let store = db.into_storage();
        assert_eq!(
            run(store.catalog()).unwrap().state.unwrap().next_id,
            i64::MAX
        );
        assert_eq!(run(store.scan("orders")), Ok(vec![]));
    }
    for malformed in [false, true] {
        let mut store = fresh();
        run(store.create_table("orders")).unwrap();
        let mut fields = order(7);
        if malformed {
            fields.insert("total_cents".into(), Value::String("wrong".into()));
        } else {
            run(store.create_table("unexpected")).unwrap();
        }
        let row = Row {
            id: "legacy".into(),
            fields,
        };
        run(store.insert("orders", row.clone())).unwrap();
        let (_, store) = match run(Database::open_recoverable(store, schema(1), vec![])) {
            Err(value) => value,
            Ok(_) => panic!("incompatible legacy data adopted"),
        };
        assert_eq!(run(store.scan("orders")), Ok(vec![row]));
        assert_eq!(run(store.catalog()).unwrap().state, None);
    }
    {
        let db = run(Database::open(fresh(), schema(1), vec![])).unwrap();
        let mut store = db.into_storage();
        let original = run(store.catalog()).unwrap();
        for change in 0..2 {
            let mut target = schema(1);
            if change == 0 {
                target.tables.remove("order_notes");
            } else {
                target.tables.get_mut("orders").unwrap().relations.insert(
                    "customer".into(),
                    RelationDefinition::ToOne {
                        table: "customers".into(),
                        foreign_key: "customer_id".into(),
                        optional: false,
                    },
                );
            }
            let (error, recovered) = match run(Database::open_recoverable(store, target, vec![])) {
                Err(value) => value,
                Ok(_) => panic!("same-version drift opened"),
            };
            assert!(matches!(error, StorageError::SchemaConflict { .. }));
            store = recovered;
            assert_eq!(run(store.catalog()).unwrap(), original);
        }
        let overshoot = Migration {
            from_version: 1,
            to: schema(3),
            steps: vec![],
        };
        let (error, store) = match run(Database::open_recoverable(
            store,
            schema(2),
            vec![overshoot],
        )) {
            Err(value) => value,
            Ok(_) => panic!("overshooting path reset"),
        };
        assert!(matches!(error, StorageError::MigrationFailure { .. }));
        assert_eq!(run(store.catalog()).unwrap(), original);
    }
    let duplicate = [
        "hoyadb-model-1",
        "1",
        "1",
        "items",
        "2",
        "value",
        "String",
        "1",
        "value",
        "String",
        "1",
        "0",
    ]
    .into_iter()
    .map(|token| format!("{}:{token}", token.len()))
    .collect::<String>();
    assert!(SchemaDefinition::from_canonical(&duplicate).is_err());
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn tc0028_unselected_migration_descriptors_validate_nested_names() {
    for failure in 0..3 {
        let db = run(Database::open(fresh(), schema(1), vec![])).unwrap();
        let store = db.into_storage();
        let original = snapshot(&store);
        let step = match failure {
            0 => MigrationStep::AddTable {
                table: "extra".into(),
                definition: TableDefinition {
                    columns: BTreeMap::from([(
                        "__hoya_shadow".into(),
                        column(ColumnType::String, true),
                    )]),
                    relations: BTreeMap::new(),
                },
            },
            1 => MigrationStep::SetRelations {
                table: "orders".into(),
                relations: BTreeMap::from([(
                    "__hoya_shadow".into(),
                    RelationDefinition::ToOne {
                        table: "customers".into(),
                        foreign_key: "another_key".into(),
                        optional: true,
                    },
                )]),
            },
            _ => MigrationStep::SetRelations {
                table: "orders".into(),
                relations: BTreeMap::from([(
                    "another".into(),
                    RelationDefinition::ToOne {
                        table: "customers".into(),
                        foreign_key: "__hoya_schema_version".into(),
                        optional: true,
                    },
                )]),
            },
        };
        let migration = Migration {
            from_version: 1,
            to: schema(2),
            steps: vec![step],
        };
        let (error, store) = match run(Database::open_recoverable(
            store,
            schema(1),
            vec![migration],
        )) {
            Err(value) => value,
            Ok(_) => panic!("invalid unselected descriptor opened"),
        };
        assert!(matches!(error, StorageError::MigrationFailure { .. }));
        assert_eq!(snapshot(&store), original);
    }
}

/// Review regression: a ToOne declared by SetRelations names its implicit
/// Reference key at once, so a later Backfill in the same migration reaches it.
#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn review_m01_backfill_reaches_implicit_key_of_same_migration_to_one() {
    let relations = BTreeMap::from([(
        "order".to_owned(),
        RelationDefinition::ToOne {
            table: "orders".into(),
            foreign_key: "order_id".into(),
            optional: false,
        },
    )]);
    let mut target = schema(2);
    target.tables.get_mut("order_notes").unwrap().relations = relations.clone();
    for value in [
        Some(Value::Reference("local:1".into())),
        Some(Value::String("local:1".into())),
        None,
    ] {
        let mut db = run(Database::open(fresh(), schema(1), vec![])).unwrap();
        let order_row = run(db.insert("orders", order(7))).unwrap();
        assert_eq!(order_row.id, "local:1");
        let note = run(db.insert(
            "order_notes",
            fields([("body", Value::String("fragile".into()))]),
        ))
        .unwrap();
        let store = db.into_storage();
        let original = snapshot(&store);
        let mut steps = vec![MigrationStep::SetRelations {
            table: "order_notes".into(),
            relations: relations.clone(),
        }];
        steps.extend(value.clone().map(|value| MigrationStep::Backfill {
            table: "order_notes".into(),
            column: "order_id".into(),
            value,
        }));
        let migration = Migration {
            from_version: 1,
            to: target.clone(),
            steps,
        };
        let opened = run(Database::open_recoverable(
            store,
            target.clone(),
            vec![migration],
        ));
        if value != Some(Value::Reference("local:1".into())) {
            // A wrong-kind value and a required key left unset still fail whole.
            let (error, store) = match opened {
                Err(value) => value,
                Ok(_) => panic!("invalid required key migration opened"),
            };
            assert!(
                matches!(error, StorageError::MigrationFailure { .. }),
                "{error:?}"
            );
            assert_eq!(snapshot(&store), original);
            continue;
        }
        let db = match opened {
            Ok(db) => db,
            Err((error, _)) => panic!("backfill of the implicit key failed: {error:?}"),
        };
        let mut expected = note.clone();
        expected
            .fields
            .insert("order_id".into(), Value::Reference("local:1".into()));
        assert_eq!(run(db.query("order_notes").fetch(&db)), Ok(vec![expected]));
        assert_eq!(
            run(db.related("order_notes", &note.id, "order").fetch(&db)),
            Ok(vec![order_row])
        );
        assert_eq!(db.schema().canonical(), target.canonical());
    }
}
