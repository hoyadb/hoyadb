//! TC-0035 S02–S07: independent native encrypted-engine acceptance.
#![cfg(not(target_arch = "wasm32"))]
use hoyadb_core::command::*;
use hoyadb_core::storage::*;
use hoyadb_storage_sqlite::{EncryptionKey, SqliteOpenOptions, SqliteStorage};
use std::{
    collections::BTreeMap,
    future::Future,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Waker},
    time::{Duration, Instant},
};

const KEY: [u8; 32] = [0x5a; 32];
const CANARY: &str = "synthetic-secret-row-東京-🌿";
const IDENTITY: &str = "synthetic-secret-identity";
const CHECKPOINT: &str = "synthetic-secret-checkpoint";
const UPDATE: &str = "synthetic-secret-sidecar-update-🌿";
static OWNERS: Mutex<()> = Mutex::new(());

#[path = "../../../tests/conformance/command_owner.rs"]
mod commands;
#[path = "../../../tests/conformance/entity_sync.rs"]
mod entities;
#[path = "../../../tests/conformance/entity_pull.rs"]
mod pulls;
#[path = "../../../tests/conformance/command_sync.rs"]
mod sync_cases;

fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("native SQLite fixture unexpectedly suspended"),
    }
}

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "hoya-encrypted-engine-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("remove only owned encrypted fixture directory");
    }
}

fn protected(path: &Path) -> SqliteStorage {
    SqliteStorage::open_with_options(
        path,
        SqliteOpenOptions {
            encryption_key: Some(EncryptionKey::from_bytes(KEY)),
        },
    )
    .expect("supplied-key protected adapter must open the owned fixture")
}

fn keyed_sql(path: &Path) -> rusqlite::Connection {
    let sql = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_CREATE,
    )
    .unwrap();
    // Fixed synthetic key; no application/production secret is read or logged.
    sql.execute_batch(&format!("PRAGMA key = \"x'{}'\";", "5a".repeat(32)))
        .unwrap();
    sql
}

fn file_snapshot(directory: &Directory) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(&directory.0)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_file());
            (
                entry.file_name().into_string().unwrap(),
                std::fs::read(entry.path()).unwrap(),
            )
        })
        .collect()
}

fn absent_canaries(directory: &Directory) {
    for bytes in file_snapshot(directory).values() {
        for canary in [
            CANARY.as_bytes(),
            IDENTITY.as_bytes(),
            CHECKPOINT.as_bytes(),
            UPDATE.as_bytes(),
            &KEY,
            "5a".repeat(32).as_bytes(),
        ] {
            assert!(
                !bytes.windows(canary.len()).any(|window| window == canary),
                "main/sidecar bytes must not contain a complete synthetic canary/key"
            );
        }
    }
}

