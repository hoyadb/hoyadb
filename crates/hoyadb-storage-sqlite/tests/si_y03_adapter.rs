#[path = "../../../tests/conformance/si_y03_adapter_characterization.rs"]
mod cases;
use hoyadb_core::storage::StoreOwnership;
use hoyadb_storage_sqlite::SqliteStorage;
use std::sync::atomic::{AtomicUsize, Ordering};
fn in_directory(name: &str, case: impl FnOnce(&dyn Fn() -> SqliteStorage)) {
    let directory =
        std::env::temp_dir().join(format!("hoyadb-si-y03-{name}-{}", std::process::id()));
    std::fs::create_dir(&directory).expect("fresh owned directory must not already exist");
    let next = AtomicUsize::new(0);
    let fresh = || {
        SqliteStorage::open_unprotected(
            directory.join(format!("{}.sqlite", next.fetch_add(1, Ordering::Relaxed))),
        )
        .unwrap()
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| case(&fresh)));
    std::fs::remove_dir_all(&directory).unwrap();
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}
#[test]
fn untouched_exclusive() {
    in_directory("untouched-exclusive", |fresh| {
        cases::untouched_exclusive(fresh)
    });
}
#[test]
fn untouched_unproven() {
    in_directory("untouched-unproven", |fresh| {
        cases::untouched_unproven(fresh)
    });
}
#[test]
fn guarded_exclusive() {
    in_directory("guarded-exclusive", |fresh| {
        cases::guarded_races(fresh, StoreOwnership::Exclusive)
    });
}
#[test]
fn guarded_unproven() {
    in_directory("guarded-unproven", |fresh| {
        cases::guarded_races(fresh, StoreOwnership::Unproven)
    });
}

#[test]
fn default_delta_matches_override() {
    in_directory("default-match", |fresh| {
        cases::default_delta_matches_override(fresh)
    });
}
#[test]
fn default_delta_rejects_valid_untouched_race() {
    in_directory("default-race", |fresh| {
        cases::default_delta_rejects_valid_untouched_race(fresh)
    });
}
