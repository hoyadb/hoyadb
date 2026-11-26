//! SPEC-0049 / TC-0053: native on-disk reopen and both atomic commit paths.
#[path = "../../../tests/conformance/durable_aliases.rs"]
mod cases;
use cases::*;
use hoyadb_core::storage::*;
use hoyadb_storage_sqlite::SqliteStorage;

fn fixture(test: impl FnOnce(&std::path::Path)) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "hoya-durable-alias-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).unwrap();
    test(&directory);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn d01_native_restart_retains_alias_after_forget_or_history_eviction() {
    fixture(|directory| {
        for history in [0, 100] {
            let path = directory.join(format!("history-{history}.sqlite"));
            let mut db = open(SqliteStorage::open_unprotected(&path).unwrap(), history);
            configure_durable(&mut db);
            let (original, key) = accepted_create(&mut db);
            if history != 0 {
                db.forget(&key).unwrap();
            }
            drop(db.into_storage());
            let mut reopened = open(SqliteStorage::open_unprotected(&path).unwrap(), history);
            configure_durable(&mut reopened);
            assert_eq!(
                reopened.resolve_entity_id("notes", &original).unwrap(),
                "server-note"
            );
            assert_eq!(
                run(reopened.query("notes").fetch(&reopened)).unwrap()[0].id,
                "server-note"
            );
        }
    });
}

#[test]
fn d05_native_full_and_delta_failure_keep_aliases_atomic() {
    fixture(|directory| {
        for (name, ownership) in [
            ("delta", StoreOwnership::Exclusive),
            ("full", StoreOwnership::Unproven),
        ] {
            failed_reconcile_is_atomic(
                SqliteStorage::open_unprotected(directory.join(format!("{name}.sqlite"))).unwrap(),
                ownership,
            );
        }
    });
}

#[test]
fn d03_native_ordinary_configuration_rejects_retained_durable_mode() {
    fixture(|directory| {
        ordinary_configuration_rejects_retained_durable_mode(
            SqliteStorage::open_unprotected(directory.join("mode.sqlite")).unwrap(),
        );
    });
}

#[test]
fn d02_native_reference_cells_resolve_through_their_target_table() {
    fixture(|directory| {
        reference_cells_resolve_through_their_target_table(
            SqliteStorage::open_unprotected(directory.join("reference.sqlite")).unwrap(),
        );
    });
}

#[test]
fn d04_native_adoption_refuses_reserved_local_rows() {
    fixture(|directory| {
        adoption_refuses_reserved_local_rows(
            SqliteStorage::open_unprotected(directory.join("reserved.sqlite")).unwrap(),
        );
    });
}

#[cfg(feature = "ledger-convert-tool")]
#[test]
fn d03_offline_converter_rejects_format_four_without_backup_or_write() {
    use hoyadb_storage_sqlite::convert::{
        ConversionMode, ConvertOptions, SourceProtection, run as convert,
    };
    fixture(|directory| {
        let path = directory.join("source.sqlite");
        let mut db = open(SqliteStorage::open_unprotected(&path).unwrap(), 0);
        configure_durable(&mut db);
        accepted_create(&mut db);
        drop(db.into_storage());
        let before = std::fs::read(&path).unwrap();
        for to in [2, 3, 4] {
            let backup = directory.join(format!("backup-{to}.sqlite"));
            assert!(
                convert(ConvertOptions {
                    store: path.clone(),
                    to,
                    protection: SourceProtection::Unprotected,
                    mode: ConversionMode::Backup(backup.clone()),
                })
                .is_err()
            );
            assert!(!backup.exists());
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
    });
}
