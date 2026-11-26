//! SPEC-0037 R11 / TC-0041 SI-X01..X08, SI-XL01..XL06: the real converter binary
//! and library entry against synthetic stores only. Raw SQLite children are this
//! same test executable re-run with `HOYA_CONVERT_CHILD`, coordinated through
//! READY/GO files; no application-owned store is ever touched.
#![cfg(all(feature = "ledger-convert-tool", not(target_arch = "wasm32")))]
use hoyadb_core::{
    command::{
        records::{SyncHeader, SyncLedger},
        *,
    },
    model::*,
    storage::*,
};
use hoyadb_storage_sqlite::{EncryptionKey, SqliteOpenOptions, SqliteStorage};
use std::{
    collections::BTreeMap,
    fs,
    future::Future,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
    time::{Duration, Instant},
};

const KEY: [u8; 32] = [0x5a; 32];
const WRONG_KEY: [u8; 32] = [0xa5; 32];
const TOOL: &str = env!("CARGO_BIN_EXE_hoyadb-convert-ledger");

fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("SQLite adapter unexpectedly suspended"),
    }
}

struct Dir(PathBuf);
impl Dir {
    fn new(label: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "hoya-convert-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([
            (
                "items".into(),
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
                "links".into(),
                TableDefinition {
                    columns: BTreeMap::from([(
                        "item".into(),
                        Column {
                            kind: ColumnType::Reference,
                            optional: false,
                        },
                    )]),
                    ..Default::default()
                },
            ),
        ]),
    }
}
fn config() -> EntityConfig {
    EntityConfig {
        scope: "convert-cases".into(),
        tables: ["items", "links"].into_iter().map(Into::into).collect(),
    }
}
fn provider() -> Result<EntityContext, CommandError> {
    Ok(EntityContext {
        identity: Json::Null,
        now_ms: 100,
    })
}
fn object<const N: usize>(items: [(&str, Json); N]) -> Json {
    Json::Object(items.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
fn text(value: &str) -> Json {
    Json::String(value.into())
}
fn open_store(path: &Path, key: Option<[u8; 32]>) -> SqliteStorage {
    match key {
        Some(key) => SqliteStorage::open_with_options(
            path,
            SqliteOpenOptions {
                encryption_key: Some(EncryptionKey::from_bytes(key)),
            },
        )
        .unwrap(),
        None => SqliteStorage::open_unprotected(path).unwrap(),
    }
}
/// Authored format-2 bytes through the public full plan, independent of
/// EntityLedger::new's default (format3_scaling uses the same construction).
fn legacy(store: SqliteStorage) -> SqliteStorage {
    let mut store = run(Database::open(store, schema(), vec![]))
        .unwrap()
        .into_storage();
    let model = run(store.catalog()).unwrap().state.unwrap();
    let mut rows = SyncLedger {
        header: SyncHeader {
            namespace: "52".repeat(16),
            sequence: 1,
            last_ms: 0,
            options: SyncOptions::default(),
        },
        entity: None,
        records: BTreeMap::new(),
    }
    .to_rows()
    .unwrap();
    rows.push(Row {
        id: "entity:state".into(),
        fields: BTreeMap::from([
            ("format".into(), Value::Integer(2)),
            (
                "data".into(),
                Value::String(
                    object([
                        (
                            "config",
                            object([
                                ("scope", text("convert-cases")),
                                ("tables", Json::Array(vec![text("items"), text("links")])),
                            ]),
                        ),
                        ("generation", Json::Integer(1)),
                        ("revision", Json::Integer(0)),
                        ("checkpoint", Json::Null),
                        ("ready", Json::Bool(false)),
                        ("pages", Json::Integer(0)),
                        ("changes", Json::Integer(0)),
                    ])
                    .canonical()
                    .unwrap(),
                ),
            ),
        ]),
    });
    store
        .apply_sync_plan(SyncPlan {
            transition: SyncTransition::Normal,
            expected_model: model.clone(),
            next_model: model,
            expected_sync: None,
            next_sync: rows,
            guards: vec![],
            mutations: vec![],
        })
        .unwrap();
    store
}
fn change(id: &str) -> EntityChange {
    EntityChange {
        table: "items".into(),
        record_id: id.into(),
        op: EntityOperation::Update,
        changes: BTreeMap::from([("body".into(), Value::String("remote 雪".into()))]),
        removed_fields: Default::default(),
        timestamp_ms: 10,
        field_timestamps: BTreeMap::from([("body".into(), 10)]),
    }
}
/// Four pulled identities plus the two local entity writes below: M=6.
const PULLED: usize = 4;
const METADATA: usize = PULLED + 2;
/// A complete synthetic entity store: six metadata entries, application rows and
/// two queued local commands (an entity write and a typed reference dependent).
fn seed(path: &Path, key: Option<[u8; 32]>, format: u8) -> ModelState {
    let store = open_store(path, key);
    let store = if format == 2 {
        legacy(store)
    } else {
        store
    };
    let mut db = run(CommandDatabase::open(
        store,
        schema(),
        vec![],
        [0x52; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    db.configure_entities(config(), provider).unwrap();
    let request = db.begin_pull().unwrap().request;
    run(db.apply_pull(
        request,
        PullPage {
            checkpoint: "cp\n雪".into(),
            complete: true,
            changes: (0..PULLED)
                .map(|n| change(&format!("server:r{n}")))
                .collect(),
        },
    ))
    .unwrap();
    let parent = run(db.insert(
        "items",
        BTreeMap::from([("body".into(), Value::String("local 🙂".into()))]),
    ))
    .unwrap();
    run(db.insert(
        "links",
        BTreeMap::from([("item".into(), Value::Reference(parent.id))]),
    ))
    .unwrap();
    db.prepare_push(100, 0).unwrap();
    let store = db.into_storage();
    run(store.catalog()).unwrap().state.unwrap()
}
fn command_only(path: &Path) {
    let mut db = run(CommandDatabase::open(
        open_store(path, None),
        schema(),
        vec![],
        [0x52; 16],
        SyncOptions::default(),
    ))
    .unwrap();
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
    run(db.scope_insert(
        ticket,
        "items",
        BTreeMap::from([("body".into(), Value::String("x".into()))]),
    ))
    .unwrap();
    run(db.finish(ticket, Json::Null)).unwrap();
}

/// FNV-1a over a file's bytes, so inventory mismatches print compactly.
fn digest(bytes: &[u8]) -> (usize, u64) {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    (bytes.len(), hash)
}
/// Every directory entry with its exact size and digest: main file, sidecars,
/// backups and key files. Child barrier files are excluded.
fn inventory(dir: &Dir) -> BTreeMap<String, (usize, u64)> {
    let mut map = BTreeMap::new();
    for entry in fs::read_dir(&dir.0).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type().unwrap().is_file()
            && !name.starts_with("ready-")
            && !name.starts_with("go-")
        {
            map.insert(name, digest(&fs::read(entry.path()).unwrap()));
        }
    }
    map
}
fn sidecars(dir: &Dir) -> Vec<String> {
    inventory(dir)
        .into_keys()
        .filter(|name| {
            name.ends_with("-journal") || name.ends_with("-wal") || name.ends_with("-shm")
        })
        .collect()
}
fn raw(path: &Path, key: Option<[u8; 32]>) -> rusqlite::Connection {
    let connection =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)
            .unwrap();
    if let Some(key) = key {
        let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
        connection
            .execute_batch(&format!(
                "PRAGMA key=\"x'{hex}'\"; PRAGMA cipher_compatibility=4;"
            ))
            .unwrap();
    }
    connection.busy_timeout(Duration::ZERO).unwrap();
    connection
}
type FieldRow = (String, String, String, i64, Option<String>, Option<i64>);
type Dump = (Vec<(String, String)>, Vec<FieldRow>);
/// Canonical model and application dumps straight from the private tables,
/// excluding recovery packets; the ledger is compared logically.
fn dumps(path: &Path, key: Option<[u8; 32]>) -> Dump {
    let connection = raw(path, key);
    let rows = connection
        .prepare(
            "SELECT table_name,id FROM __hoya_rows WHERE table_name<>'__hoya_sync' ORDER BY 1,2",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let fields = connection
        .prepare("SELECT table_name,row_id,name,kind,text_value,int_value FROM __hoya_fields WHERE table_name<>'__hoya_sync' ORDER BY 1,2,3")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    (rows, fields)
}
fn ledger(path: &Path, key: Option<[u8; 32]>) -> (Vec<Row>, SyncLedger) {
    let store = open_store(path, key);
    let rows = store.load_sync().unwrap().unwrap();
    let ledger = SyncLedger::from_rows(&rows).unwrap();
    (rows, ledger)
}
fn write_key(dir: &Dir, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.path(name);
    fs::write(&path, bytes).unwrap();
    path
}

struct Outcome {
    code: i32,
    stdout: String,
    stderr: String,
}
fn tool(args: &[&str]) -> Outcome {
    let output = Command::new(TOOL).args(args).output().unwrap();
    Outcome {
        code: output.status.code().unwrap(),
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}
fn assert_redacted(outcome: &Outcome, dir: &Dir) {
    for text in [&outcome.stdout, &outcome.stderr] {
        assert!(!text.contains("5a5a"), "key bytes printed");
        assert!(
            !text.contains("雪") && !text.contains("local"),
            "rows or packets printed"
        );
        assert!(!text.contains(dir.0.to_str().unwrap()), "path printed");
    }
}
fn report(m: usize, from: u8, to: u8) -> String {
    format!(
        "{{\"from\":{from},\"to\":{to},\"metadata_entries\":{m},\"packets_rewritten\":{}}}\n",
        m + 1
    )
}
fn assert_format(ledger: &SyncLedger, format: u8) {
    let entity = ledger.entity.as_ref().unwrap();
    assert_eq!(entity.ledger_format, format);
    let rows = ledger.to_rows().unwrap();
    for row in rows.iter().filter(|r| r.id.starts_with("entity:metadata:")) {
        let suffix = &row.id["entity:metadata:".len()..];
        if format == 2 {
            assert_eq!(suffix.len(), 16);
        } else {
            assert!(suffix.len() > 16 || suffix.starts_with("00000005"));
        }
    }
}

// ---- same-executable raw SQLite children ---------------------------------

fn wait_for(path: &Path) {
    let start = Instant::now();
    while !path.exists() {
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "child never reached {path:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
struct ChildLease {
    child: Child,
    go: PathBuf,
}
impl ChildLease {
    /// Starts a raw SQLite child in `mode`, blocks until it signals READY.
    fn start(dir: &Dir, mode: &str, store: &Path, key: Option<[u8; 32]>) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let ready = dir.path(&format!("ready-{n}"));
        let go = dir.path(&format!("go-{n}"));
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "child_main", "--nocapture", "--test-threads=1"])
            .env("HOYA_CONVERT_CHILD", mode)
            .env("HOYA_CONVERT_STORE", store)
            .env("HOYA_CONVERT_READY", &ready)
            .env("HOYA_CONVERT_GO", &go)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        if let Some(key) = key {
            command.env(
                "HOYA_CONVERT_KEY",
                key.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            );
        }
        let child = command.spawn().unwrap();
        wait_for(&ready);
        Self {
            child,
            go,
        }
    }
    fn release(mut self) {
        fs::write(&self.go, b"go").unwrap();
        let status = self.child.wait().unwrap();
        assert!(status.success(), "child exited {status}");
    }
}

/// Asks from a separate process whether a writer can take RESERVED right now.
/// Same-process connections share SQLite's own lock table and cannot observe
/// kernel locks this process has lost.
fn probe_writer(dir: &Dir, store: &Path) -> String {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let ready = dir.path(&format!("ready-probe-{n}"));
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_main", "--nocapture", "--test-threads=1"])
        .env("HOYA_CONVERT_CHILD", "probe-write")
        .env("HOYA_CONVERT_STORE", store)
        .env("HOYA_CONVERT_READY", &ready)
        .env("HOYA_CONVERT_GO", dir.path(&format!("go-probe-{n}")))
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .unwrap();
    assert!(status.success(), "probe exited {status}");
    fs::read_to_string(&ready).unwrap()
}

/// Child entry: only acts when spawned by `ChildLease`; otherwise a no-op test.
#[test]
fn child_main() {
    let Ok(mode) = std::env::var("HOYA_CONVERT_CHILD") else {
        return;
    };
    let store = PathBuf::from(std::env::var("HOYA_CONVERT_STORE").unwrap());
    let ready = PathBuf::from(std::env::var("HOYA_CONVERT_READY").unwrap());
    let go = PathBuf::from(std::env::var("HOYA_CONVERT_GO").unwrap());
    let key = std::env::var("HOYA_CONVERT_KEY").ok().map(|hex| {
        let mut key = [0_u8; 32];
        for (i, byte) in key.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap();
        }
        key
    });
    let connection = raw(&store, key);
    match mode.as_str() {
        "reserved" => connection.execute_batch("BEGIN IMMEDIATE").unwrap(),
        "shared" => {
            connection.execute_batch("BEGIN").unwrap();
            connection
                .query_row("SELECT count(*) FROM __hoya_tables", [], |r| {
                    r.get::<_, i64>(0)
                })
                .unwrap();
        }
        // A real hot journal. SQLite zeroes the journal magic until the journal is
        // synced, which happens when dirty pages spill to the database file
        // mid-transaction; a tiny cache plus many large rows forces that spill,
        // and the process then dies without committing.
        "hot-journal" => {
            connection
                .execute_batch("PRAGMA cache_size=2; BEGIN IMMEDIATE;")
                .unwrap();
            let mut insert = connection
                .prepare("INSERT INTO __hoya_rows(table_name,id) VALUES('zz-hot',?1)")
                .unwrap();
            for n in 0..400 {
                insert
                    .execute([format!("{n}-{}", "x".repeat(2000))])
                    .unwrap();
            }
        }
        // A crashed WAL writer: committed frames, sidecars left behind at exit.
        "wal-crash" => connection
            .execute_batch("INSERT INTO __hoya_tables(name) VALUES('zz-wal');")
            .unwrap(),
        // A separate process asking for the write lease the converter must hold.
        // Reports the outcome through its READY file and exits without waiting.
        "probe-write" => {
            let outcome = match connection.execute_batch("BEGIN IMMEDIATE") {
                Ok(()) => {
                    connection.execute_batch("ROLLBACK").unwrap();
                    "acquired"
                }
                Err(error)
                    if error.sqlite_error_code() == Some(rusqlite::ErrorCode::DatabaseBusy) =>
                {
                    "busy"
                }
                Err(error) => panic!("unexpected probe failure {error}"),
            };
            drop(connection);
            fs::write(&ready, outcome).unwrap();
            return;
        }
        other => panic!("unknown child mode {other}"),
    }
    fs::write(&ready, b"ready").unwrap();
    wait_for(&go);
    if mode == "hot-journal" || mode == "wal-crash" {
        // Leave the journal/WAL exactly as a killed process would.
        std::process::exit(0);
    }
    drop(connection);
}

// ---- SI-X01 / SI-XL06: round trip through the real binary ----------------

fn round_trip(key: Option<[u8; 32]>, label: &str) {
    let dir = Dir::new(label);
    let store = dir.path("store.sqlite");
    let model = seed(&store, key, 2);
    let (source_rows, source_ledger) = ledger(&store, key);
    assert_format(&source_ledger, 2);
    let (rows_before, fields_before) = dumps(&store, key);
    let main_before = fs::read(&store).unwrap();
    assert!(sidecars(&dir).is_empty());
    let key_args: Vec<String> = match key {
        Some(_) => vec![
            "--key-file".into(),
            write_key(&dir, "key", &KEY).to_string_lossy().into(),
        ],
        None => vec!["--unprotected".into()],
    };
    let key_args: Vec<&str> = key_args.iter().map(String::as_str).collect();
    let backup = dir.path("backup-v2.sqlite");
    let mut args = vec!["--store", store.to_str().unwrap(), "--to", "v3"];
    args.extend(&key_args);
    args.extend(["--backup", backup.to_str().unwrap()]);
    let outcome = tool(&args);
    assert_eq!(
        (outcome.code, outcome.stderr.as_str()),
        (0, ""),
        "{}",
        outcome.stdout
    );
    assert_eq!(outcome.stdout, report(METADATA, 2, 3));
    assert_redacted(&outcome, &dir);
    assert_eq!(
        fs::read(&backup).unwrap(),
        main_before,
        "backup is the exact pre-write main file"
    );
    assert!(sidecars(&dir).is_empty(), "no sidecar after close");
    let (v3_rows, v3) = ledger(&store, key);
    assert_format(&v3, 3);
    let mut normalized = v3.clone();
    normalized.entity.as_mut().unwrap().ledger_format = 2;
    assert_eq!(
        normalized, source_ledger,
        "only the marker changed logically"
    );
    assert_eq!(
        dumps(&store, key),
        (rows_before.clone(), fields_before.clone())
    );
    assert_eq!(
        run(open_store(&store, key).catalog())
            .unwrap()
            .state
            .unwrap(),
        model
    );
    // Untouched packets are byte-identical; metadata packets change id only.
    let by_id = |rows: &[Row]| {
        rows.iter()
            .map(|r| (r.id.clone(), r.fields.clone()))
            .collect::<BTreeMap<_, _>>()
    };
    let (before, after) = (by_id(&source_rows), by_id(&v3_rows));
    for (id, fields) in &before {
        if !id.starts_with("entity:") {
            assert_eq!(after.get(id), Some(fields), "{id}");
        }
    }
    let backup2 = dir.path("backup-v3.sqlite");
    let mut args = vec!["--store", store.to_str().unwrap(), "--to", "v2"];
    args.extend(&key_args);
    args.extend(["--backup", backup2.to_str().unwrap()]);
    let outcome = tool(&args);
    assert_eq!(outcome.code, 0, "{}", outcome.stderr);
    assert_eq!(outcome.stdout, report(METADATA, 3, 2));
    let (restored_rows, restored) = ledger(&store, key);
    assert_eq!(restored, source_ledger);
    assert_eq!(
        by_id(&restored_rows),
        before,
        "exact ledger ids and packet field bytes restored"
    );
    assert_eq!(dumps(&store, key), (rows_before, fields_before));
    assert!(sidecars(&dir).is_empty());
    // The strict owner reopens the restored store and sees the queued work.
    let db = run(CommandDatabase::open(
        open_store(&store, key),
        schema(),
        vec![],
        [0x52; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    // The parent entity write was dispatched (Sent); its typed dependent stays queued.
    let status = db.queue_status();
    assert_eq!((status.pending, status.live), (1, 2));
    assert_eq!(db.sync_progress().changes, PULLED as i64);
}

#[test]
fn si_x01_unprotected_v2_v3_v2_round_trip_restores_exact_bytes_and_counts() {
    round_trip(None, "roundtrip-plain");
}

#[test]
fn si_x01_protected_v2_v3_v2_round_trip_restores_exact_bytes_and_counts() {
    round_trip(Some(KEY), "roundtrip-protected");
}

#[test]
fn si_x03_dry_run_reports_and_changes_nothing() {
    let dir = Dir::new("dry-run");
    let store = dir.path("store.sqlite");
    seed(&store, Some(KEY), 3);
    let key = write_key(&dir, "key", &KEY);
    let before = inventory(&dir);
    let outcome = tool(&[
        "--store",
        store.to_str().unwrap(),
        "--to",
        "v2",
        "--key-file",
        key.to_str().unwrap(),
        "--dry-run",
    ]);
    assert_eq!((outcome.code, outcome.stderr.as_str()), (0, ""));
    assert_eq!(outcome.stdout, report(METADATA, 3, 2));
    assert_eq!(
        inventory(&dir),
        before,
        "dry run changes no byte and creates no file"
    );
    let missing = dir.path("absent.sqlite");
    let outcome = tool(&[
        "--store",
        missing.to_str().unwrap(),
        "--to",
        "v2",
        "--key-file",
        key.to_str().unwrap(),
        "--dry-run",
    ]);
    assert_eq!(outcome.code, 2);
    assert!(!missing.exists(), "no missing store is created");
    assert_eq!(inventory(&dir), before);
}

// ---- SI-X02: refusals that write nothing ----------------------------------

#[test]
fn si_x02_arguments_sources_and_targets_are_refused_without_writing() {
    let dir = Dir::new("refusals");
    let store = dir.path("store.sqlite");
    seed(&store, None, 3);
    let s = store.to_str().unwrap();
    let before = inventory(&dir);
    let cases: Vec<Vec<&str>> = vec![
        vec![],
        vec!["--store", s],
        vec!["--store", s, "--to", "v4", "--unprotected", "--dry-run"],
        vec!["--store", s, "--to", "v3", "--unprotected"],
        vec![
            "--store",
            s,
            "--to",
            "v3",
            "--unprotected",
            "--dry-run",
            "--backup",
            "x",
        ],
        vec![
            "--store",
            s,
            "--to",
            "v3",
            "--unprotected",
            "--key-file",
            "k",
            "--dry-run",
        ],
        vec![
            "--store",
            s,
            "--to",
            "v3",
            "--unprotected",
            "--dry-run",
            "--store",
            s,
        ],
        vec!["--store", s, "--to", "v3", "--unprotected", "--dry-run"], // target already present
    ];
    for args in &cases {
        let outcome = tool(args);
        assert_eq!(outcome.code, 2, "{args:?}: {}", outcome.stderr);
        assert_redacted(&outcome, &dir);
    }
    // Missing, empty, directory and symlinked (no-follow) stores.
    let empty = dir.path("empty.sqlite");
    fs::write(&empty, b"").unwrap();
    let link = dir.path("link.sqlite");
    std::os::unix::fs::symlink(&store, &link).unwrap();
    for path in [
        dir.path("missing.sqlite"),
        empty.clone(),
        dir.0.clone(),
        link.clone(),
    ] {
        let outcome = tool(&[
            "--store",
            path.to_str().unwrap(),
            "--to",
            "v2",
            "--unprotected",
            "--dry-run",
        ]);
        assert_eq!(outcome.code, 2, "{path:?}");
    }
    assert!(!dir.path("missing.sqlite").exists());
    fs::remove_file(&link).unwrap();
    fs::remove_file(&empty).unwrap();
    // A command-only store has no entity ledger.
    let plain = dir.path("command-only.sqlite");
    command_only(&plain);
    let outcome = tool(&[
        "--store",
        plain.to_str().unwrap(),
        "--to",
        "v3",
        "--unprotected",
        "--dry-run",
    ]);
    assert_eq!(outcome.code, 2);
    // A foreign SQLite file and a noncanonical packet are invalid sources.
    let foreign = dir.path("foreign.sqlite");
    rusqlite::Connection::open(&foreign)
        .unwrap()
        .execute_batch("CREATE TABLE t(x)")
        .unwrap();
    let outcome = tool(&[
        "--store",
        foreign.to_str().unwrap(),
        "--to",
        "v3",
        "--unprotected",
        "--dry-run",
    ]);
    assert_eq!(outcome.code, 2);
    let edited = dir.path("edited.sqlite");
    seed(&edited, None, 2);
    raw(&edited, None)
        .execute("UPDATE __hoya_fields SET text_value=text_value||' ' WHERE table_name='__hoya_sync' AND name='data' AND row_id='entity:state'", [])
        .unwrap();
    let edited_bytes = fs::read(&edited).unwrap();
    let backup = dir.path("edited-backup.sqlite");
    let outcome = tool(&[
        "--store",
        edited.to_str().unwrap(),
        "--to",
        "v3",
        "--unprotected",
        "--backup",
        backup.to_str().unwrap(),
    ]);
    assert_eq!(outcome.code, 2);
    assert_eq!(fs::read(&edited).unwrap(), edited_bytes);
    assert!(!backup.exists(), "a pre-write refusal creates no backup");
    for name in ["command-only.sqlite", "foreign.sqlite", "edited.sqlite"] {
        fs::remove_file(dir.path(name)).unwrap();
    }
    assert_eq!(
        inventory(&dir),
        before,
        "the valid store and directory are untouched"
    );
    assert!(sidecars(&dir).is_empty());
}

#[test]
fn si_x02_key_file_errors_are_exit_4() {
    let dir = Dir::new("keyfile");
    let store = dir.path("store.sqlite");
    seed(&store, Some(KEY), 2);
    let before = inventory(&dir);
    for (name, bytes) in [("short", &KEY[..31]), ("long", &[0x5a; 33][..])] {
        let key = write_key(&dir, name, bytes);
        let outcome = tool(&[
            "--store",
            store.to_str().unwrap(),
            "--to",
            "v3",
            "--key-file",
            key.to_str().unwrap(),
            "--dry-run",
        ]);
        assert_eq!(outcome.code, 4, "{name}");
        assert_redacted(&outcome, &dir);
        fs::remove_file(key).unwrap();
    }
    let outcome = tool(&[
        "--store",
        store.to_str().unwrap(),
        "--to",
        "v3",
        "--key-file",
        dir.path("absent-key").to_str().unwrap(),
        "--dry-run",
    ]);
    assert_eq!(outcome.code, 4);
    assert_eq!(inventory(&dir), before);
}

#[test]
fn si_x05_wrong_key_is_redacted_exit_5_without_touching_the_store() {
    let dir = Dir::new("wrong-key");
    let store = dir.path("store.sqlite");
    seed(&store, Some(KEY), 2);
    let wrong = write_key(&dir, "wrong", &WRONG_KEY);
    let before = inventory(&dir);
    let backup = dir.path("backup.sqlite");
    let outcome = tool(&[
        "--store",
        store.to_str().unwrap(),
        "--to",
        "v3",
        "--key-file",
        wrong.to_str().unwrap(),
        "--backup",
        backup.to_str().unwrap(),
    ]);
    assert_eq!(outcome.code, 5);
    assert_eq!(outcome.stderr, "protected storage open failed\n");
    assert_eq!(outcome.stdout, "");
    assert_eq!(inventory(&dir), before);
    assert!(!backup.exists());
    // An unprotected opener against a protected file is an invalid source, not a key error.
    let outcome = tool(&[
        "--store",
        store.to_str().unwrap(),
        "--to",
        "v3",
        "--unprotected",
        "--dry-run",
    ]);
    assert_eq!(outcome.code, 2);
    assert_eq!(inventory(&dir), before);
}

#[test]
fn si_x06_existing_backup_is_exit_6_and_never_overwritten() {
    let dir = Dir::new("backup-exists");
    let store = dir.path("store.sqlite");
    seed(&store, None, 2);
    let backup = dir.path("backup.sqlite");
    fs::write(&backup, b"keep these exact bytes").unwrap();
    let before = inventory(&dir);
    let outcome = tool(&[
        "--store",
        store.to_str().unwrap(),
        "--to",
        "v3",
        "--unprotected",
        "--backup",
        backup.to_str().unwrap(),
    ]);
    assert_eq!(outcome.code, 6);
    assert_eq!(outcome.stderr, "backup already exists\n");
    assert_eq!(inventory(&dir), before);
    assert_format(&ledger(&store, None).1, 2);
}

#[test]
fn si_x06_backup_is_never_more_permissive_than_the_source_store() {
    use std::os::unix::fs::PermissionsExt;
    let dir = Dir::new("backup-mode");
    let store = dir.path("store.sqlite");
    seed(&store, None, 2);
    fs::set_permissions(&store, fs::Permissions::from_mode(0o600)).unwrap();
    let backup = dir.path("backup.sqlite");
    let outcome = tool(&[
        "--store",
        store.to_str().unwrap(),
        "--to",
        "v3",
        "--unprotected",
        "--backup",
        backup.to_str().unwrap(),
    ]);
    assert_eq!(outcome.code, 0, "{}", outcome.stderr);
    let mode = fs::metadata(&backup).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode & !0o600, 0, "backup mode {mode:o} exceeds source 600");
    assert_format(&ledger(&store, None).1, 3);
}

#[test]
fn si_x06_backup_path_naming_a_store_sidecar_is_refused_without_writing() {
    for suffix in ["-journal", "-wal", "-shm"] {
        let dir = Dir::new("backup-sidecar");
        let store = dir.path("store.sqlite");
        seed(&store, None, 2);
        let before = inventory(&dir);
        let backup = dir.path(&format!("store.sqlite{suffix}"));
        let outcome = tool(&[
            "--store",
            store.to_str().unwrap(),
            "--to",
            "v3",
            "--unprotected",
            "--backup",
            backup.to_str().unwrap(),
        ]);
        assert_eq!(outcome.code, 2, "{suffix}: {}", outcome.stdout);
        assert_eq!(
            outcome.stderr,
            "unsupported or invalid source or arguments\n"
        );
        assert_eq!(
            inventory(&dir),
            before,
            "{suffix}: nothing created or changed"
        );
        assert_format(&ledger(&store, None).1, 2);
    }
}

// ---- SI-X08: WAL sources never admit; keyed IOERR_LOCK is classified -------

fn wal_source(dir: &Dir, key: Option<[u8; 32]>, sidecars_present: bool) -> PathBuf {
    let store = dir.path(if sidecars_present {
        "wal-crashed.sqlite"
    } else {
        "wal-clean.sqlite"
    });
    seed(&store, key, 2);
    {
        let connection = raw(&store, key);
        let mode: String = connection
            .query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }
    if sidecars_present {
        ChildLease::start(dir, "wal-crash", &store, key).release();
        assert!(
            store.with_extension("sqlite-wal").exists()
                && store.with_extension("sqlite-shm").exists()
        );
    } else {
        assert!(!store.with_extension("sqlite-wal").exists());
    }
    store
}

#[test]
fn si_x08_persisted_wal_is_unsupported_and_wrong_key_is_classified_on_the_same_connection() {
    let dir = Dir::new("wal");
    let key = write_key(&dir, "key", &KEY);
    let wrong = write_key(&dir, "wrong", &WRONG_KEY);
    for present in [false, true] {
        let protected = wal_source(&dir, Some(KEY), present);
        let before = inventory(&dir);
        let backup = dir.path("never-created.sqlite");
        for (key_file, expected) in [(&key, 2), (&wrong, 5)] {
            let outcome = tool(&[
                "--store",
                protected.to_str().unwrap(),
                "--to",
                "v3",
                "--key-file",
                key_file.to_str().unwrap(),
                "--backup",
                backup.to_str().unwrap(),
            ]);
            assert_eq!(
                outcome.code, expected,
                "sidecars={present} stderr={}",
                outcome.stderr
            );
            assert_redacted(&outcome, &dir);
            assert!(
                !outcome.stderr.contains("HMAC") && !outcome.stderr.contains("page"),
                "integrity result text printed"
            );
            assert_eq!(
                inventory(&dir),
                before,
                "WAL/SHM and main bytes unchanged, no new file"
            );
        }
        fs::remove_file(&protected).unwrap();
        for sidecar in ["sqlite-wal", "sqlite-shm"] {
            let _ = fs::remove_file(protected.with_extension(sidecar));
        }
        let plain = wal_source(&dir, None, present);
        let before = inventory(&dir);
        let outcome = tool(&[
            "--store",
            plain.to_str().unwrap(),
            "--to",
            "v3",
            "--unprotected",
            "--backup",
            backup.to_str().unwrap(),
        ]);
        assert_eq!(outcome.code, 2, "unprotected WAL sidecars={present}");
        assert_eq!(inventory(&dir), before);
        fs::remove_file(&plain).unwrap();
        for sidecar in ["sqlite-wal", "sqlite-shm"] {
            let _ = fs::remove_file(plain.with_extension(sidecar));
        }
    }
    // Control: the ordinary rollback-journal source passes the same opener.
    let control = dir.path("control.sqlite");
    seed(&control, Some(KEY), 2);
    let outcome = tool(&[
        "--store",
        control.to_str().unwrap(),
        "--to",
        "v3",
        "--key-file",
        key.to_str().unwrap(),
        "--dry-run",
    ]);
    assert_eq!(outcome.code, 0, "{}", outcome.stderr);
}

/// A hot journal has its synced magic header; an unsynced one is ignorable.
fn assert_hot(journal: &Path) {
    let bytes = fs::read(journal).unwrap();
    assert_eq!(
        &bytes[..8],
        &[0xd9, 0xd5, 0x05, 0xf9, 0x20, 0xa1, 0x63, 0xd7],
        "journal header synced"
    );
}

// ---- SI-XL01 / SI-XL03 / SI-XL05 through the real binary -------------------

#[test]
fn si_xl01_reserved_writer_before_validation_is_busy_exit_3() {
    let dir = Dir::new("reserved");
    let store = dir.path("store.sqlite");
    seed(&store, Some(KEY), 2);
    let key = write_key(&dir, "key", &KEY);
    let before = inventory(&dir);
    let writer = ChildLease::start(&dir, "reserved", &store, Some(KEY));
    let backup = dir.path("backup.sqlite");
    let outcome = tool(&[
        "--store",
        store.to_str().unwrap(),
        "--to",
        "v3",
        "--key-file",
        key.to_str().unwrap(),
        "--backup",
        backup.to_str().unwrap(),
    ]);
    writer.release();
    assert_eq!(outcome.code, 3, "{}", outcome.stderr);
    assert_eq!(outcome.stderr, "store is in use\n");
    assert!(!backup.exists());
    assert_eq!(inventory(&dir), before);
}

#[test]
fn si_xl03_hot_journal_before_admission_is_recovery_required_exit_2_without_replay() {
    for key in [None, Some(KEY)] {
        let dir = Dir::new("hot-journal");
        let store = dir.path("store.sqlite");
        seed(&store, key, 2);
        ChildLease::start(&dir, "hot-journal", &store, key).release();
        let journal = store.with_extension("sqlite-journal");
        assert_hot(&journal);
        let before = inventory(&dir);
        let key_file = key.map(|_| write_key(&dir, "key", &KEY));
        let backup = dir.path("backup.sqlite");
        let mut args = vec!["--store", store.to_str().unwrap(), "--to", "v3"];
        let key_string;
        match &key_file {
            Some(path) => {
                key_string = path.to_string_lossy().into_owned();
                args.extend(["--key-file", key_string.as_str()]);
            }
            None => args.push("--unprotected"),
        }
        args.extend(["--backup", backup.to_str().unwrap()]);
        let outcome = tool(&args);
        assert_eq!(outcome.code, 2, "{}", outcome.stderr);
        if let Some(path) = &key_file {
            fs::remove_file(path).unwrap();
        }
        assert_eq!(
            inventory(&dir),
            before,
            "main and journal bytes unchanged, no replay"
        );
        assert!(!backup.exists());
    }
}

#[test]
fn si_xl05_live_owner_makes_the_converter_exit_3() {
    let dir = Dir::new("owner");
    let store = dir.path("store.sqlite");
    seed(&store, None, 2);
    let before = inventory(&dir);
    let owner = open_store(&store, None);
    let outcome = tool(&[
        "--store",
        store.to_str().unwrap(),
        "--to",
        "v3",
        "--unprotected",
        "--dry-run",
    ]);
    assert_eq!(outcome.code, 3);
    drop(owner);
    assert_eq!(inventory(&dir), before);
}

// ---- Library entry with deterministic lifecycle stages (test-hooks) --------

#[cfg(feature = "test-hooks")]
mod staged {
    use super::*;
    use hoyadb_storage_sqlite::convert::{
        ConversionMode, ConvertError, ConvertOptions, SourceProtection,
        testing::{Stage, run_with_observer},
    };

    fn options(store: &Path, backup: &Path) -> ConvertOptions {
        ConvertOptions {
            store: store.to_path_buf(),
            to: 3,
            protection: SourceProtection::Unprotected,
            mode: ConversionMode::Backup(backup.to_path_buf()),
        }
    }

    #[test]
    fn si_x03_backup_file_sync_then_directory_sync_precede_every_source_write() {
        let dir = Dir::new("stages");
        let store = dir.path("store.sqlite");
        seed(&store, None, 2);
        let main_before = fs::read(&store).unwrap();
        let backup = dir.path("backup.sqlite");
        let mut seen = vec![];
        let store_bytes_at_write = std::cell::RefCell::new(None);
        let report = run_with_observer(options(&store, &backup), &mut |stage| {
            seen.push(stage);
            if stage == Stage::BeforeSourceWrite {
                *store_bytes_at_write.borrow_mut() =
                    Some((fs::read(&store).unwrap(), fs::read(&backup).unwrap()));
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(
            (
                report.from,
                report.to,
                report.metadata_entries,
                report.packets_rewritten
            ),
            (2, 3, METADATA, METADATA + 1)
        );
        let position = |stage| seen.iter().position(|s| *s == stage).unwrap();
        assert!(position(Stage::ReadOnlyValidated) < position(Stage::ReadWriteReserved));
        assert!(position(Stage::ReadWriteReserved) < position(Stage::ReadOnlyClosed));
        assert!(position(Stage::BackupCreated) < position(Stage::BackupFileSynced));
        assert!(position(Stage::BackupFileSynced) < position(Stage::BackupVerified));
        assert!(position(Stage::BackupVerified) < position(Stage::BackupDirectorySynced));
        assert!(position(Stage::BackupDirectorySynced) < position(Stage::BeforeSourceWrite));
        assert!(position(Stage::BeforeSourceWrite) < position(Stage::BeforeCommit));
        assert!(!seen.contains(&Stage::BeforeCleanup));
        let (store_at_write, backup_at_write) = store_bytes_at_write.borrow().clone().unwrap();
        assert_eq!(
            store_at_write, main_before,
            "no source byte changed before the backup was durable"
        );
        assert_eq!(backup_at_write, main_before);
        assert_format(&ledger(&store, None).1, 3);
        assert!(sidecars(&dir).is_empty());
    }

    #[test]
    fn si_x07_backup_sync_failures_are_exit_7_and_clean_only_this_backup() {
        for failing in [
            Stage::BeforeBackupFileSync,
            Stage::BeforeBackupDirectorySync,
        ] {
            let dir = Dir::new("backup-fault");
            let store = dir.path("store.sqlite");
            seed(&store, None, 2);
            let before = inventory(&dir);
            let backup = dir.path("backup.sqlite");
            let mut writes = 0;
            let error = run_with_observer(options(&store, &backup), &mut |stage| {
                if stage == Stage::BeforeSourceWrite {
                    writes += 1;
                }
                if stage == failing {
                    Err(std::io::Error::other("injected sync failure"))
                } else {
                    Ok(())
                }
            })
            .err()
            .unwrap();
            assert_eq!(
                error,
                ConvertError::Failed {
                    incomplete_backup: false
                },
                "{failing:?}"
            );
            assert_eq!(error.exit_code(), 7);
            assert_eq!(writes, 0, "no conversion after a backup failure");
            assert_eq!(
                inventory(&dir),
                before,
                "incomplete backup removed, source exact, no sidecar"
            );
            assert_format(&ledger(&store, None).1, 2);
        }
    }

    #[test]
    fn si_x07_swapped_backup_path_is_preserved_and_disclosed() {
        let dir = Dir::new("backup-swap");
        let store = dir.path("store.sqlite");
        seed(&store, None, 2);
        let backup = dir.path("backup.sqlite");
        let decoy = dir.path("decoy");
        fs::write(&decoy, b"someone else's file").unwrap();
        let error = run_with_observer(options(&store, &backup), &mut |stage| match stage {
            Stage::BeforeBackupDirectorySync => Err(std::io::Error::other("injected")),
            Stage::BeforeCleanup => {
                fs::rename(&decoy, &backup).unwrap();
                Ok(())
            }
            _ => Ok(()),
        })
        .err()
        .unwrap();
        assert_eq!(
            error,
            ConvertError::Failed {
                incomplete_backup: true
            }
        );
        assert_eq!(
            fs::read(&backup).unwrap(),
            b"someone else's file",
            "swapped-in file never deleted"
        );
        assert!(!decoy.exists());
        assert_format(&ledger(&store, None).1, 2);
    }

    #[test]
    fn si_xl02_writer_taking_reserved_during_handoff_is_busy_without_recovery() {
        let dir = Dir::new("handoff");
        let store = dir.path("store.sqlite");
        seed(&store, None, 2);
        let before = inventory(&dir);
        let backup = dir.path("backup.sqlite");
        let mut lease = None;
        let error = run_with_observer(options(&store, &backup), &mut |stage| {
            if stage == Stage::ReadOnlyValidated {
                lease = Some(ChildLease::start(&dir, "reserved", &store, None));
            }
            Ok(())
        })
        .err()
        .unwrap();
        lease.take().unwrap().release();
        assert_eq!(error, ConvertError::StoreInUse);
        assert!(!backup.exists());
        assert_eq!(inventory(&dir), before);
    }

    /// While the converter holds SHARED no SQLite writer can sync a journal,
    /// so publication is simulated by an external actor copying a genuine hot
    /// journal (spilled from a donor store) beside the source at the handoff.
    #[test]
    fn si_xl03_hot_journal_published_during_handoff_is_busy_without_replay() {
        let dir = Dir::new("handoff-journal");
        let store = dir.path("store.sqlite");
        seed(&store, None, 2);
        let donor = dir.path("donor.sqlite");
        seed(&donor, None, 2);
        ChildLease::start(&dir, "hot-journal", &donor, None).release();
        let donor_journal = donor.with_extension("sqlite-journal");
        assert_hot(&donor_journal);
        let backup = dir.path("backup.sqlite");
        let journal = store.with_extension("sqlite-journal");
        let mut journal_bytes = None;
        let error = run_with_observer(options(&store, &backup), &mut |stage| {
            if stage == Stage::ReadOnlyValidated {
                fs::copy(&donor_journal, &journal).unwrap();
                journal_bytes = Some(fs::read(&journal).unwrap());
            }
            Ok(())
        })
        .err()
        .unwrap();
        assert_eq!(
            error,
            ConvertError::StoreInUse,
            "recovery needs EXCLUSIVE, which our SHARED lease denies"
        );
        assert_eq!(
            fs::read(&journal).unwrap(),
            journal_bytes.unwrap(),
            "journal not replayed or removed"
        );
        assert!(!backup.exists());
    }

    #[test]
    fn si_xl04_late_reader_makes_commit_fail_exit_7_with_rollback_and_retained_backup() {
        let dir = Dir::new("late-reader");
        let store = dir.path("store.sqlite");
        seed(&store, None, 2);
        let (source_rows, source_ledger) = ledger(&store, None);
        let main_before = fs::read(&store).unwrap();
        let dumps_before = dumps(&store, None);
        let backup = dir.path("backup.sqlite");
        let mut reader = None;
        let error = run_with_observer(options(&store, &backup), &mut |stage| {
            if stage == Stage::BeforeCommit {
                reader = Some(ChildLease::start(&dir, "shared", &store, None));
            }
            Ok(())
        })
        .err()
        .unwrap();
        reader.take().unwrap().release();
        assert_eq!(
            error,
            ConvertError::Failed {
                incomplete_backup: false
            }
        );
        assert_eq!(
            fs::read(&backup).unwrap(),
            main_before,
            "verified backup retained"
        );
        let (rows, restored) = ledger(&store, None);
        assert_eq!(
            restored, source_ledger,
            "exact logical source state after rollback"
        );
        assert_eq!(rows.len(), source_rows.len());
        assert_eq!(dumps(&store, None), dumps_before);
        assert!(sidecars(&dir).is_empty());
    }

    #[test]
    fn si_xl05_no_production_owner_enters_during_the_handoff() {
        let dir = Dir::new("barriers");
        let store = dir.path("store.sqlite");
        seed(&store, None, 2);
        let backup = dir.path("backup.sqlite");
        let mut attempts = vec![];
        run_with_observer(options(&store, &backup), &mut |stage| {
            if matches!(
                stage,
                Stage::ReadOnlyValidated
                    | Stage::ReadWriteReserved
                    | Stage::ReadOnlyClosed
                    | Stage::BeforeSourceWrite
                    | Stage::BeforeCommit
            ) {
                let result = SqliteStorage::open_unprotected(&store).map(|_| ());
                attempts.push((stage, matches!(result, Err(StorageError::StoreInUse))));
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(attempts.len(), 5);
        assert!(attempts.iter().all(|(_, in_use)| *in_use), "{attempts:?}");
        assert_format(&ledger(&store, None).1, 3);
    }

    /// The in-process probes above share SQLite's lock table with the converter.
    /// Only another process sees whether the kernel still holds its RESERVED lock
    /// once the backup source has been read.
    #[test]
    fn si_xl05_no_other_process_takes_the_write_lease_after_the_backup_copy() {
        let dir = Dir::new("lease-after-backup");
        let store = dir.path("store.sqlite");
        seed(&store, None, 2);
        let backup = dir.path("backup.sqlite");
        let mut probes = vec![];
        run_with_observer(options(&store, &backup), &mut |stage| {
            if matches!(stage, Stage::BeforeSourceWrite | Stage::BeforeCommit) {
                probes.push((stage, probe_writer(&dir, &store)));
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(
            probes,
            [
                (Stage::BeforeSourceWrite, "busy".to_owned()),
                (Stage::BeforeCommit, "busy".to_owned())
            ]
        );
        assert_format(&ledger(&store, None).1, 3);
    }

    #[test]
    fn si_x06_existing_backup_refused_before_write() {
        let dir = Dir::new("lib-backup-exists");
        let store = dir.path("store.sqlite");
        seed(&store, None, 2);
        let backup = dir.path("backup.sqlite");
        fs::write(&backup, b"existing").unwrap();
        let mut seen = vec![];
        let error = run_with_observer(options(&store, &backup), &mut |stage| {
            seen.push(stage);
            Ok(())
        })
        .err()
        .unwrap();
        assert_eq!(error, ConvertError::BackupExists);
        assert_eq!(seen, [Stage::ReadOnlyValidated]);
        assert_eq!(fs::read(&backup).unwrap(), b"existing");
    }
}
