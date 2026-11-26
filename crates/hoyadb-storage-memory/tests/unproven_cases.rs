//! TC-0041 SI-A07: adapters without proven exclusivity keep the global coordinator.
//! This binary holds one test so no other owner shares its process-wide coordinator.
// Only the shared opener is needed here; the per-database cases run elsewhere.
#[allow(dead_code)]
#[path = "../../../tests/conformance/ownership.rs"]
mod cases;
use hoyadb_core::{command::CommandError, storage::*};
use std::collections::BTreeMap;
#[path = "support/unproven.rs"]
mod unproven;
use unproven::Unproven;

#[test]
fn si_a07_unproven_adapters_stay_on_the_global_coordinator() {
    assert_eq!(Unproven::default().ownership(), StoreOwnership::Unproven);
    let a = cases::open(Unproven::default(), 0x47);
    let mut b = cases::open(Unproven::default(), 0x48);
    let queued = a.request().unwrap();
    let name = BTreeMap::from([("name".into(), Value::String("blocked".into()))]);
    assert_eq!(
        cases::run(b.insert("parents", name.clone())),
        Err(CommandError::Busy)
    );
    a.cancel_request(queued).unwrap();
    cases::run(b.insert("parents", name)).unwrap();
}
