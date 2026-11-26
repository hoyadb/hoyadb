//! TC-0042 C4 (O2) on SQLite. Every case closes its owner and reopens the file through a fresh
//! connection, which must read the identical ledger rows; EO-ORDER and EO17 also restart through
//! a fresh connection after C4 and before the later rollback. Exclusive adapter: the default parallel
//! runner applies.
#[path = "../../../tests/conformance/entity_overlap.rs"]
mod cases;
use hoyadb_core::storage::SyncStorage;
use hoyadb_storage_sqlite::SqliteStorage;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn tc0044_cp03_c4_precedes_discard() {
    on_sqlite(cases::cp03_c4_before_policy);
}

/// Runs a case on a store in a directory this run creates (creation fails if the path exists);
/// only that directory is removed afterwards, also when the case panics. Then restarts.
fn on_sqlite(case: impl FnOnce(SqliteStorage) -> cases::Outcome<SqliteStorage>) {
    on_sqlite_reopening(|storage, _| case(storage));
}

/// As `on_sqlite`, and the case also receives a mid-case restart: it closes the given storage
/// (dropping its connection) and opens the same file through a fresh connection.
fn on_sqlite_reopening(
    case: impl FnOnce(
        SqliteStorage,
        &dyn Fn(SqliteStorage) -> SqliteStorage,
    ) -> cases::Outcome<SqliteStorage>,
) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "hoya-c4-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).expect("a new directory owned by this run");
    let path = directory.join("store.sqlite");
    let reopen = |storage: SqliteStorage| {
        drop(storage);
        SqliteStorage::open_unprotected(&path).unwrap()
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (storage, rows) = case(SqliteStorage::open_unprotected(&path).unwrap(), &reopen);
        drop(storage);
        let reopened = SqliteStorage::open_unprotected(&path).unwrap();
        assert_eq!(
            reopened.load_sync().unwrap(),
            Some(rows),
            "ledger identical after restart"
        );
    }));
    std::fs::remove_dir_all(&directory).unwrap();
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

#[test]
fn tc0042_c4_eo2_correction_rebases_a_winning_pending_writer() {
    on_sqlite(cases::eo2);
}

#[test]
fn tc0042_c4_eo_order_promotion_only_into_eff0() {
    on_sqlite_reopening(|s, reopen| cases::eo_order(s, reopen));
}

#[test]
fn tc0042_c4_eo4_two_pending_writers_win() {
    on_sqlite(cases::eo4_win);
}

#[test]
fn tc0042_c4_eo5_rollback_rebases_pending_writer_on_other_field() {
    on_sqlite(cases::eo5);
}

#[test]
fn tc0042_c4_eo16_finalized_untouched_field_correction() {
    on_sqlite(|s| cases::eo16(s, false));
}

#[test]
fn tc0042_c4_eo16_finalized_untouched_field_rollback() {
    on_sqlite(|s| cases::eo16(s, true));
}

#[test]
fn tc0042_c4_eo17_correction_removal_then_null() {
    on_sqlite_reopening(|s, reopen| cases::eo17(s, false, true, reopen));
}

#[test]
fn tc0042_c4_eo17_correction_null_then_removal() {
    on_sqlite_reopening(|s, reopen| cases::eo17(s, false, false, reopen));
}

#[test]
fn tc0042_c4_eo17_rollback_removal_then_null() {
    on_sqlite_reopening(|s, reopen| cases::eo17(s, true, true, reopen));
}

#[test]
fn tc0042_c4_eo17_rollback_null_then_removal() {
    on_sqlite_reopening(|s, reopen| cases::eo17(s, true, false, reopen));
}

#[test]
fn tc0042_c4_eo3_newer_correction_refuses() {
    on_sqlite(|s| cases::eo3(s, 25));
}

#[test]
fn tc0042_c4_eo3_equal_stamp_refuses() {
    on_sqlite(|s| cases::eo3(s, 20));
}

#[test]
fn tc0042_c4_eo4_refuse_when_first_writer_loses() {
    on_sqlite(cases::eo4_refuse);
}

#[test]
fn tc0042_c4_eo15_finalized_correction_only_field_refuses() {
    on_sqlite(cases::eo15);
}

#[test]
fn tc0042_c4_eo14_pull_cannot_interleave() {
    on_sqlite(cases::eo14_pull);
}

