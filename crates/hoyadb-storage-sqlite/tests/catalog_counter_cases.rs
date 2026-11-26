//! TC-0041 SI-W03 (catalog line): strict SQLite open and catalog count all three catalog
//! consistency statements at N=10 and N=1000 against a direct statement-status oracle, with the
//! adapter's already measured work (B) accounted for.
//!
//! The fixture is a closed, managed store with N rows, so the orphan, unknown-reserved and
//! row-version checks all run. The oracle replays the three statements' exact SQL and parameters
//! twice on a separate connection to the same closed file; the adapter's exclusive lease means
//! it never runs while the adapter is open.
#![cfg(not(target_arch = "wasm32"))]
use hoyadb_core::{
    model::{Database, RESERVED_PREFIX, ROW_VERSION, SchemaDefinition, TableDefinition},
    storage::{Column, ColumnType, MODEL_TABLE, SYNC_TABLE, SchemaStorage, Value, counters},
};
use hoyadb_storage_sqlite::SqliteStorage;
use rusqlite::{Connection, StatementStatus, params};
use std::{
    collections::BTreeMap,
    future::Future,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
};

fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("synchronous adapter unexpectedly suspended"),
    }
}

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "hoya-catalog-counters-{}-{}",
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
        std::fs::remove_dir_all(&self.0).expect("remove only the owned catalog fixture");
    }
}

fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([(
            "items".into(),
            TableDefinition {
                columns: BTreeMap::from([(
                    "note".into(),
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

/// A closed managed store at `path` holding exactly `n` rows.
fn fixture(path: &Path, n: usize) {
    let mut db = run(Database::open(
        SqliteStorage::open_unprotected(path).unwrap(),
        schema(),
        vec![],
    ))
    .unwrap();
    for index in 0..n {
        run(db.insert(
            "items",
            BTreeMap::from([("note".into(), Value::String(format!("row {index}")))]),
        ))
        .unwrap();
    }
    drop(db.into_storage());
}

/// (fullscan steps, VM steps) of one statement, read as the adapter's `measure` reads them.
type Work = (u64, u64);

const ORPHAN: &str = "SELECT EXISTS(SELECT 1 FROM __hoya_rows r LEFT JOIN __hoya_tables t ON t.name=r.table_name WHERE t.name IS NULL) OR EXISTS(SELECT 1 FROM __hoya_fields f LEFT JOIN __hoya_rows r ON r.table_name=f.table_name AND r.id=f.row_id WHERE r.id IS NULL)";
const UNKNOWN: &str = "SELECT EXISTS(SELECT 1 FROM __hoya_fields WHERE table_name<>?1 AND table_name<>?2 AND name GLOB ?3 AND (?4 OR name<>?5))";
const INVALID: &str = "SELECT EXISTS(SELECT 1 FROM __hoya_rows r WHERE r.table_name<>?1 AND r.table_name<>?2 AND NOT EXISTS(SELECT 1 FROM __hoya_fields f WHERE f.table_name=r.table_name AND f.row_id=r.id AND f.name=?3 AND f.kind=2 AND f.text_value IS NULL AND typeof(f.int_value)='integer' AND f.int_value=?4))";

fn statement_work(connection: &Connection, sql: &str, parameters: impl rusqlite::Params) -> Work {
    let mut statement = connection.prepare(sql).unwrap();
    let found: bool = statement.query_row(parameters, |row| row.get(0)).unwrap();
    assert!(!found, "the consistent fixture passes {sql}");
    let read = |status| u64::try_from(statement.get_status(status)).unwrap();
    (
        read(StatementStatus::FullscanStep),
        read(StatementStatus::VmStep),
    )
}

/// The oracle: each statement's work on a separate connection to the closed fixture, in the
/// adapter's order, with the parameters the adapter binds for a managed store (version 1).
fn oracle(path: &Path) -> [Work; 3] {
    let connection = Connection::open(path).unwrap();
    [
        statement_work(&connection, ORPHAN, []),
        statement_work(
            &connection,
            UNKNOWN,
            params![
                MODEL_TABLE,
                SYNC_TABLE,
                format!("{RESERVED_PREFIX}*"),
                false,
                ROW_VERSION
            ],
        ),
        statement_work(
            &connection,
            INVALID,
            params![MODEL_TABLE, SYNC_TABLE, ROW_VERSION, 1_i64],
        ),
    ]
}

/// The whole catalog operation on the reopened adapter, captured immediately.
fn whole(path: &Path) -> counters::WorkSnapshot {
    let store = SqliteStorage::open_unprotected(path).unwrap();
    counters::reset();
    let catalog = run(store.catalog()).unwrap();
    let work = counters::snapshot();
    assert_eq!(
        catalog.state.map(|state| state.version),
        Some(1),
        "managed store"
    );
    work
}

/// Records B on unchanged code and proves the whole operation and the oracle repeat exactly.
#[test]
fn si_w03_catalog_work_and_oracle_are_deterministic() {
    let directory = Directory::new();
    for n in [10, 1000] {
        let path = directory.path(&format!("catalog-{n}.sqlite"));
        fixture(&path, n);
        let first = oracle(&path);
        let second = oracle(&path);
        assert_eq!(first, second, "N={n}: the oracle repeats");
        let a = whole(&path);
        let b = whole(&path);
        eprintln!("catalog N={n} oracle {first:?} whole {a:?}");
        assert_eq!(a, b, "N={n}: the whole operation repeats");
    }
}

/// B(N): the whole catalog operation's measured (fullscan, VM) work on UNCHANGED code, recorded
/// by `si_w03_catalog_work_and_oracle_are_deterministic` in the baseline probe
/// (logs/baseline-probe2.txt sha256 6717c141…, source manifest probe2-input.sha256 a26d7604…). It is
/// the already measured model-row read and does not depend on N.
fn baseline(n: usize) -> Work {
    match n {
        10 | 1000 => (0, 111),
        _ => unreachable!("B is recorded for N=10 and N=1000 only"),
    }
}

/// The whole operation equals B plus the oracle's three statements, exactly, at both N.
#[test]
fn si_w03_catalog_counts_all_three_consistency_statements() {
    let directory = Directory::new();
    let mut totals = vec![];
    for n in [10, 1000] {
        let path = directory.path(&format!("catalog-{n}.sqlite"));
        fixture(&path, n);
        let statements = oracle(&path);
        assert_eq!(statements, oracle(&path), "N={n}: the oracle repeats");
        for (index, (fullscan, vm)) in statements.iter().enumerate() {
            assert!(
                *fullscan > 0 && *vm > 0,
                "N={n}: statement {index} does work"
            );
        }
        let o = statements
            .iter()
            .fold((0, 0), |(f, v), (fullscan, vm)| (f + fullscan, v + vm));
        let work = whole(&path);
        let measured = (work.sqlite_fullscan_steps, work.sqlite_vm_steps);
        let (b_fullscan, b_vm) = baseline(n);
        eprintln!(
            "catalog N={n} B {:?} O {statements:?} whole {measured:?}",
            baseline(n)
        );
        assert_eq!(
            measured,
            (b_fullscan + o.0, b_vm + o.1),
            "N={n}: catalog work must equal B + O"
        );
        totals.push(o);
    }
    assert!(
        totals[1].0 > totals[0].0 && totals[1].1 > totals[0].1,
        "O(1000) > O(10): {totals:?}"
    );
}
