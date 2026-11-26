//! SPEC-0039 / TC-0043 storage seam: a SyncPlan may carry read-only expectation guards
//! (mutated rows are a subset of guarded rows). Duplicate, identity, version, unknown-table and
//! Reset-inventory checks are unchanged. Uses only the existing SyncPlan/RowGuard API, so the
//! unchanged adapters compile and fail by result.
#[allow(dead_code)]
#[path = "sync_storage.rs"]
mod base;
use base::{plan, row, run, setup};
use hoyadb_core::storage::*;

const GUARD_CONFLICT: &str = "command application row changed before commit";

/// The adapter's conflict message (StorageError's Debug form is redacted).
fn message(error: &StorageError) -> &str {
    match error {
        StorageError::SchemaConflict {
            message,
        } => message,
        _ => "<not a schema conflict>",
    }
}

#[track_caller]
fn committed<S: SyncStorage>(store: &mut S, plan: SyncPlan) -> Vec<CommittedChange> {
    match store.apply_sync_plan(plan) {
        Ok(changes) => changes,
        Err(error) => panic!("plan refused: {}", message(&error)),
    }
}

type State = (Option<Vec<Row>>, Option<ModelState>, Vec<Row>);

fn state<S: SyncStorage>(store: &S) -> State {
    (
        store.load_sync().unwrap(),
        run(store.catalog()).unwrap().state,
        run(store.scan("items")).unwrap(),
    )
}

/// local:1 = 1 (owned by the fixture ledger) and an unrelated s = 5, then a base plan that
/// replaces local:1 with its exact initial guard and leaves the ledger unchanged.
fn fixture<S: SyncStorage>(store: S) -> (S, SyncPlan) {
    let mut store = setup(store);
    store.apply_sync_plan(plan(&store)).unwrap();
    let sync = store.load_sync().unwrap();
    let model = run(store.catalog()).unwrap().state.unwrap();
    let mut unchanged = SyncPlan {
        transition: SyncTransition::Normal,
        expected_model: model.clone(),
        next_model: model,
        expected_sync: sync.clone(),
        next_sync: sync.unwrap(),
        guards: vec![RowGuard {
            table: "items".into(),
            id: "s".into(),
            expected: None,
        }],
        mutations: vec![Mutation::Insert {
            table: "items".into(),
            row: row("s", 5),
        }],
    };
    store.apply_sync_plan(unchanged.clone()).unwrap();
    unchanged.guards = vec![RowGuard {
        table: "items".into(),
        id: "local:1".into(),
        expected: Some(row("local:1", 1)),
    }];
    unchanged.mutations = vec![Mutation::Replace {
        table: "items".into(),
        row: row("local:1", 2),
    }];
    (store, unchanged)
}

fn guard(id: &str, expected: Option<Row>) -> RowGuard {
    RowGuard {
        table: "items".into(),
        id: id.into(),
        expected,
    }
}

#[track_caller]
fn refused<S: SyncStorage>(store: &mut S, plan: SyncPlan, message: Option<&str>) {
    let before = state(store);
    let error = store.apply_sync_plan(plan).unwrap_err();
    if let Some(expected) = message {
        assert_eq!(self::message(&error), expected);
    }
    assert_eq!(state(store), before, "nothing changed");
}

/// RG1: matching read-only guards (an existing row and an absent row) commit with the mutation.
pub fn matching_read_only_guards_commit<S: SyncStorage>(store: S) {
    let (mut store, mut base) = fixture(store);
    base.guards.push(guard("s", Some(row("s", 5))));
    base.guards.push(guard("absent", None));
    assert_eq!(committed(&mut store, base).len(), 1);
    assert_eq!(
        run(store.scan("items")).unwrap(),
        vec![row("local:1", 2), row("s", 5)]
    );
}

/// RG2: a mismatching read-only guard refuses the whole plan as a guard conflict.
pub fn mismatching_read_only_guard_is_atomic<S: SyncStorage>(store: S) {
    let (mut store, base) = fixture(store);
    for extra in [
        guard("s", Some(row("s", 9))),
        guard("s", None),
        guard("absent", Some(row("absent", 1))),
    ] {
        let mut bad = base.clone();
        bad.guards.push(extra);
        refused(&mut store, bad, Some(GUARD_CONFLICT));
    }
}

/// RG3: an expectation with no mutation at all (a no-op absent Remove) is a storage guard.
pub fn guard_only_absence_expectation<S: SyncStorage>(store: S) {
    let (mut store, mut base) = fixture(store);
    base.mutations.clear();
    base.guards = vec![guard("absent", None)];
    let before = state(&store);
    assert_eq!(committed(&mut store, base.clone()), vec![]);
    assert_eq!(state(&store), before);
    base.guards = vec![guard("s", None)];
    refused(&mut store, base, Some(GUARD_CONFLICT));
}

/// RG4 (control): an unknown or reserved-table guard, also with expected absence, is refused.
pub fn unknown_table_guard_is_refused<S: SyncStorage>(store: S) {
    let (mut store, base) = fixture(store);
    for table in ["unmanaged", "__hoya_sync", "__hoya_model"] {
        for expected in [None, Some(row("x", 1))] {
            let mut bad = base.clone();
            bad.guards.push(RowGuard {
                table: table.into(),
                id: "x".into(),
                expected,
            });
            refused(&mut store, bad, None);
        }
    }
}

/// RG5 (control): mutations still require exact initial guards; duplicates still refused.
pub fn mutations_still_require_guards<S: SyncStorage>(store: S) {
    let (mut store, base) = fixture(store);
    let mut bad = base.clone();
    bad.guards.clear();
    refused(&mut store, bad, None);
    let mut bad = base.clone();
    bad.guards.push(guard("s", Some(row("s", 5))));
    bad.guards.push(guard("s", Some(row("s", 5))));
    refused(&mut store, bad, None);
}
