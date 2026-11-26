//! TC-0041 SI-O03-SI-O05 on native SQLite.
#![cfg(not(target_arch = "wasm32"))]
#[path = "../../../tests/conformance/entity_scaling.rs"]
mod cases;
use hoyadb_core::command::CommandError;
use hoyadb_storage_sqlite::SqliteStorage;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "hoya-entity-scaling-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn store(&self, name: &str) -> SqliteStorage {
        SqliteStorage::open_unprotected(self.path(name)).unwrap()
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("remove only the owned scaling fixture");
    }
}

#[test]
fn si_o03_entity_staging_is_equal_at_n_without_inventory_validation() {
    let directory = Directory::new();
    let small = cases::entity_staging_work(directory.store("o03-10.sqlite"), 10);
    let large = cases::entity_staging_work(directory.store("o03-1000.sqlite"), 1000);
    assert!(
        small.iter().all(|(_, inventory)| *inventory == 0),
        "{small:?}"
    );
    assert_eq!(small, large, "work depends on N");
}

#[test]
fn si_o04_reconfigure_emptiness_check_is_equal_at_n() {
    let directory = Directory::new();
    let small = cases::reconfigure_emptiness_work(
        directory.store("o04-10.sqlite"),
        directory.store("o04-empty-10.sqlite"),
        10,
    );
    let large = cases::reconfigure_emptiness_work(
        directory.store("o04-1000.sqlite"),
        directory.store("o04-empty-1000.sqlite"),
        1000,
    );
    assert_eq!(small.0, Err(CommandError::Protocol));
    assert_eq!(small.2, Ok(()));
    assert_eq!(small.1, large.1, "work depends on N");
    assert_eq!((small.0, small.2), (large.0, large.2));
}

#[test]
fn si_o05_merge_prepare_is_equal_at_n() {
    let directory = Directory::new();
    let mut work = Vec::new();
    for n in [10, 1000] {
        let (mut db, _) = cases::configured(directory.store(&format!("o05-{n}.sqlite")), n);
        let (session, snapshot) = cases::merge_prepare_work(&mut db);
        cases::run(db.merge_commit(session)).unwrap();
        work.push(snapshot);
    }
    assert_eq!(work[0].sqlite_fullscan_steps, 0, "{work:?}");
    assert_eq!(work[0], work[1], "work depends on N: {work:?}");
}

/// SI-O05: a test-hook change to a touched row between prepare and commit
/// still rejects StalePull (the trigger fires on an unrelated owner write).
#[test]
fn si_o05_touched_row_change_rejects_stale_pull() {
    let directory = Directory::new();
    let path = directory.path("stale.sqlite");
    let (db, filler) = cases::configured(SqliteStorage::open_unprotected(&path).unwrap(), 3);
    drop(db);
    // SPEC-0037 R2.3: installed while closed; fires only while armed.
    rusqlite::Connection::open(&path).unwrap().execute_batch("CREATE TRIGGER touch_record AFTER INSERT ON __hoya_fields WHEN NEW.table_name='local' AND hoya_test_fault('touch_record') BEGIN UPDATE __hoya_fields SET text_value='tampered' WHERE table_name='items' AND row_id='record' AND name='body'; END").unwrap();
    let mut db = cases::open(SqliteStorage::open_unprotected(&path).unwrap());
    let (session, _) = cases::merge_prepare_work(&mut db);
    let fault = hoyadb_storage_sqlite::test_hooks::arm(&path, "touch_record");
    cases::run(db.replace(
        "local",
        &filler[0],
        BTreeMap::from([(
            "note".into(),
            hoyadb_core::storage::Value::String("edited".into()),
        )]),
    ))
    .unwrap();
    drop(fault);
    assert_eq!(
        cases::run(db.merge_commit(session)),
        Err(CommandError::StalePull)
    );
}
