#[path = "../../../tests/conformance/conflict_policy.rs"]
mod cases;
use hoyadb_storage_memory::MemoryStorage;
#[path = "../../../tests/conformance/conflict_policy_race.rs"]
mod race;

#[test]
fn tc0044_cp04() {
    cases::cp04(MemoryStorage::default());
}
#[test]
fn tc0044_cp06() {
    cases::cp06(MemoryStorage::default());
}

#[test]
fn tc0044_cp01_02_03_configuration() {
    cases::configuration(MemoryStorage::default());
}
#[test]
fn tc0044_cp05_10_correction_guards() {
    cases::correction_guards(MemoryStorage::default());
}
#[test]
fn tc0044_cp08_16_cancel() {
    cases::later_work_cancel(MemoryStorage::default());
}
#[test]
fn tc0044_cp15a_b_owner_reopen_history() {
    for history in [1, 100] {
        for sent in [false, true] {
            cases::history(MemoryStorage::default(), history, sent, |s| s);
        }
    }
}
#[test]
fn tc0044_cp15d_e_owner_reopen_dependent() {
    for mapping in [true, false] {
        cases::dependent_only(MemoryStorage::default(), mapping, |s| s);
    }
}
#[test]
fn tc0044_cp07_in_flight() {
    cases::in_flight(MemoryStorage::default());
}
#[test]
fn tc0044_cp09_entity_refusal() {
    cases::entity_refusal(MemoryStorage::default());
}
#[test]
fn tc0044_cp11_report_limit() {
    cases::report_limit(MemoryStorage::default());
}
#[test]
fn tc0044_cp15c_mapped_owner_reopen() {
    cases::mapped_history(MemoryStorage::default(), |s| s);
}
#[test]
fn tc0044_cp10_commit_guard_race() {
    race::cp10(MemoryStorage::default());
}
#[test]
fn tc0044_cp06_transitive_cancelled_closure() {
    cases::transitive(MemoryStorage::default());
}