fn synced(path: &Path) -> CommandDatabase<SqliteStorage> {
    let mut db = run(CommandDatabase::open(
        protected(path),
        pulls::schema(),
        vec![],
        [0x42; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    db.configure_entities(
        EntityConfig {
            scope: "synthetic-secret-scope".into(),
            tables: ["items".into()].into_iter().collect(),
        },
        || {
            Ok(EntityContext {
                identity: Json::String(IDENTITY.into()),
                now_ms: 1000,
            })
        },
    )
    .unwrap();
    db
}

fn seed_recovery(path: &Path) {
    let mut db = synced(path);
    let request = db.begin_pull().unwrap().request;
    run(db.apply_pull(
        request,
        pulls::page(
            CHECKPOINT,
            false,
            vec![pulls::change("items", "remote:seed", Some(CANARY))],
        ),
    ))
    .unwrap();
    let row = run(db.insert(
        "items",
        [("label".into(), Value::String(CANARY.into()))]
            .into_iter()
            .collect(),
    ))
    .unwrap();
    let batch = db.prepare_push(1000, 0).unwrap();
    assert_eq!(batch.envelopes.len(), 1);
    let key = db.queue_status().commands[0].key.clone();
    db.receive_entity("items", &row.id, &key, None).unwrap();
    db.close();
}

#[test]
fn s02_supplied_key_opens_a_named_cipher_and_protects_real_rows() {
    let directory = Directory::new();
    let path = directory.path("protected.sqlite");
    let mut store = protected(&path);
    let info = store.storage_protection();
    assert!(
        info.encrypted,
        "protected opens must report actual encryption"
    );
    assert_eq!(info.cipher, "SQLCipher");
    assert!(
        info.version.starts_with("4.19.0"),
        "runtime cipher must match the approved source"
    );
    run(store.create_table("items")).unwrap();
    let row = Row {
        id: "private-record-id".into(),
        fields: BTreeMap::from([("label".into(), Value::String(CANARY.into()))]),
    };
    run(store.insert("items", row.clone())).unwrap();
    assert_eq!(run(store.get("items", &row.id)).unwrap(), Some(row.clone()));
    drop(store);
    let bytes = std::fs::read(&path).unwrap();
    assert!(!bytes.starts_with(b"SQLite format 3\0"));
    for secret in [
        CANARY.as_bytes(),
        b"private-record-id",
        &KEY,
        "5a".repeat(32).as_bytes(),
    ] {
        assert!(
            !bytes.windows(secret.len()).any(|window| window == secret),
            "raw store must omit the complete synthetic canary/key"
        );
    }
    let plain =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    assert!(
        plain
            .query_row("SELECT count(*) FROM sqlite_schema", [], |row| row
                .get::<_, i64>(0))
            .is_err(),
        "an independent unkeyed reader must not read schema"
    );
    drop(plain);
    assert_eq!(
        run(protected(&path).get("items", &row.id)).unwrap(),
        Some(row)
    );
}

#[test]
fn s02_s07_encrypted_recovery_preserves_identity_frozen_bytes_and_checkpoint() {
    let _serial = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    let directory = Directory::new();
    let path = directory.path("store.sqlite");
    seed_recovery(&path);
    absent_canaries(&directory);
    let store = protected(&path);
    let ledger = records::SyncLedger::from_rows(&store.load_sync().unwrap().unwrap()).unwrap();
    let record = ledger.records.values().next().unwrap();
    assert!(record.frozen.as_ref().unwrap().contains(IDENTITY));
    assert!(record.frozen.as_ref().unwrap().contains(CANARY));
    assert_eq!(record.state, SyncState::Acknowledged);
    assert_eq!(
        ledger.entity.as_ref().unwrap().checkpoint.as_deref(),
        Some(CHECKPOINT)
    );
    assert_eq!(ledger.header.namespace, "42".repeat(16));
    drop(store);
    let mut db = synced(&path);
    assert_eq!(db.sync_progress().pages, 1);
    assert!(!db.sync_progress().ready);
    assert_eq!(db.queue_status().awaiting_outcome, 1);
    assert!(db.prepare_push(900_000, 0).unwrap().envelopes.is_empty());
    assert_eq!(run(db.fetch(&db.query("items"))).unwrap().len(), 2);
    db.close();
}

#[test]
fn s03_wrong_missing_plaintext_and_foreign_opens_preserve_every_existing_byte() {
    let directory = Directory::new();
    let path = directory.path("encrypted.sqlite");
    let mut store = protected(&path);
    run(store.create_table("items")).unwrap();
    let row = Row {
        id: "r".into(),
        fields: [("label".into(), Value::String(CANARY.into()))]
            .into_iter()
            .collect(),
    };
    run(store.insert("items", row.clone())).unwrap();
    drop(store);
    let before = file_snapshot(&directory);
    assert!(SqliteStorage::open(&path).is_err());
    assert_eq!(file_snapshot(&directory), before);
    assert!(SqliteStorage::open_with_options(&path, SqliteOpenOptions::default()).is_err());
    assert_eq!(file_snapshot(&directory), before);
    assert!(
        SqliteStorage::open_with_options(
            &path,
            SqliteOpenOptions {
                encryption_key: Some(EncryptionKey::from_bytes([0xa5; 32]))
            }
        )
        .is_err()
    );
    assert_eq!(file_snapshot(&directory), before);
    assert!(SqliteStorage::open_unprotected(&path).is_err());
    assert_eq!(file_snapshot(&directory), before);
    assert_eq!(run(protected(&path).get("items", "r")).unwrap(), Some(row));

    let plaintext = directory.path("plaintext.sqlite");
    drop(SqliteStorage::open_unprotected(&plaintext).unwrap());
    let foreign = directory.path("foreign.sqlite");
    let sql = keyed_sql(&foreign);
    sql.execute_batch("CREATE TABLE unrelated(value TEXT); INSERT INTO unrelated VALUES('synthetic foreign value');").unwrap();
    drop(sql);
    for incompatible in [&plaintext, &foreign] {
        let before = file_snapshot(&directory);
        assert!(
            SqliteStorage::open_with_options(
                incompatible,
                SqliteOpenOptions {
                    encryption_key: Some(EncryptionKey::from_bytes(KEY))
                }
            )
            .is_err()
        );
        assert_eq!(
            file_snapshot(&directory),
            before,
            "rejected authenticated or plaintext formats must not be initialized/reset"
        );
    }
}

#[test]
fn s04_authenticated_page_tampering_rejects_without_repair_or_partial_owner() {
    let _serial = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    let directory = Directory::new();
    let original = directory.path("store.sqlite");
    seed_recovery(&original);
    let damaged = directory.path("damaged.sqlite");
    let mut bytes = std::fs::read(&original).unwrap();
    assert!(bytes.len() >= 4096);
    bytes[4095] ^= 1; // Corrupt the authenticated first page, never a user store.
    std::fs::write(&damaged, &bytes).unwrap();
    let before = file_snapshot(&directory);
    assert!(
        SqliteStorage::open_with_options(
            &damaged,
            SqliteOpenOptions {
                encryption_key: Some(EncryptionKey::from_bytes(KEY))
            }
        )
        .is_err(),
        "authenticated tampering must fail before an owner can publish data/progress"
    );
    assert_eq!(
        file_snapshot(&directory),
        before,
        "failed authentication must not repair or reset the input"
    );
    let db = synced(&original);
    assert_eq!(db.sync_progress().pages, 1);
    assert_eq!(db.queue_status().awaiting_outcome, 1);
    assert_eq!(run(db.fetch(&db.query("items"))).unwrap().len(), 2);
    db.close();
}

// Identical public conformance functions, with only their concrete factory
// changed to an encrypted adapter. Forwarding owns cleanup, not DB algorithms.
struct Fixture {
    storage: Option<SqliteStorage>,
    _directory: Directory,
    validated_queries: AtomicUsize,
}

fn fresh() -> Fixture {
    let directory = Directory::new();
    let storage = protected(&directory.path("store.sqlite"));
    assert!(storage.storage_protection().encrypted);
    Fixture {
        storage: Some(storage),
        _directory: directory,
        validated_queries: AtomicUsize::new(0),
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        drop(self.storage.take());
    }
}

impl Storage for Fixture {
    fn create_table(
        &mut self,
        name: &str,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.storage.as_mut().unwrap().create_table(name)
    }
    fn insert(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.storage.as_mut().unwrap().insert(table, row)
    }
    fn get(
        &self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<Option<Row>, StorageError>> + Send {
        self.storage.as_ref().unwrap().get(table, id)
    }
    fn replace(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.storage.as_mut().unwrap().replace(table, row)
    }
    fn remove(
        &mut self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send {
        self.storage.as_mut().unwrap().remove(table, id)
    }
}

impl TransactionalStorage for Fixture {
    fn apply_batch(
        &mut self,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.storage.as_mut().unwrap().apply_batch(mutations)
    }
}

impl TypedStorage for Fixture {
    fn query(
        &self,
        schema: &Schema,
        query: &Query,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.storage.as_ref().unwrap().query(schema, query)
    }
    fn query_validated(
        &self,
        proof: ValidatedQuery<'_>,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.validated_queries.fetch_add(1, Ordering::Relaxed);
        self.storage.as_ref().unwrap().query_validated(proof)
    }
}

impl TypedTransactionalStorage for Fixture {
    fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send {
        self.storage
            .as_mut()
            .unwrap()
            .apply_typed_batch(schema, mutations)
    }
}

impl SchemaStorage for Fixture {
    fn catalog(&self) -> impl Future<Output = Result<ModelCatalog, StorageError>> + Send {
        self.storage.as_ref().unwrap().catalog()
    }
    fn scan(&self, table: &str) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.storage.as_ref().unwrap().scan(table)
    }
    fn apply_schema_plan(
        &mut self,
        plan: SchemaPlan,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        self.storage.as_mut().unwrap().apply_schema_plan(plan)
    }
}

impl ObservationStorage for Fixture {
    fn project(
        &self,
        schema: &Schema,
        query: &Query,
        fields: &[String],
    ) -> impl Future<Output = Result<Vec<ProjectedRow>, StorageError>> + Send {
        self.storage
            .as_ref()
            .unwrap()
            .project(schema, query, fields)
    }
}

impl SyncStorage for Fixture {
    fn ownership(&self) -> StoreOwnership {
        self.storage.as_ref().unwrap().ownership()
    }
    fn reads_ready(&self) -> bool {
        self.storage.as_ref().unwrap().reads_ready()
    }

    fn load_sync(&self) -> Result<Option<Vec<Row>>, StorageError> {
        self.storage.as_ref().unwrap().load_sync()
    }
    fn apply_sync_plan(&mut self, plan: SyncPlan) -> Result<Vec<CommittedChange>, StorageError> {
        self.storage.as_mut().unwrap().apply_sync_plan(plan)
    }
}

#[path = "../../../tests/conformance/data_model.rs"]
mod model_conformance;
#[path = "../../../tests/conformance/reactivity.rs"]
mod reactive_conformance;
#[path = "../../../tests/conformance/typed_storage.rs"]
mod typed_conformance;

#[test]
fn s06_encrypted_command_owner_contract() {
    let _serial = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    commands::owner_contract(fresh);
}

#[test]
fn s06_encrypted_command_dispatch_reconcile_and_capacity() {
    let _serial = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    sync_cases::dispatch(fresh());
    sync_cases::correction_inverse(fresh());
    sync_cases::graph(fresh());
    sync_cases::zero_history(fresh());
    sync_cases::capacity_boundary(fresh());
}

#[test]
fn s06_encrypted_entity_mutation_mapping_and_resolution() {
    let _serial = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    entities::automatic_entities(fresh);
    entities::admission_and_inverse(fresh);
    entities::two_client_resolution(fresh);
    entities::managed_links_and_mapped_rows(fresh);
    entities::reconfiguration_guards(fresh);
}

#[test]
fn s06_encrypted_pull_bootstrap_reset_and_comparison_guards() {
    let _serial = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    pulls::pull_atomicity_bootstrap_and_reset(fresh());
    pulls::reset_inventory(fresh());
    pulls::lifecycle_comparison_guard(fresh());
    pulls::pull_recovery_protection(fresh());
    pulls::pull_tombstone_mapping_target(fresh());
    pulls::pull_after_rolled_back_sent_cancellation(fresh());
    pulls::progress_transition_guard(fresh());
    pulls::reset_modes_and_coordinator_guard(fresh);
    pulls::typed_correction_clocks(fresh());
    pulls::reconfigure_inventory(fresh());
}

fn publish_barrier(directory: &Path, mode: &str, after_create: impl FnOnce()) {
    let path = directory.join("barrier");
    let temporary = directory.join(format!("barrier.{}.tmp", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .unwrap();
    after_create();
    std::io::Write::write_all(&mut file, mode.as_bytes()).unwrap();
    file.sync_all().unwrap();
    std::fs::rename(temporary, path).unwrap();
}

#[test]
fn si_k03_barrier_becomes_visible_only_with_complete_mode() {
    let directory = Directory::new();
    publish_barrier(&directory.0, "journal-precommit", || {
        assert!(
            !directory.path("barrier").exists(),
            "readiness must not be visible before the complete mode is written"
        );
    });
    assert_eq!(
        std::fs::read_to_string(directory.path("barrier")).unwrap(),
        "journal-precommit"
    );
    let entries = std::fs::read_dir(&directory.0)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    assert_eq!(entries, vec![std::ffi::OsString::from("barrier")]);
}

fn barrier(directory: &Path, mode: &str) -> ! {
    publish_barrier(directory, mode, || {});
    loop {
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn s05_child_process_entry() {
    let Some(mode) = std::env::var_os("HOYA_SECURITY_CHILD_MODE") else {
        return;
    };
    let mode = mode.to_str().unwrap();
    let directory =
        PathBuf::from(std::env::var_os("HOYA_SECURITY_CHILD_ROOT").expect("test-owned root"));
    let canonical = std::fs::canonicalize(&directory).unwrap();
    assert_eq!(canonical, directory);
    assert!(directory.starts_with(std::fs::canonicalize(std::env::temp_dir()).unwrap()));
    assert!(
        directory
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("hoya-encrypted-engine-")
    );
    let path = directory.join("store.sqlite");
    match mode {
        "journal-precommit" | "wal-postcommit" => {
            let sql = keyed_sql(&path);
            let journal = if mode == "journal-precommit" {
                "PERSIST"
            } else {
                "WAL"
            };
            let actual: String = sql
                .query_row(&format!("PRAGMA journal_mode={journal}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(actual.to_uppercase(), journal);
            sql.execute_batch("PRAGMA synchronous=FULL; PRAGMA cache_size=4; PRAGMA cache_spill=ON; PRAGMA wal_autocheckpoint=0;").unwrap();
            if mode == "journal-precommit" {
                sql.execute_batch("BEGIN IMMEDIATE").unwrap();
            }
            sql.execute(
                "UPDATE __hoya_fields SET text_value=?1 WHERE table_name='items' AND name='label'",
                [UPDATE.repeat(10_000)],
            )
            .unwrap();
            barrier(&directory, mode)
        }
        "owner-precommit" | "owner-postcommit" => {
            let mut db = synced(&path);
            let request = db.request().unwrap();
            assert!(
                run(db.begin(
                    request,
                    CommandDefinition {
                        name: "security-v1".into(),
                        arguments: [(
                            "value".into(),
                            Argument {
                                kind: ArgumentType::String,
                                optional: false
                            }
                        )]
                        .into_iter()
                        .collect(),
                        reference_paths: vec![],
                    },
                    Json::Object(
                        [("value".into(), Json::String(CANARY.into()))]
                            .into_iter()
                            .collect()
                    ),
                    Some(Json::String(IDENTITY.into())),
                    2000
                ))
                .unwrap()
            );
            run(db.scope_insert(
                request,
                "items",
                [("label".into(), Value::String(UPDATE.into()))]
                    .into_iter()
                    .collect(),
            ))
            .unwrap();
            if mode == "owner-postcommit" {
                run(db.finish(request, Json::String(CANARY.into()))).unwrap();
                assert_eq!(db.prepare_push(2000, 0).unwrap().envelopes.len(), 1);
            }
            barrier(&directory, mode)
        }
        _ => panic!("unknown owned security child mode"),
    }
}

struct Child(std::process::Child);

impl Child {
    fn at_barrier(directory: &Directory, mode: &str) -> Self {
        let mut child = Self(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "s05_child_process_entry", "--nocapture"])
                .env("HOYA_SECURITY_CHILD_MODE", mode)
                .env(
                    "HOYA_SECURITY_CHILD_ROOT",
                    std::fs::canonicalize(&directory.0).unwrap(),
                )
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(30);
        while !directory.path("barrier").exists() {
            if let Some(status) = child.0.try_wait().unwrap() {
                let stderr = child.safe_stderr();
                panic!("child exited before its named commit barrier: {status}; {stderr}");
            }
            assert!(
                Instant::now() < deadline,
                "child did not reach its bounded commit barrier"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            std::fs::read_to_string(directory.path("barrier")).unwrap(),
            mode
        );
        assert!(
            !directory
                .path(&format!("barrier.{}.tmp", child.0.id()))
                .exists(),
            "temporary readiness file must be gone before the parent proceeds"
        );
        assert_ne!(child.0.id(), std::process::id());
        child
    }

    fn terminate(&mut self) {
        self.0.kill().unwrap();
        assert!(
            !self.0.wait().unwrap().success(),
            "named interruption must terminate the actual writer process"
        );
        self.safe_stderr();
    }

    fn safe_stderr(&mut self) -> String {
        let mut bytes = Vec::new();
        self.0
            .stderr
            .take()
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        for secret in [
            CANARY.as_bytes(),
            IDENTITY.as_bytes(),
            CHECKPOINT.as_bytes(),
            UPDATE.as_bytes(),
            &KEY,
            "5a".repeat(32).as_bytes(),
        ] {
            assert!(
                !bytes.windows(secret.len()).any(|window| window == secret),
                "child stderr must not contain domain/key canaries"
            );
        }
        String::from_utf8(bytes).unwrap()
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn s05_real_journal_and_wal_pages_stay_encrypted_through_process_interruption() {
    for (mode, suffix, committed) in [
        ("journal-precommit", "store.sqlite-journal", false),
        ("wal-postcommit", "store.sqlite-wal", true),
    ] {
        let directory = Directory::new();
        let path = directory.path("store.sqlite");
        let mut store = protected(&path);
        run(store.create_table("items")).unwrap();
        let original = Row {
            id: "r".into(),
            fields: [("label".into(), Value::String(CANARY.repeat(10_000)))]
                .into_iter()
                .collect(),
        };
        run(store.insert("items", original.clone())).unwrap();
        drop(store);
        let mut child = Child::at_barrier(&directory, mode);
        assert!(
            std::fs::metadata(directory.path(suffix)).unwrap().len() > 512,
            "real encrypted sidecar pages must exist before interruption"
        );
        absent_canaries(&directory);
        child.terminate();
        let actual = run(protected(&path).get("items", "r")).unwrap().unwrap();
        let expected = if committed {
            UPDATE.repeat(10_000)
        } else {
            CANARY.repeat(10_000)
        };
        assert!(
            actual.fields["label"] == Value::String(expected),
            "reopen must respect the named pre/postcommit barrier"
        );
        absent_canaries(&directory);
    }
}

#[test]
fn s07_command_and_entity_recovery_survive_distinct_pre_and_postcommit_processes() {
    let _serial = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    for (mode, committed) in [("owner-precommit", false), ("owner-postcommit", true)] {
        let directory = Directory::new();
        let path = directory.path("store.sqlite");
        seed_recovery(&path);
        let mut child = Child::at_barrier(&directory, mode);
        absent_canaries(&directory);
        child.terminate();
        let store = protected(&path);
        assert_eq!(
            run(store.catalog()).unwrap().state.unwrap().next_id,
            if committed {
                3
            } else {
                2
            }
        );
        let ledger = records::SyncLedger::from_rows(&store.load_sync().unwrap().unwrap()).unwrap();
        assert_eq!(ledger.header.namespace, "42".repeat(16));
        let mut expected_retry = ledger
            .records
            .values()
            .filter(|record| record.state == SyncState::Sent)
            .collect::<Vec<_>>();
        expected_retry.sort_by_key(|record| record.order);
        let expected_retry = expected_retry
            .into_iter()
            .map(|record| record.frozen.clone().unwrap())
            .collect::<Vec<_>>();
        let mut db = run(CommandDatabase::open(
            store,
            pulls::schema(),
            vec![],
            [0x99; 16],
            SyncOptions::default(),
        ))
        .unwrap();
        assert_eq!(db.sync_progress().pages, 1);
        assert!(!db.sync_progress().ready);
        assert_eq!(db.queue_status().awaiting_outcome, 1);
        assert_eq!(db.queue_status().sending, usize::from(committed));
        assert_eq!(
            run(db.fetch(&db.query("items"))).unwrap().len(),
            if committed {
                3
            } else {
                2
            }
        );
        let request = db.begin_pull().unwrap();
        assert_eq!(request.since.as_deref(), Some(CHECKPOINT));
        db.abort_pull(request.request).unwrap();
        assert_eq!(
            db.prepare_push(900_000, 0).unwrap().envelopes,
            expected_retry,
            "acknowledged entity must not resend; command retry bytes remain exact"
        );
        db.close();
        absent_canaries(&directory);
    }
}

#[test]
fn s05_large_operand_temp_query_preserves_owned_file_inventory() {
    let directory = Directory::new();
    let path = directory.path("store.sqlite");
    let mut store = protected(&path);
    run(store.create_table("items")).unwrap();
    let schema = Schema {
        tables: [(
            "items".into(),
            TableSchema {
                columns: [(
                    "label".into(),
                    Column {
                        kind: ColumnType::String,
                        optional: false,
                    },
                )]
                .into_iter()
                .collect(),
                relations: BTreeMap::new(),
            },
        )]
        .into_iter()
        .collect(),
    };
    let row = Row {
        id: "r".into(),
        fields: [("label".into(), Value::String(CANARY.into()))]
            .into_iter()
            .collect(),
    };
    run(store.apply_typed_batch(
        &schema,
        vec![Mutation::Insert {
            table: "items".into(),
            row: row.clone(),
        }],
    ))
    .unwrap();
    let before = file_snapshot(&directory).into_keys().collect::<Vec<_>>();
    let values = (0..33_005)
        .map(|n| Value::String(format!("missing-{n}")))
        .chain([Value::String("r".into())])
        .collect();
    let query = Query {
        table: "items".into(),
        predicates: vec![Predicate {
            field: "id".into(),
            operator: Operator::In,
            values,
        }],
        order: Some(OrderBy {
            field: "label".into(),
            direction: Direction::Desc,
        }),
        limit: None,
    };
    assert_eq!(run(store.query(&schema, &query)).unwrap(), vec![row]);
    assert_eq!(
        file_snapshot(&directory).into_keys().collect::<Vec<_>>(),
        before
    );
    drop(store);
    absent_canaries(&directory);
}

/// TC-0048 QUALITY08 / DEF-0042: cleanup wrappers preserve adapter capabilities.
#[test]
fn quality08_encrypted_fixture_uses_native_validated_queries() {
    let fixture = fresh();
    assert_eq!(fixture.ownership(), StoreOwnership::Exclusive);
    assert!(fixture.reads_ready());
    let mut db = run(CommandDatabase::open(
        fixture,
        commands::schema(),
        vec![],
        [0x48; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    let row = run(db.insert("parents", commands::fields("protected"))).unwrap();
    let query = db.query("parents").limit(1);
    assert_eq!(run(db.fetch(&query)).unwrap(), vec![row]);
    assert!(db.into_storage().validated_queries.load(Ordering::Relaxed) > 0);
}
