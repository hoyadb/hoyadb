//! TC-0042 C4 (O2) on memory. Exclusive adapter: the default parallel runner applies. Memory has
//! no restart: the EO-ORDER/EO17 reopen step passes the same instance (an owner reopen only).
#[path = "../../../tests/conformance/entity_overlap.rs"]
mod cases;
use hoyadb_storage_memory::MemoryStorage;

#[test]
fn tc0044_cp03_c4_precedes_discard() {
    cases::cp03_c4_before_policy(MemoryStorage::default());
}

#[test]
fn tc0042_c4_eo2_correction_rebases_a_winning_pending_writer() {
    let s = MemoryStorage::default();
    cases::eo2(s);
}

#[test]
fn tc0042_c4_eo_order_promotion_only_into_eff0() {
    let s = MemoryStorage::default();
    cases::eo_order(s, |s| s);
}

#[test]
fn tc0042_c4_eo4_two_pending_writers_win() {
    let s = MemoryStorage::default();
    cases::eo4_win(s);
}

#[test]
fn tc0042_c4_eo5_rollback_rebases_pending_writer_on_other_field() {
    let s = MemoryStorage::default();
    cases::eo5(s);
}

#[test]
fn tc0042_c4_eo16_finalized_untouched_field_correction() {
    let s = MemoryStorage::default();
    cases::eo16(s, false);
}

#[test]
fn tc0042_c4_eo16_finalized_untouched_field_rollback() {
    let s = MemoryStorage::default();
    cases::eo16(s, true);
}

#[test]
fn tc0042_c4_eo17_correction_removal_then_null() {
    let s = MemoryStorage::default();
    cases::eo17(s, false, true, |s| s);
}

#[test]
fn tc0042_c4_eo17_correction_null_then_removal() {
    let s = MemoryStorage::default();
    cases::eo17(s, false, false, |s| s);
}

#[test]
fn tc0042_c4_eo17_rollback_removal_then_null() {
    let s = MemoryStorage::default();
    cases::eo17(s, true, true, |s| s);
}

#[test]
fn tc0042_c4_eo17_rollback_null_then_removal() {
    let s = MemoryStorage::default();
    cases::eo17(s, true, false, |s| s);
}

#[test]
fn tc0042_c4_eo3_newer_correction_refuses() {
    let s = MemoryStorage::default();
    cases::eo3(s, 25);
}

#[test]
fn tc0042_c4_eo3_equal_stamp_refuses() {
    let s = MemoryStorage::default();
    cases::eo3(s, 20);
}

#[test]
fn tc0042_c4_eo4_refuse_when_first_writer_loses() {
    let s = MemoryStorage::default();
    cases::eo4_refuse(s);
}

#[test]
fn tc0042_c4_eo15_finalized_correction_only_field_refuses() {
    let s = MemoryStorage::default();
    cases::eo15(s);
}

#[test]
fn tc0042_c4_eo14_pull_cannot_interleave() {
    let s = MemoryStorage::default();
    cases::eo14_pull(s);
}

#[test]
fn tc0042_c4_eo_precedence_earlier_errors_and_c1_unchanged() {
    let s = MemoryStorage::default();
    cases::eo_precedence(s);
}

#[test]
fn tc0042_c4_eo_exit_c1_only() {
    let s = MemoryStorage::default();
    cases::eo_exit(s);
}

#[test]
fn tc0042_c4_refuse_live_named() {
    let s = MemoryStorage::default();
    cases::refuse_live(s, cases::Live::Named);
}

#[test]
fn tc0042_c4_refuse_live_cancelled() {
    let s = MemoryStorage::default();
    cases::refuse_live(s, cases::Live::Cancelled);
}

#[test]
fn tc0042_c4_refuse_live_applied_unreconciled() {
    let s = MemoryStorage::default();
    cases::refuse_live(s, cases::Live::AppliedUnreconciled);
}

#[test]
fn tc0042_c4_refuse_live_corrected_unreconciled() {
    let s = MemoryStorage::default();
    cases::refuse_live(s, cases::Live::CorrectedUnreconciled);
}

#[test]
fn tc0042_c4_refuse_live_rejected_unreconciled() {
    let s = MemoryStorage::default();
    cases::refuse_live(s, cases::Live::RejectedUnreconciled);
}

#[test]
fn tc0042_c4_refuse_live_earlier_live() {
    let s = MemoryStorage::default();
    cases::refuse_live(s, cases::Live::Earlier);
}

#[test]
fn tc0042_c4_refuse_live_delete_in_l() {
    let s = MemoryStorage::default();
    cases::refuse_live(s, cases::Live::DeleteInL);
}

#[test]
fn tc0042_c4_refuse_live_merge_hook() {
    let s = MemoryStorage::default();
    cases::refuse_live(s, cases::Live::MergeHook);
}

#[test]
fn tc0042_c4_refuse_live_typed_delete() {
    let s = MemoryStorage::default();
    cases::refuse_live(s, cases::Live::TypedDelete);
}

#[test]
fn tc0042_c4_refuse_live_cancel_processed_rejection() {
    let s = MemoryStorage::default();
    cases::cancel_processed_rejection(s);
}

#[test]
fn tc0042_c4_refuse_owned_finalized_entity() {
    let s = MemoryStorage::default();
    cases::refuse_owned(s, cases::Owned::FinalizedEntity);
}

#[test]
fn tc0042_c4_refuse_owned_trimmed() {
    let s = MemoryStorage::default();
    cases::refuse_owned(s, cases::Owned::Trimmed);
}

#[test]
fn tc0042_c4_refuse_owned_legacy_named_eo10b() {
    let s = MemoryStorage::default();
    cases::refuse_owned(s, cases::Owned::LegacyNamed);
}

#[test]
fn tc0042_c4_g1_open_finalized_owned_field() {
    let s = MemoryStorage::default();
    cases::g1_open(s);
}

#[test]
fn tc0042_c4_refuse_live_typed_create() {
    let s = MemoryStorage::default();
    cases::refuse_live(s, cases::Live::TypedCreate);
}

#[test]
fn tc0042_c4_refuse_rollback_named() {
    let s = MemoryStorage::default();
    cases::refuse_rollback(s, cases::RollbackLive::Named);
}

#[test]
fn tc0042_c4_refuse_rollback_earlier_live() {
    let s = MemoryStorage::default();
    cases::refuse_rollback(s, cases::RollbackLive::Earlier);
}

#[test]
fn tc0042_c4_refuse_rollback_applied_unreconciled() {
    let s = MemoryStorage::default();
    cases::refuse_rollback(s, cases::RollbackLive::AppliedUnreconciled);
}

#[test]
fn tc0042_c4_refuse_rollback_delete_in_l() {
    let s = MemoryStorage::default();
    cases::refuse_rollback(s, cases::RollbackLive::DeleteInL);
}

#[test]
fn tc0042_c4_refuse_rollback_merge_hook() {
    let s = MemoryStorage::default();
    cases::refuse_rollback(s, cases::RollbackLive::MergeHook);
}

#[test]
fn tc0042_c4_refuse_rollback_equal_stamp() {
    let s = MemoryStorage::default();
    cases::refuse_rollback(s, cases::RollbackLive::EqualStamp);
}

#[test]
fn tc0042_c4_refuse_rollback_older_stamp() {
    let s = MemoryStorage::default();
    cases::refuse_rollback(s, cases::RollbackLive::OlderStamp);
}

#[test]
fn tc0042_c4_g2_missing_stamp_counts_as_zero() {
    let s = MemoryStorage::default();
    cases::g2_missing_stamp_boundary(s);
}
