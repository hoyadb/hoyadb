//! TC-0041 SI-O03-SI-O05 on the memory adapter.
#[path = "../../../tests/conformance/entity_scaling.rs"]
mod cases;
use hoyadb_core::command::CommandError;
use hoyadb_storage_memory::MemoryStorage;

#[test]
fn si_o03_entity_staging_is_equal_at_n_without_inventory_validation() {
    let small = cases::entity_staging_work(MemoryStorage::default(), 10);
    let large = cases::entity_staging_work(MemoryStorage::default(), 1000);
    assert!(
        small.iter().all(|(_, inventory)| *inventory == 0),
        "{small:?}"
    );
    assert_eq!(small, large, "work depends on N");
}

#[test]
fn si_o04_reconfigure_emptiness_check_is_equal_at_n() {
    let small =
        cases::reconfigure_emptiness_work(MemoryStorage::default(), MemoryStorage::default(), 10);
    let large =
        cases::reconfigure_emptiness_work(MemoryStorage::default(), MemoryStorage::default(), 1000);
    assert_eq!(small.0, Err(CommandError::Protocol));
    assert_eq!(small.2, Ok(()));
    assert_eq!(
        (small.1, &large.0, &large.2),
        (large.1, &small.0, &small.2),
        "work depends on N"
    );
}

#[test]
fn si_o05_merge_prepare_is_equal_at_n() {
    let mut work = Vec::new();
    for n in [10, 1000] {
        let (mut db, _) = cases::configured(MemoryStorage::default(), n);
        let (session, snapshot) = cases::merge_prepare_work(&mut db);
        cases::run(db.merge_commit(session)).unwrap();
        work.push(snapshot);
    }
    assert_eq!(work[0], work[1], "work depends on N: {work:?}");
}

/// SI-O05 (char): any owner write between merge prepare and commit moves the
/// ledger revision, which the commit compares, so the merge is stale.
#[test]
fn si_o05_owner_write_between_prepare_and_commit_stales_the_merge() {
    let (mut db, filler) = cases::configured(MemoryStorage::default(), 3);
    let (session, _) = cases::merge_prepare_work(&mut db);
    cases::run(db.replace(
        "local",
        &filler[0],
        std::collections::BTreeMap::from([(
            "note".into(),
            hoyadb_core::storage::Value::String("edited".into()),
        )]),
    ))
    .unwrap();
    assert_eq!(
        cases::run(db.merge_commit(session)),
        Err(CommandError::StalePull)
    );
}
