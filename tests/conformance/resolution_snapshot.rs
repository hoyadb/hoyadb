//! TC-0041 SI-Y08: real owner mapping rewrites against the conservative twin.
use hoyadb_core::{command::*, model::*, storage::*};
use std::{
    collections::BTreeMap,
    future::Future,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};
fn run<F: Future>(f: F) -> F::Output {
    match std::pin::pin!(f).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("synchronous fixture suspended"),
    }
}
type Captured = Arc<Mutex<Vec<Vec<RowGuard>>>>;
struct Trace<S>(S, Captured, StoreOwnership);
impl<S: SyncStorage> Storage for Trace<S> {
    fn create_table(
        &mut self,
        name: &str,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.0.create_table(name)
    }

    fn insert(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.0.insert(table, row)
    }

    fn get(
        &self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<Option<Row>, StorageError>> + Send {
        self.0.get(table, id)
    }

    fn replace(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.0.replace(table, row)
    }

    fn remove(
        &mut self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send {
        self.0.remove(table, id)
    }
}

impl<S: SyncStorage> TransactionalStorage for Trace<S> {
    fn apply_batch(
        &mut self,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.0.apply_batch(mutations)
    }
}

impl<S: SyncStorage> TypedStorage for Trace<S> {
    fn query_validated(
        &self,
        proof: ValidatedQuery<'_>,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.0.query_validated(proof)
    }

    fn query(
        &self,
        schema: &Schema,
        query: &Query,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.0.query(schema, query)
    }
}

impl<S: SyncStorage> TypedTransactionalStorage for Trace<S> {
    fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send {
        self.0.apply_typed_batch(schema, mutations)
    }
}

impl<S: SyncStorage> SchemaStorage for Trace<S> {
    fn catalog(&self) -> impl Future<Output = Result<ModelCatalog, StorageError>> + Send {
        self.0.catalog()
    }

    fn scan(&self, table: &str) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.0.scan(table)
    }

    fn apply_schema_plan(
        &mut self,
        plan: SchemaPlan,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.0.apply_schema_plan(plan)
    }
}

impl<S: SyncStorage> ObservationStorage for Trace<S> {
    fn project(
        &self,
        schema: &Schema,
        query: &Query,
        fields: &[String],
    ) -> impl Future<Output = Result<Vec<ProjectedRow>, StorageError>> + Send {
        self.0.project(schema, query, fields)
    }
}

impl<S: SyncStorage> SyncStorage for Trace<S> {
    fn ownership(&self) -> StoreOwnership {
        self.2
    }
    fn reads_ready(&self) -> bool {
        self.0.reads_ready()
    }
    fn apply_sync_delta(&mut self, delta: SyncDelta) -> Result<Vec<CommittedChange>, StorageError> {
        self.1.lock().unwrap().push(delta.guards().to_vec());
        self.0.apply_sync_delta(delta)
    }

    fn load_sync(&self) -> Result<Option<Vec<Row>>, StorageError> {
        self.0.load_sync()
    }

