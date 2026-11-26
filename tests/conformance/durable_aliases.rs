//! SPEC-0049 / TC-0053: external IDs must outlive bounded recovery history.
use hoyadb_core::{command::*, model::*, storage::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    task::{Context, Poll, Waker},
};

pub fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("memory fixture unexpectedly suspended"),
    }
}

pub fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([(
            "notes".into(),
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
        )]),
    }
}

pub fn config() -> EntityConfig {
    EntityConfig {
        scope: "durable-alias-test".into(),
        tables: BTreeSet::from(["notes".into()]),
    }
}

pub fn provider() -> Result<EntityContext, CommandError> {
    Ok(EntityContext {
        identity: Json::Null,
        now_ms: 1000,
    })
}

pub fn open<S: SyncStorage>(store: S, history: usize) -> CommandDatabase<S> {
    run(CommandDatabase::open(
        store,
        schema(),
        vec![],
        [0x53; 16],
        SyncOptions {
            history,
            ..Default::default()
        },
    ))
    .unwrap()
}

pub fn configure_durable<S: SyncStorage>(db: &mut CommandDatabase<S>) {
    db.configure_entities_durable(config(), provider).unwrap();
}

pub fn accepted_create<S: SyncStorage>(db: &mut CommandDatabase<S>) -> (String, String) {
    accepted_create_as(db, "server-note")
}

pub fn accepted_create_as<S: SyncStorage>(
    db: &mut CommandDatabase<S>,
    remote: &str,
) -> (String, String) {
    let row = run(db.insert(
        "notes",
        BTreeMap::from([("body".into(), Value::String("kept".into()))]),
    ))
    .unwrap();
    let batch = db.prepare_push(1000, 0).unwrap();
    assert_eq!(batch.envelopes.len(), 1);
    let Json::Object(envelope) = Json::parse(&batch.envelopes[0]).unwrap() else {
        panic!("entity envelope")
    };
    let Json::String(key) = &envelope["writeId"] else {
        panic!("write id")
    };
    db.receive_entity(
        "notes",
        &row.id,
        key,
        Some(RemoteOutcome::Applied(Json::Null)),
    )
    .unwrap();
    run(db.reconcile(
        key,
        ReconcilePlan {
            mappings: BTreeMap::from([(row.id.clone(), remote.into())]),
            writes: vec![],
        },
    ))
    .unwrap();
    (row.id, key.clone())
}

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
struct Fault<S>(S, Arc<AtomicBool>, StoreOwnership);
impl<S> Fault<S> {
    fn check_failure(&self) -> Result<(), StorageError> {
        if self.1.swap(false, Ordering::SeqCst) {
            Err(StorageError::BackendFailure {
                message: "task-owned injected commit failure".into(),
            })
        } else {
            Ok(())
        }
    }
}
impl<S: SyncStorage> Storage for Fault<S> {
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

impl<S: SyncStorage> TransactionalStorage for Fault<S> {
    fn apply_batch(
        &mut self,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.0.apply_batch(mutations)
    }
}

impl<S: SyncStorage> TypedStorage for Fault<S> {
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

impl<S: SyncStorage> TypedTransactionalStorage for Fault<S> {
    fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send {
        self.0.apply_typed_batch(schema, mutations)
    }
}

impl<S: SyncStorage> SchemaStorage for Fault<S> {
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

impl<S: SyncStorage> ObservationStorage for Fault<S> {
    fn project(
        &self,
        schema: &Schema,
        query: &Query,
        fields: &[String],
    ) -> impl Future<Output = Result<Vec<ProjectedRow>, StorageError>> + Send {
        self.0.project(schema, query, fields)
    }
}

impl<S: SyncStorage> SyncStorage for Fault<S> {
    fn ownership(&self) -> StoreOwnership {
        self.2
    }
    fn reads_ready(&self) -> bool {
        self.0.reads_ready()
    }
    fn apply_sync_delta(&mut self, delta: SyncDelta) -> Result<Vec<CommittedChange>, StorageError> {
        self.check_failure()?;
        self.0.apply_sync_delta(delta)
    }

