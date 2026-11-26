#![cfg(feature = "work-counters")]
#[path = "../../../tests/conformance/resolution_snapshot.rs"]
mod cases;
use hoyadb_core::storage::StoreOwnership;
use hoyadb_storage_memory::MemoryStorage;
fn store() -> MemoryStorage {
    MemoryStorage::default()
}
#[test]
fn si_y08_mapping_references_guards_and_occupancy_match_unproven() {
    for occupied in [false, true] {
        let mut measured = vec![];
        for n in [10, 1000] {
            let mut outcomes = vec![];
            for ownership in [StoreOwnership::Exclusive, StoreOwnership::Unproven] {
                let (outcome, work) = cases::exercise(store(), n, ownership, occupied);
                outcomes.push(outcome);
                if ownership == StoreOwnership::Exclusive {
                    measured.push(work);
                }
            }
            assert_eq!(outcomes[0], outcomes[1]);
        }
        cases::assert_bounded(&measured[0], &measured[1]);
    }
}
