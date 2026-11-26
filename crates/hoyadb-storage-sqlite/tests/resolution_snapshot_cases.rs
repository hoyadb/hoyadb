#![cfg(feature = "work-counters")]
#[path = "../../../tests/conformance/resolution_snapshot.rs"]
mod cases;
use hoyadb_core::storage::StoreOwnership;
use hoyadb_storage_sqlite::SqliteStorage;
struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("hoya-resolution-snapshot-{}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self, name: &str) -> SqliteStorage {
        SqliteStorage::open_unprotected(self.0.join(name)).unwrap()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn si_y08_mapping_references_guards_and_occupancy_match_unproven() {
    let directory = Directory::new();
    for occupied in [false, true] {
        let mut measured = vec![];
        for n in [10, 1000] {
            let mut outcomes = vec![];
            for ownership in [StoreOwnership::Exclusive, StoreOwnership::Unproven] {
                let (outcome, work) = cases::exercise(
                    directory.store(&format!("{n}-{occupied}-{ownership:?}.sqlite")),
                    n,
                    ownership,
                    occupied,
                );
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
