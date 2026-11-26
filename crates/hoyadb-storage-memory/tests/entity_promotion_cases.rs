//! TC-0042 C1a (DEF-0039) on memory. Exclusive adapter: the default parallel runner applies.
#[path = "../../../tests/conformance/entity_promotion.rs"]
mod cases;
use hoyadb_storage_memory::MemoryStorage;

#[test]
fn tc0042_c1a_a_equal_clock_confirm_a_first() {
    cases::c1a_a(MemoryStorage::default());
}

#[test]
fn tc0042_c1a_b_equal_clock_confirm_e_first() {
    cases::c1a_b(MemoryStorage::default());
}

#[test]
fn tc0042_c1a_null_removal_then_null() {
    cases::c1a_null(MemoryStorage::default(), false);
}

#[test]
fn tc0042_c1a_null_null_then_removal() {
    cases::c1a_null(MemoryStorage::default(), true);
}

#[test]
fn tc0042_c1a_corrected_keeps_selected_stamp() {
    cases::c1a_corrected(MemoryStorage::default());
}

#[test]
fn tc0042_c1a_delete_promotes_matching_tombstone() {
    cases::c1a_delete(MemoryStorage::default());
}

#[test]
fn tc0042_c1a_ctrl_distinct_clocks_and_present_zero() {
    cases::c1a_ctrl(MemoryStorage::default);
}

#[test]
fn tc0042_c1a_no_retroactive_repair() {
    cases::c1a_no_repair(MemoryStorage::default());
}

#[test]
fn tc0042_c1a_mapping_rekeys_and_promotes_only_the_accepted_value() {
    cases::c1a_mapping(MemoryStorage::default());
}