    fn apply_sync_plan(&mut self, plan: SyncPlan) -> Result<Vec<CommittedChange>, StorageError> {
        self.1.lock().unwrap().push(plan.guards.clone());
        self.0.apply_sync_plan(plan)
    }
}

fn fields(body: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([("body".into(), Value::String(body.into()))])
}
#[derive(Debug, PartialEq)]
pub struct Outcome {
    result: Result<(), CommandError>,
    parents: Vec<Row>,
    children: Vec<Row>,
    guards: Vec<RowGuard>,
    packets: Option<Vec<Row>>,
}
pub fn exercise<S: SyncStorage>(
    store: S,
    n: usize,
    ownership: StoreOwnership,
    occupied: bool,
) -> (Outcome, counters::WorkSnapshot) {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let schema = SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([
            (
                "parents".into(),
                TableDefinition {
                    columns: BTreeMap::from([(
                        "body".into(),
                        Column {
                            kind: ColumnType::String,
                            optional: false,
                        },
                    )]),
                    ..Default::default()
                },
            ),
            (
                "children".into(),
                TableDefinition {
                    relations: BTreeMap::from([(
                        "parent".into(),
                        RelationDefinition::ToOne {
                            table: "parents".into(),
                            foreign_key: "parent_id".into(),
                            optional: false,
                        },
                    )]),
                    ..Default::default()
                },
            ),
        ]),
    };
    let mut db = run(CommandDatabase::open(
        Trace(store, captured.clone(), ownership),
        schema,
        vec![],
        [0x78; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    for i in 0..n {
        run(db.insert_proposed(
            "parents",
            fields("unrelated"),
            Some(format!("server:filler:{i:04}")),
        ))
        .unwrap();
    }
    run(db.insert_proposed("parents", fields("audit"), Some("server:audit".into()))).unwrap();
    if occupied {
        run(db.insert_proposed(
            "parents",
            fields("occupied"),
            Some("server:accepted".into()),
        ))
        .unwrap();
    }
    let ticket = db.request().unwrap();
    assert!(
        run(db.begin(
            ticket,
            CommandDefinition {
                name: "create-v1".into(),
                arguments: BTreeMap::new(),
                reference_paths: vec![]
            },
            Json::Object(BTreeMap::new()),
            Some(Json::Null),
            1
        ))
        .unwrap()
    );
    let local = run(db.scope_insert_proposed(
        ticket,
        "parents",
        fields("new"),
        Some("local:parent".into()),
    ))
    .unwrap();
    let key = run(db.finish(ticket, Json::Null)).unwrap().key;
    run(db.insert_proposed(
        "children",
        BTreeMap::from([("parent_id".into(), Value::Reference(local.id.clone()))]),
        Some("server:child".into()),
    ))
    .unwrap();
    db.prepare_push(1, 0).unwrap();
    db.receive(&key, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    captured.lock().unwrap().clear();
    counters::reset();
    let result = run(db.reconcile_guarded(
        &key,
        ReconcilePlan {
            mappings: BTreeMap::from([(local.id.clone(), "server:accepted".into())]),
            writes: vec![ReconcileWrite::Put {
                table: "parents".into(),
                id: "server:audit".into(),
                fields: fields("checked"),
            }],
        },
        vec![CorrectionGuard {
            table: "parents".into(),
            id: "server:audit".into(),
            expected: Some(fields("audit")),
        }],
    ));
    let work = counters::snapshot();
    let guards = captured
        .lock()
        .unwrap()
        .iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    if occupied {
        assert_eq!(result, Err(CommandError::InvalidCommand));
        assert!(guards.is_empty(), "occupied target must fail before commit");
    } else {
        assert_eq!(result, Ok(()));
        assert!(
            guards.iter().any(|guard| guard.table == "children"
                && guard.id == "server:child"
                && guard
                    .expected
                    .as_ref()
                    .is_some_and(|row| row.fields.get("parent_id")
                        == Some(&Value::Reference("local:parent".into())))),
            "outside reference rewrite must have its exact prior guard"
        );
        assert!(
            guards.iter().any(|guard| guard.table == "parents"
                && guard.id == "server:accepted"
                && guard.expected.is_none()),
            "mapping destination must be guarded absent"
        );
    }
    let parents = run(db.fetch(&db.query("parents")))
        .unwrap()
        .into_iter()
        .filter(|row| !row.id.starts_with("server:filler:"))
        .collect();
    let children = run(db.fetch(&db.query("children"))).unwrap();
    assert_eq!(
        children[0].fields["parent_id"],
        Value::Reference(
            if occupied {
                "local:parent"
            } else {
                "server:accepted"
            }
            .into()
        )
    );
    let packets = db.into_storage().load_sync().unwrap();
    (
        Outcome {
            result,
            parents,
            children,
            guards,
            packets,
        },
        work,
    )
}
pub fn assert_bounded(small: &counters::WorkSnapshot, large: &counters::WorkSnapshot) {
    assert_eq!(small.rows_cloned, large.rows_cloned);
    assert_eq!(small.rows_validated, large.rows_validated);
    assert_eq!(small.memory_rows_visited, large.memory_rows_visited);
    assert_eq!(small.sqlite_rows_read, large.sqlite_rows_read);
    assert_eq!(small.sqlite_rows_written, large.sqlite_rows_written);
    assert_eq!(small.sqlite_fullscan_steps, 0);
    assert_eq!(large.sqlite_fullscan_steps, 0);
}
