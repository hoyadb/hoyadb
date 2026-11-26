//! TC-0041 SI-L01–L08 and SI-A01–A07: exclusive native ownership and admission.
#![cfg(not(target_arch = "wasm32"))]
#[path = "../../../tests/conformance/ownership.rs"]
mod cases;
use hoyadb_core::storage::{Row, Storage, StorageError, StoreOwnership, SyncStorage, Value};
use hoyadb_storage_sqlite::{EncryptionKey, SqliteOpenOptions, SqliteStorage};
use rusqlite::{Connection, ErrorCode};
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

const KEY: [u8; 32] = [0x3c; 32];
const WRONG: [u8; 32] = [0x3d; 32];
const CANARY: &str = "synthetic-ownership-canary-東京-🌿";

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "hoya-ownership-{}-{}",
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
        std::fs::remove_dir_all(&self.0).expect("remove only the owned ownership fixture");
    }
}

fn protected(path: &Path, key: [u8; 32]) -> Result<SqliteStorage, StorageError> {
    SqliteStorage::open_with_options(
        path,
        SqliteOpenOptions {
            encryption_key: Some(EncryptionKey::from_bytes(key)),
        },
    )
}

fn row(id: &str, text: &str) -> Row {
    Row {
        id: id.into(),
        fields: BTreeMap::from([("text".into(), Value::String(text.into()))]),
    }
}

fn seed(store: &mut SqliteStorage) {
    cases::run(store.create_table("notes")).unwrap();
    cases::run(store.insert("notes", row("seed", CANARY))).unwrap();
}

fn store_in_use(result: Result<SqliteStorage, StorageError>) {
    assert!(
        matches!(result, Err(StorageError::StoreInUse)),
        "expected StoreInUse, got {:?}",
        result.map(|_| "an opened second owner")
    );
}

fn redacted(result: Result<SqliteStorage, StorageError>) {
    assert!(
        matches!(
            &result,
            Err(StorageError::BackendFailure { message }) if message == "protected storage open failed"
        ),
        "expected the redacted protected-open error, got {:?}",
        result.map(|_| "an opened store")
    );
}

fn still_owns(store: &mut SqliteStorage, id: &str) {
    assert_eq!(
        cases::run(store.get("notes", "seed")).unwrap(),
        Some(row("seed", CANARY))
    );
    cases::run(store.insert("notes", row(id, "after"))).unwrap();
    assert_eq!(
        cases::run(store.get("notes", id)).unwrap(),
        Some(row(id, "after"))
    );
}

fn busy(result: rusqlite::Result<impl Sized>) {
    let code = result.err().and_then(|error| error.sqlite_error_code());
    assert_eq!(code, Some(ErrorCode::DatabaseBusy));
}

#[test]
fn si_l01_second_unprotected_open_is_store_in_use() {
    let directory = Directory::new();
    let path = directory.path("owned.sqlite");
    let mut owner = SqliteStorage::open_unprotected(&path).unwrap();
    seed(&mut owner);
    store_in_use(SqliteStorage::open_unprotected(&path));
    still_owns(&mut owner, "l01");
}

#[test]
fn si_l02a_protected_second_open_with_the_correct_key_is_store_in_use() {
    let directory = Directory::new();
    let path = directory.path("owned.sqlite");
    let mut owner = protected(&path, KEY).unwrap();
    seed(&mut owner);
    store_in_use(protected(&path, KEY));
    still_owns(&mut owner, "l02a");
}

#[test]
fn si_l02b_wrong_key_is_store_in_use_only_while_owned() {
    let directory = Directory::new();
    let path = directory.path("owned.sqlite");
    let mut owner = protected(&path, KEY).unwrap();
    seed(&mut owner);
    store_in_use(protected(&path, WRONG));
    still_owns(&mut owner, "l02b");
    let unowned = directory.path("unowned.sqlite");
    seed(&mut protected(&unowned, KEY).unwrap());
    let before = std::fs::read(&unowned).unwrap();
    redacted(protected(&unowned, WRONG));
    assert_eq!(std::fs::read(&unowned).unwrap(), before);
    for suffix in ["-journal", "-wal", "-shm"] {
        assert!(!directory.path(&format!("unowned.sqlite{suffix}")).exists());
    }
}