#[test]
fn tc0042_c4_eo_precedence_earlier_errors_and_c1_unchanged() {
    on_sqlite(cases::eo_precedence);
}

#[test]
fn tc0042_c4_eo_exit_c1_only() {
    on_sqlite(cases::eo_exit);
}

#[test]
fn tc0042_c4_refuse_live_named() {
    on_sqlite(|s| cases::refuse_live(s, cases::Live::Named));
}

#[test]
fn tc0042_c4_refuse_live_cancelled() {
    on_sqlite(|s| cases::refuse_live(s, cases::Live::Cancelled));
}

#[test]
fn tc0042_c4_refuse_live_applied_unreconciled() {
    on_sqlite(|s| cases::refuse_live(s, cases::Live::AppliedUnreconciled));
}

#[test]
fn tc0042_c4_refuse_live_corrected_unreconciled() {
    on_sqlite(|s| cases::refuse_live(s, cases::Live::CorrectedUnreconciled));
}

#[test]
fn tc0042_c4_refuse_live_rejected_unreconciled() {
    on_sqlite(|s| cases::refuse_live(s, cases::Live::RejectedUnreconciled));
}

#[test]
fn tc0042_c4_refuse_live_earlier_live() {
    on_sqlite(|s| cases::refuse_live(s, cases::Live::Earlier));
}

#[test]
fn tc0042_c4_refuse_live_delete_in_l() {
    on_sqlite(|s| cases::refuse_live(s, cases::Live::DeleteInL));
}

#[test]
fn tc0042_c4_refuse_live_merge_hook() {
    on_sqlite(|s| cases::refuse_live(s, cases::Live::MergeHook));
}

#[test]
fn tc0042_c4_refuse_live_typed_delete() {
    on_sqlite(|s| cases::refuse_live(s, cases::Live::TypedDelete));
}

#[test]
fn tc0042_c4_refuse_live_cancel_processed_rejection() {
    on_sqlite(cases::cancel_processed_rejection);
}

#[test]
fn tc0042_c4_refuse_owned_finalized_entity() {
    on_sqlite(|s| cases::refuse_owned(s, cases::Owned::FinalizedEntity));
}

#[test]
fn tc0042_c4_refuse_owned_trimmed() {
    on_sqlite(|s| cases::refuse_owned(s, cases::Owned::Trimmed));
}

#[test]
fn tc0042_c4_refuse_owned_legacy_named_eo10b() {
    on_sqlite(|s| cases::refuse_owned(s, cases::Owned::LegacyNamed));
}

#[test]
fn tc0042_c4_g1_open_finalized_owned_field() {
    on_sqlite(cases::g1_open);
}

#[test]
fn tc0042_c4_refuse_live_typed_create() {
    on_sqlite(|s| cases::refuse_live(s, cases::Live::TypedCreate));
}

#[test]
fn tc0042_c4_refuse_rollback_named() {
    on_sqlite(|s| cases::refuse_rollback(s, cases::RollbackLive::Named));
}

#[test]
fn tc0042_c4_refuse_rollback_earlier_live() {
    on_sqlite(|s| cases::refuse_rollback(s, cases::RollbackLive::Earlier));
}

#[test]
fn tc0042_c4_refuse_rollback_applied_unreconciled() {
    on_sqlite(|s| cases::refuse_rollback(s, cases::RollbackLive::AppliedUnreconciled));
}

#[test]
fn tc0042_c4_refuse_rollback_delete_in_l() {
    on_sqlite(|s| cases::refuse_rollback(s, cases::RollbackLive::DeleteInL));
}

#[test]
fn tc0042_c4_refuse_rollback_merge_hook() {
    on_sqlite(|s| cases::refuse_rollback(s, cases::RollbackLive::MergeHook));
}

#[test]
fn tc0042_c4_refuse_rollback_equal_stamp() {
    on_sqlite(|s| cases::refuse_rollback(s, cases::RollbackLive::EqualStamp));
}

#[test]
fn tc0042_c4_refuse_rollback_older_stamp() {
    on_sqlite(|s| cases::refuse_rollback(s, cases::RollbackLive::OlderStamp));
}

#[test]
fn tc0042_c4_g2_missing_stamp_counts_as_zero() {
    on_sqlite(cases::g2_missing_stamp_boundary);
}
