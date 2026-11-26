//! Validation-only one-run debug measurement; delegates all algorithms to production adapters.
use hoyadb_core::{command::*, model::*, storage::*};
use hoyadb_storage_memory::MemoryStorage;
use hoyadb_storage_sqlite::SqliteStorage;
use std::{
    collections::{BTreeMap, BTreeSet},
    future::{Future, ready},
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
    time::Instant,
};

fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("production adapter unexpectedly pending"),
    }
}
#[derive(Clone, Debug)]
struct Commit {
    packets: usize,
    data_bytes: usize,
    application_mutations: usize,
}
struct Shared<S> {
    store: Arc<Mutex<S>>,
    commits: Arc<Mutex<Vec<Commit>>>,
}
impl<S> Clone for Shared<S> {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            commits: self.commits.clone(),
        }
    }
}
impl<S> Shared<S> {
    fn new(store: S) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
            commits: Arc::new(Mutex::new(vec![])),
        }
    }
}
impl<S: SyncStorage + Send> Storage for Shared<S> {
    fn create_table(
        &mut self,
        name: &str,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(run(self.store.lock().unwrap().create_table(name)))
    }
    fn insert(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(run(self.store.lock().unwrap().insert(table, row)))
    }
    fn get(
        &self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<Option<Row>, StorageError>> + Send {
        ready(run(self.store.lock().unwrap().get(table, id)))
    }
    fn replace(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(run(self.store.lock().unwrap().replace(table, row)))
    }
    fn remove(
        &mut self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send {
        ready(run(self.store.lock().unwrap().remove(table, id)))
    }
}
impl<S: SyncStorage + Send> TransactionalStorage for Shared<S> {
    fn apply_batch(
        &mut self,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(run(self.store.lock().unwrap().apply_batch(mutations)))
    }
}
impl<S: SyncStorage + Send> TypedStorage for Shared<S> {
    fn query(
        &self,
        schema: &Schema,
        query: &Query,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        ready(run(self.store.lock().unwrap().query(schema, query)))
    }
}
impl<S: SyncStorage + Send> TypedTransactionalStorage for Shared<S> {
    fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send {
        ready(run(self
            .store
            .lock()
            .unwrap()
            .apply_typed_batch(schema, mutations)))
    }
}
impl<S: SyncStorage + Send> SchemaStorage for Shared<S> {
    fn catalog(&self) -> impl Future<Output = Result<ModelCatalog, StorageError>> + Send {
        ready(run(self.store.lock().unwrap().catalog()))
    }
    fn scan(&self, table: &str) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        ready(run(self.store.lock().unwrap().scan(table)))
    }
    fn apply_schema_plan(
        &mut self,
        plan: SchemaPlan,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(run(self.store.lock().unwrap().apply_schema_plan(plan)))
    }
}
impl<S: SyncStorage + Send> ObservationStorage for Shared<S> {
    fn project(
        &self,
        schema: &Schema,
        query: &Query,
        fields: &[String],
    ) -> impl Future<Output = Result<Vec<ProjectedRow>, StorageError>> + Send {
        ready(run(self
            .store
            .lock()
            .unwrap()
            .project(schema, query, fields)))
    }
}
impl<S: SyncStorage + Send> SyncStorage for Shared<S> {
    fn load_sync(&self) -> Result<Option<Vec<Row>>, StorageError> {
        self.store.lock().unwrap().load_sync()
    }
    fn apply_sync_plan(&mut self, plan: SyncPlan) -> Result<Vec<CommittedChange>, StorageError> {
        let count = Commit {
            packets: plan.next_sync.len(),
            data_bytes: data_bytes(&plan.next_sync),
            application_mutations: plan.mutations.len(),
        };
        let result = self.store.lock().unwrap().apply_sync_plan(plan);
        if result.is_ok() {
            self.commits.lock().unwrap().push(count);
        }
        result
    }
}
fn text(value: impl ToString) -> Json {
    Json::String(value.to_string())
}
fn object<const N: usize>(pairs: [(&str, Json); N]) -> Json {
    Json::Object(pairs.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
fn number(value: usize) -> Json {
    Json::Integer(i64::try_from(value).unwrap())
}
fn data(row: &Row) -> &str {
    let Value::String(value) = &row.fields["data"] else {
        panic!("sync data cell is not decoded String")
    };
    value
}
fn data_bytes(rows: &[Row]) -> usize {
    rows.iter().map(|r| data(r).len()).sum()
}
fn packet_bytes(rows: &[Row]) -> usize {
    rows.iter()
        .map(|row| {
            let Value::Integer(format) = row.fields["format"] else {
                panic!("format")
            };
            object([
                ("id", text(&row.id)),
                ("format", Json::Integer(format)),
                ("data", text(data(row))),
            ])
            .canonical()
            .unwrap()
            .len()
        })
        .sum()
}
fn metadata(rows: &[Row]) -> BTreeMap<String, String> {
    rows.iter()
        .filter(|r| r.id.starts_with("entity:metadata:"))
        .map(|r| (r.id.clone(), data(r).into()))
        .collect()
}
fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([(
            "items".into(),
            TableDefinition {
                columns: BTreeMap::from([
                    (
                        "label".into(),
                        Column {
                            kind: ColumnType::String,
                            optional: false,
                        },
                    ),
                    (
                        "rank".into(),
                        Column {
                            kind: ColumnType::Integer,
                            optional: false,
                        },
                    ),
                ]),
                ..Default::default()
            },
        )]),
    }
}
fn probe<S: SyncStorage + Send>(adapter: &str, size: usize, store: S) {
    let shared = Shared::new(store);
    let inspect = shared.clone();
    let mut owner = run(CommandDatabase::open(
        shared,
        schema(),
        vec![],
        [0x66; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    owner
        .configure_entities(
            EntityConfig {
                scope: "footprint:synthetic".into(),
                tables: BTreeSet::from(["items".into()]),
            },
            || {
                Ok(EntityContext {
                    identity: Json::Null,
                    now_ms: 10000,
                })
            },
        )
        .unwrap();
    let changes = (0..size)
        .map(|i| EntityChange {
            table: "items".into(),
            record_id: format!("server:{i:08}"),
            op: EntityOperation::Create,
            changes: BTreeMap::from([
                ("label".into(), Value::String(format!("item-{i:08}"))),
                ("rank".into(), Value::Integer(i as i64)),
            ]),
            removed_fields: BTreeSet::new(),
            timestamp_ms: 10000,
            field_timestamps: BTreeMap::from([("label".into(), 10000), ("rank".into(), 10000)]),
        })
        .collect();
    let page = PullPage {
        checkpoint: format!("rows:{size}:v1"),
        complete: false,
        changes,
    };
    let page_bytes = page.to_json().unwrap().canonical().unwrap().len();
    let request = owner.begin_pull().unwrap().request;
    let started = Instant::now();
    run(owner.apply_pull(request, page)).unwrap();
    let insert_us = started.elapsed().as_micros();
    let first = inspect.load_sync().unwrap().unwrap();
    assert_eq!(first.len(), size + 2);
    let first_metadata = metadata(&first);
    assert_eq!(first_metadata.len(), size);
    let empty = PullPage {
        checkpoint: format!("rows:{size}:v2"),
        complete: true,
        changes: vec![],
    };
    let request = owner.begin_pull().unwrap().request;
    let started = Instant::now();
    run(owner.apply_pull(request, empty)).unwrap();
    let empty_us = started.elapsed().as_micros();
    let second = inspect.load_sync().unwrap().unwrap();
    assert_eq!(
        metadata(&second),
        first_metadata,
        "checkpoint-only commit changes no comparison packets"
    );
    assert_eq!(run(owner.fetch(&owner.query("items"))).unwrap().len(), size);
    assert_eq!(owner.sync_progress().pages, 2);
    assert_eq!(owner.queue_status().commands.len(), 0);
    let commits = inspect.commits.lock().unwrap();
    assert_eq!(commits.len(), 3);
    assert_eq!(commits[1].application_mutations, size);
    assert_eq!(commits[2].application_mutations, 0);
    assert_eq!(commits[2].packets, size + 2);
    assert_eq!(commits[2].data_bytes, data_bytes(&second));
    let report = object([
        ("adapter", text(adapter)),
        ("rows", number(size)),
        ("fields_per_row", number(2)),
        ("input_page_utf8_bytes", number(page_bytes)),
        ("metadata_packet_count", number(first_metadata.len())),
        (
            "metadata_data_json_utf8_bytes",
            number(first_metadata.values().map(String::len).sum()),
        ),
        ("all_packet_count", number(second.len())),
        ("all_data_json_utf8_bytes", number(data_bytes(&second))),
        (
            "all_packet_canonical_object_utf8_bytes",
            number(packet_bytes(&second)),
        ),
        (
            "insert_page_us",
            Json::Integer(i64::try_from(insert_us).unwrap()),
        ),
        (
            "checkpoint_only_page_us",
            Json::Integer(i64::try_from(empty_us).unwrap()),
        ),
        (
            "checkpoint_only_rewritten_packet_count",
            number(commits[2].packets),
        ),
        (
            "checkpoint_only_application_mutation_count",
            number(commits[2].application_mutations),
        ),
    ]);
    println!("{}", report.canonical().unwrap());
    drop(commits);
    drop(owner);
    drop(inspect);
}
fn main() {
    let root = std::env::args()
        .nth(1)
        .expect("explicit task-owned store directory");
    let root = std::path::PathBuf::from(root);
    std::fs::create_dir(&root).unwrap();
    let load = std::process::Command::new("sysctl")
        .args(["-n", "vm.loadavg"])
        .output()
        .unwrap();
    println!(
        "{}",
        object([
            ("kind", text("environment")),
            ("os", text(std::env::consts::OS)),
            ("arch", text(std::env::consts::ARCH)),
            ("profile", text("debug")),
            (
                "parallelism",
                number(std::thread::available_parallelism().unwrap().get())
            ),
            (
                "loadavg_before",
                text(String::from_utf8(load.stdout).unwrap().trim())
            )
        ])
        .canonical()
        .unwrap()
    );
    for size in [10, 100, 1000] {
        probe("memory", size, MemoryStorage::default());
        probe(
            "sqlite",
            size,
            SqliteStorage::open(root.join(format!("store-{size}.sqlite"))).unwrap(),
        );
    }
    let load = std::process::Command::new("sysctl")
        .args(["-n", "vm.loadavg"])
        .output()
        .unwrap();
    println!(
        "{}",
        object([
            ("kind", text("environment_end")),
            (
                "loadavg_after",
                text(String::from_utf8(load.stdout).unwrap().trim())
            )
        ])
        .canonical()
        .unwrap()
    );
    std::fs::remove_dir_all(&root).unwrap();
}