#[test]
fn si_l05_external_raw_connection_is_excluded() {
    let directory = Directory::new();
    let path = directory.path("owned.sqlite");
    let mut owner = SqliteStorage::open_unprotected(&path).unwrap();
    seed(&mut owner);
    let raw = Connection::open(&path).unwrap();
    busy(raw.execute_batch("BEGIN IMMEDIATE"));
    busy(
        raw.query_row("SELECT count(*) FROM __hoya_rows", [], |row| {
            row.get::<_, i64>(0)
        }),
    );
    drop(raw);
    still_owns(&mut owner, "l05");
}

#[test]
fn si_l07_close_and_reopen_release_the_lease() {
    let directory = Directory::new();
    let path = directory.path("owned.sqlite");
    let mut owner = SqliteStorage::open_unprotected(&path).unwrap();
    seed(&mut owner);
    drop(owner);
    let mut reopened = SqliteStorage::open_unprotected(&path).unwrap();
    still_owns(&mut reopened, "l07");
    drop(reopened);
    let protected_path = directory.path("protected.sqlite");
    seed(&mut protected(&protected_path, KEY).unwrap());
    still_owns(&mut protected(&protected_path, KEY).unwrap(), "l07");
}

#[test]
fn si_l08_exclusive_mode_journal_holds_no_plaintext() {
    let directory = Directory::new();
    let path = directory.path("protected.sqlite");
    let journal = directory.path("protected.sqlite-journal");
    let mut owner = protected(&path, KEY).unwrap();
    seed(&mut owner);
    cases::run(owner.replace("notes", row("seed", CANARY))).unwrap();
    cases::run(owner.insert("notes", row("second", CANARY))).unwrap();
    let owned = journal.exists();
    println!("SI-L08 evidence: -journal present while owned: {owned}");
    if owned {
        let bytes = std::fs::read(&journal).unwrap();
        assert!(!bytes.windows(CANARY.len()).any(|w| w == CANARY.as_bytes()));
    }
    // While owned no other connection may read it, so the store must be exclusive.
    busy(Connection::open(&path).unwrap().query_row(
        "SELECT count(*) FROM sqlite_schema",
        [],
        |row| row.get::<_, i64>(0),
    ));
    drop(owner);
    println!(
        "SI-L08 evidence: -journal present after close: {}",
        journal.exists()
    );
    let reopened = protected(&path, KEY).unwrap();
    assert_eq!(
        cases::run(reopened.get("notes", "seed")).unwrap(),
        Some(row("seed", CANARY))
    );
    assert_eq!(
        cases::run(reopened.get("notes", "second")).unwrap(),
        Some(row("second", CANARY))
    );
    let main = std::fs::read(&path).unwrap();
    assert!(!main.windows(CANARY.len()).any(|w| w == CANARY.as_bytes()));
}

#[test]
fn si_l03_second_process_is_store_in_use() {
    let directory = Directory::new();
    let path = directory.path("child.sqlite");
    let mut child = Command::new(env!("CARGO_BIN_EXE_restart_fixture"))
        .args(["hold", "ownership"])
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut barrier = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut barrier)
        .unwrap();
    assert_eq!(barrier, "held\n");
    let parent = SqliteStorage::open_unprotected(&path);
    let in_use = matches!(parent, Err(StorageError::StoreInUse));
    drop(parent);
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(in_use, "the parent must not own a store its child holds");
    let reopened = SqliteStorage::open_unprotected(&path).unwrap();
    assert_eq!(
        cases::run(reopened.get("notes", "held")).unwrap(),
        Some(row("held", CANARY))
    );
}

fn fresh(directory: &Directory) -> impl FnMut() -> SqliteStorage + '_ {
    let mut sequence = 0;
    move || {
        sequence += 1;
        SqliteStorage::open_unprotected(directory.path(&format!("admission-{sequence}.sqlite")))
            .unwrap()
    }
}

#[test]
fn si_a01_queued_request_does_not_block() {
    let directory = Directory::new();
    cases::queued_request_does_not_block(fresh(&directory));
}

#[test]
fn si_a02_active_handler_does_not_block() {
    let directory = Directory::new();
    cases::active_handler_does_not_block(fresh(&directory));
}

#[test]
fn si_a03_same_database_serialization() {
    let directory = Directory::new();
    cases::same_database_serialization(fresh(&directory)());
}

#[test]
fn si_a04_a05_tickets_stay_with_their_owner() {
    let directory = Directory::new();
    cases::tickets_stay_with_their_owner(fresh(&directory));
}

#[test]
fn si_a07_native_sqlite_reports_exclusive() {
    let directory = Directory::new();
    assert_eq!(fresh(&directory)().ownership(), StoreOwnership::Exclusive);
}
