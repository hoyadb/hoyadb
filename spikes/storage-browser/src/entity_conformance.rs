//! API-0009 section 6: unchanged shared Phase 5 assertions on the real OPFS adapter.
use super::*;
use std::cell::Cell;
#[path = "../../../tests/conformance/entity_sync.rs"]
mod entity;
#[path = "../../../tests/conformance/entity_pull.rs"]
mod pull;
thread_local! { static CASE: Cell<u32> = const { Cell::new(0) }; }

fn inventory() -> Vec<String> {
    POOL.with(|slot| {
        let mut names = slot
            .borrow()
            .as_ref()
            .expect("initialized pool")
            .names()
            .expect("active owned pool");
        names.sort();
        names
    })
}

#[wasm_bindgen]
pub fn entity_conformance(index: u32) -> Result<JsValue, JsValue> {
    if index >= 15 {
        return Err(backend("unknown entity case"));
    }
    let before = inventory();
    let serial = CASE.with(|counter| {
        let next = counter.get();
        counter.set(next.checked_add(1).expect("case counter"));
        next
    });
    let prefix = format!(
        "phase-five-conformance-{}-{serial}-",
        js_sys::Date::now() as u64
    );
    let allocated = Cell::new(0u32);
    let fresh = || {
        let next = allocated.get();
        allocated.set(next.checked_add(1).expect("fixture counter"));
        let name = format!("{prefix}{next}.sqlite");
        assert!(!before.iter().any(|old| old.trim_start_matches('/') == name));
        fixture_open(name).expect("fresh owned entity conformance store")
    };
    let name = match index {
        0 => {
            entity::automatic_entities(fresh);
            "automatic_entities"
        }
        1 => {
            entity::admission_and_inverse(fresh);
            "admission_and_inverse"
        }
        2 => {
            entity::two_client_resolution(fresh);
            "two_client_resolution"
        }
        3 => {
            pull::pull_atomicity_bootstrap_and_reset(fresh());
            "pull_atomicity_bootstrap_and_reset"
        }
        4 => {
            pull::reset_inventory(fresh());
            "reset_inventory"
        }
        5 => {
            pull::lifecycle_comparison_guard(fresh());
            "lifecycle_comparison_guard"
        }
        6 => {
            pull::pull_recovery_protection(fresh());
            "pull_recovery_protection"
        }
        7 => {
            pull::progress_transition_guard(fresh());
            "progress_transition_guard"
        }
        8 => {
            pull::reset_modes_and_coordinator_guard(fresh);
            "reset_modes_and_coordinator_guard"
        }
        9 => {
            entity::managed_links_and_mapped_rows(fresh);
            "managed_links_and_mapped_rows"
        }
        10 => {
            entity::reconfiguration_guards(fresh);
            "reconfiguration_guards"
        }
        11 => {
            pull::typed_correction_clocks(fresh());
            "typed_correction_clocks"
        }
        12 => {
            pull::reconfigure_inventory(fresh());
            "reconfigure_inventory"
        }
        13 => {
            pull::pull_tombstone_mapping_target(fresh());
            "pull_tombstone_mapping_target"
        }
        14 => {
            pull::pull_after_rolled_back_sent_cancellation(fresh());
            "pull_after_rolled_back_sent_cancellation"
        }
        _ => unreachable!(),
    };
    POOL.with(|slot| {
        let slot = slot.borrow();
        let pool = slot.as_ref().expect("initialized pool");
        for n in 0..allocated.get() {
            pool.remove(&format!("{prefix}{n}.sqlite"))
                .expect("closed owned entity fixture");
        }
    });
    let after = inventory();
    assert_eq!(
        before, after,
        "entity conformance restores pooled file inventory"
    );
    let array = |names: &[String]| names.iter().map(JsValue::from).collect::<Array>();
    Ok(object(&[
        ("case", name.into()),
        ("files_before", array(&before).into()),
        ("files_after", array(&after).into()),
    ]))
}