    fn load_sync(&self) -> Result<Option<Vec<Row>>, StorageError> {
        self.0.load_sync()
    }

    fn apply_sync_plan(&mut self, plan: SyncPlan) -> Result<Vec<CommittedChange>, StorageError> {
        self.check_failure()?;
        self.0.apply_sync_plan(plan)
    }
}

/// Same source mutation must roll back aliases, rows, recovery and indexes on either commit path.
pub fn failed_reconcile_is_atomic<S: SyncStorage>(store: S, ownership: StoreOwnership) {
    let fail = Arc::new(AtomicBool::new(false));
    let mut db = open(Fault(store, fail.clone(), ownership), 0);
    configure_durable(&mut db);
    let row = run(db.insert(
        "notes",
        BTreeMap::from([("body".into(), Value::String("kept".into()))]),
    ))
    .unwrap();
    let batch = db.prepare_push(1000, 0).unwrap();
    let Json::Object(envelope) = Json::parse(&batch.envelopes[0]).unwrap() else {
        panic!("envelope")
    };
    let Json::String(key) = &envelope["writeId"] else {
        panic!("write id")
    };
    db.receive_entity(
        "notes",
        &row.id,
        key,
        Some(RemoteOutcome::Applied(Json::Null)),
    )
    .unwrap();
    let before = db.queue_status();
    fail.store(true, Ordering::SeqCst);
    let plan = || ReconcilePlan {
        mappings: BTreeMap::from([(row.id.clone(), "server-note".into())]),
        writes: vec![],
    };
    assert!(run(db.reconcile(key, plan())).is_err());
    assert_eq!(db.resolve_entity_id("notes", &row.id).unwrap(), row.id);
    assert_eq!(db.queue_status(), before);
    assert_eq!(
        run(db.query("notes").fetch(&db)).unwrap(),
        vec![row.clone()]
    );
    let store = db.into_storage();
    let packets = store.load_sync().unwrap().unwrap();
    assert!(
        !packets
            .iter()
            .any(|packet| packet.id.starts_with("entity:alias:"))
    );
    let mut db = open(store, 0);
    configure_durable(&mut db);
    run(db.reconcile(key, plan())).unwrap();
    assert_eq!(
        db.resolve_entity_id("notes", &row.id).unwrap(),
        "server-note"
    );
    assert!(db.queue_status().commands.is_empty());
    let mut db = open(db.into_storage(), 0);
    configure_durable(&mut db);
    assert_eq!(
        db.resolve_entity_id("notes", &row.id).unwrap(),
        "server-note"
    );
}

/// SPEC-0049: reopening an opted-in store through ordinary configuration must
/// report the retained durable mode instead of silently accepting it.
pub fn ordinary_configuration_rejects_retained_durable_mode<S: SyncStorage>(store: S) {
    let mut db = open(store, 100);
    configure_durable(&mut db);
    let (original, _) = accepted_create(&mut db);
    let mut reopened = open(db.into_storage(), 100);
    assert_eq!(
        reopened.configure_entities(config(), provider),
        Err(CommandError::InvalidCommand),
        "ordinary configuration must not adopt a retained format-4 store"
    );
    // The refused call installs no provider and leaves the durable mode usable.
    assert_eq!(
        run(reopened.insert(
            "notes",
            BTreeMap::from([("body".into(), Value::String("unconfigured".into()))]),
        )),
        Err(CommandError::InvalidCommand)
    );
    configure_durable(&mut reopened);
    assert_eq!(
        reopened.resolve_entity_id("notes", &original).unwrap(),
        "server-note"
    );
}

/// SPEC-0049: a declared to-one cell resolves through its target table. An
/// accepted canonical ID there is not another table's alias source.
pub fn reference_cells_resolve_through_their_target_table<S: SyncStorage>(store: S) {
    let mut definition = schema();
    let notes = definition.tables["notes"].clone();
    definition.tables.insert("other".into(), notes.clone());
    let mut links = notes;
    links.relations.insert(
        "note".into(),
        RelationDefinition::ToOne {
            table: "notes".into(),
            foreign_key: "note_id".into(),
            optional: false,
        },
    );
    definition.tables.insert("links".into(), links);
    let mut db = run(CommandDatabase::open(
        store,
        definition,
        vec![],
        [0x53; 16],
        SyncOptions {
            history: 0,
            ..Default::default()
        },
    ))
    .unwrap();
    let mut configured = config();
    configured.tables.insert("other".into());
    configured.tables.insert("links".into());
    db.configure_entities_durable(configured, provider).unwrap();
    // notes: generated 1 -> canonical literal 2; other: generated 2 -> server-other.
    let canonical_note = format!("entity:{}:0000000000000002", "53".repeat(16));
    let (original_note, _) = accepted_create_as(&mut db, &canonical_note);
    let other = run(db.insert(
        "other",
        BTreeMap::from([("body".into(), Value::String("other".into()))]),
    ))
    .unwrap();
    assert_eq!(other.id, canonical_note);
    let key = db.queue_status().commands[0].key.clone();
    db.prepare_push(1000, 0).unwrap();
    db.receive_entity(
        "other",
        &other.id,
        &key,
        Some(RemoteOutcome::Applied(Json::Null)),
    )
    .unwrap();
    run(db.reconcile(
        &key,
        ReconcilePlan {
            mappings: BTreeMap::from([(other.id.clone(), "server-other".into())]),
            writes: vec![],
        },
    ))
    .unwrap();
    let link = |target: &str| {
        BTreeMap::from([
            ("body".into(), Value::String("link".into())),
            ("note_id".into(), Value::Reference(target.into())),
        ])
    };
    assert_eq!(
        db.normalize_entity_fields("links", link(&canonical_note))
            .unwrap(),
        link(&canonical_note),
        "the notes canonical ID must not follow the other table's alias"
    );
    assert_eq!(
        db.normalize_entity_fields("links", link(&original_note))
            .unwrap(),
        link(&canonical_note),
        "a stale notes ID still resolves through the notes alias"
    );
    let row = run(db.insert("links", link(&canonical_note))).unwrap();
    assert_eq!(
        row.fields["note_id"],
        Value::Reference(canonical_note.clone())
    );
    let patched = run(db.patch(
        "links",
        &row.id,
        BTreeMap::from([("note_id".into(), Value::Reference(original_note))]),
    ))
    .unwrap();
    assert_eq!(patched.fields["note_id"], Value::Reference(canonical_note));
}

/// API-0009: adoption refuses a configured table that already holds a reserved
/// `local:` row, because the entity path would reject every write addressing it.
pub fn adoption_refuses_reserved_local_rows<S: SyncStorage>(store: S) {
    let body = |value: &str| BTreeMap::from([("body".into(), Value::String(value.into()))]);
    let mut db = open(store, 100);
    let seeded = run(db.insert("notes", body("seeded"))).unwrap();
    assert_eq!(seeded.id, "local:1");
    assert_eq!(
        db.configure_entities(config(), provider),
        Err(CommandError::Protocol)
    );
    assert_eq!(
        db.configure_entities_durable(config(), provider),
        Err(CommandError::Protocol)
    );
    assert_eq!(db.queue_status().commands.len(), 0);
    // Nothing was adopted or persisted, and the store remains ordinary after reopen.
    let store = db.into_storage();
    assert!(store.load_sync().unwrap().is_none());
    let mut db = open(store, 100);
    assert_eq!(
        run(db.query("notes").fetch(&db)).unwrap(),
        vec![seeded.clone()]
    );
    run(db.replace("notes", &seeded.id, body("still ordinary"))).unwrap();
    assert_eq!(
        db.configure_entities_durable(config(), provider),
        Err(CommandError::Protocol)
    );
    // A host-proposed ordinary ID is a seeded legacy row and stays adoptable.
    assert!(run(db.remove("notes", &seeded.id)).unwrap());
    let legacy =
        run(db.insert_proposed("notes", body("legacy"), Some("legacy-note".into()))).unwrap();
    configure_durable(&mut db);
    let patched = run(db.patch("notes", &legacy.id, body("adopted"))).unwrap();
    assert_eq!(patched.id, "legacy-note");
    assert_eq!(db.queue_status().pending, 1);
}
