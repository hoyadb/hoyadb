//! API-0008: unchanged shared assertions on actual OPFS, separate from host RPC.
use super::*;

#[path = "../../../tests/conformance/command_owner.rs"]
mod owner;
#[path = "../../../tests/conformance/sync_storage.rs"]
mod storage;
#[path = "../../../tests/conformance/command_sync.rs"]
mod sync;
thread_local! { static CASE: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }

fn inventory() -> Vec<String> {
    POOL.with(|slot| {
        let slot = slot.borrow();
        let mut names = slot
            .as_ref()
            .expect("initialized pool")
            .names()
            .expect("active owned pool");
        names.sort();
        names
    })
}

#[wasm_bindgen]
pub fn command_conformance(index: u32) -> Result<JsValue, JsValue> {
    if index >= 14 {
        return Err(backend("unknown command case"));
    }
    let before = inventory();
    let serial = CASE.with(|n| {
        let value = n.get();
        n.set(value.checked_add(1).expect("case counter"));
        value
    });
    // Include wall clock and per-worker counter. Even an interrupted earlier
    // worker's evidence cannot be mistaken for this case's owned files.
    let prefix = format!(
        "phase-four-conformance-{}-{serial}-",
        js_sys::Date::now() as u64
    );
    let mut next = 0u32;
    let mut fresh = || {
        let name = format!("{prefix}{next}.sqlite");
        next += 1;
        assert!(!before.iter().any(|old| old.trim_start_matches('/') == name));
        fixture_open(name).expect("fresh owned conformance store")
    };
    let name = match index {
        0 => {
            storage::atomic_initialization_and_order(fresh());
            "atomic_initialization_and_order"
        }
        1 => {
            storage::ordinary_failure_preserves_absence_data_and_counter(fresh());
            "ordinary_failure_preserves_absence_data_and_counter"
        }
        2 => {
            storage::complete_cas_and_initial_guards(fresh());
            "complete_cas_and_initial_guards"
        }
        3 => {
            storage::reserved_corrupt_and_future_packets_reject(fresh());
            "reserved_corrupt_and_future_packets_reject"
        }
        4 => {
            storage::live_lifecycle_guard_and_resolved_history(fresh());
            "live_lifecycle_guard_and_resolved_history"
        }
        5 => {
            storage::identity_and_frozen_transition_guards(fresh());
            "identity_and_frozen_transition_guards"
        }
        6 => {
            storage::live_undo_uses_public_rows_and_checked_model_types(fresh());
            "live_undo_uses_public_rows_and_checked_model_types"
        }
        7 => {
            storage::live_created_ownership_requires_current_model_tables(fresh());
            "live_created_ownership_requires_current_model_tables"
        }
        8 => {
            owner::owner_contract(&mut fresh);
            "owner_contract"
        }
        9 => {
            sync::dispatch(fresh());
            "dispatch"
        }
        10 => {
            sync::correction_inverse(fresh());
            "correction_inverse"
        }
        11 => {
            sync::graph(fresh());
            "graph"
        }
        12 => {
            sync::zero_history(fresh());
            "zero_history"
        }
        13 => {
            sync::capacity_boundary(fresh());
            "capacity_boundary"
        }
        _ => unreachable!(),
    };
    // Every shared function drops its stores before returning. Delete exactly
    // our allocated names, preserving unrelated and interrupted-case files.
    POOL.with(|slot| {
        let slot = slot.borrow();
        let pool = slot.as_ref().expect("initialized pool");
        for n in 0..next {
            pool.remove(&format!("{prefix}{n}.sqlite"))
                .expect("closed owned fixture");
        }
    });
    let after = inventory();
    assert_eq!(
        before, after,
        "command conformance restores pooled file inventory"
    );
    let array = |names: &[String]| names.iter().map(JsValue::from).collect::<Array>();
    Ok(object(&[
        ("case", name.into()),
        ("files_before", array(&before).into()),
        ("files_after", array(&after).into()),
    ]))
}
