//! API-0007 / TC-0032 C06/C07/C11/C17: actual atomic recovery capability.
use hoyadb_core::{
    command::{Json, records::*},
    model::*,
    storage::*,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    task::{Context, Poll, Waker},
};

pub fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("immediate adapter suspended"),
    }
}

pub fn schema(version: u32) -> SchemaDefinition {
    SchemaDefinition {
        version,
        tables: BTreeMap::from([(
            "items".into(),
            TableDefinition {
                columns: BTreeMap::from([(
                    "value".into(),
                    Column {
                        kind: ColumnType::Integer,
                        optional: false,
                    },
                )]),
                relations: BTreeMap::new(),
            },
        )]),
    }
}

pub fn row(id: &str, value: i64) -> Row {
    Row {
        id: id.into(),
        fields: BTreeMap::from([
            ("value".into(), Value::Integer(value)),
            (ROW_VERSION.into(), Value::Integer(1)),
        ]),
    }
}

fn ledger() -> SyncLedger {
    let key = format!("{:016x}-{}-{:016x}", 100, "0".repeat(32), 1);
    let record = SyncRecord {
        entity: None,
        metadata_undo: vec![],
        key: key.clone(),
        command: "test-v1".into(),
        args: Json::Object(BTreeMap::new()),
        context: Json::Object(BTreeMap::from([
            ("identity".into(), Json::Null),
            (
                "capturedAt".into(),
                Json::String("2026-10-23T12:00:00.000Z".into()),
            ),
        ])),
        schema_version: 1,
        order: 1,
        reference_paths: vec![],
        dependencies: BTreeSet::new(),
        created: BTreeMap::from([("local:1".into(), "items".into())]),
        mappings: BTreeMap::new(),
        undo: vec![],
        state: SyncState::Queued,
        frozen: None,
        attempts: 0,
        due_ms: 0,
        outcome: None,
        rolled_back: false,
        reconciled: false,
    };
    SyncLedger {
        entity: None,
        header: SyncHeader {
            namespace: "0".repeat(32),
            sequence: 2,
            last_ms: 100,
            options: SyncOptions::default(),
        },
        records: BTreeMap::from([(key, record)]),
    }
}

pub fn setup<S: SyncStorage>(store: S) -> S {
    run(Database::open(store, schema(1), vec![]))
        .unwrap()
        .into_storage()
}

pub fn plan<S: SyncStorage>(store: &S) -> SyncPlan {
    let state = run(store.catalog()).unwrap().state.unwrap();
    let mut next = state.clone();
    next.next_id = 2;
    SyncPlan {
        transition: SyncTransition::Normal,
        expected_model: state,
        next_model: next,
        expected_sync: store.load_sync().unwrap(),
        next_sync: ledger().to_rows().unwrap(),
        guards: vec![RowGuard {
            table: "items".into(),
            id: "local:1".into(),
            expected: None,
        }],
        mutations: vec![Mutation::Insert {
            table: "items".into(),
            row: row("local:1", 1),
        }],
    }
}

pub fn atomic_initialization_and_order<S: SyncStorage>(store: S) {
    let mut store = setup(store);
    assert_eq!(store.load_sync().unwrap(), None);
    let mut plan = plan(&store);
    plan.mutations.push(Mutation::Replace {
        table: "items".into(),
        row: row("local:1", 2),
    });
    let changes = store.apply_sync_plan(plan).unwrap();
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].before, None);
    assert_eq!(changes[1].before, Some(row("local:1", 1)));
    assert_eq!(
        run(store.get("items", "local:1")).unwrap(),
        Some(row("local:1", 2))
    );
    assert_eq!(run(store.catalog()).unwrap().tables, vec!["items"]);
    assert_eq!(run(store.catalog()).unwrap().state.unwrap().next_id, 2);
    assert_eq!(
        SyncLedger::from_rows(&store.load_sync().unwrap().unwrap()).unwrap(),
        ledger()
    );
}

pub fn ordinary_failure_preserves_absence_data_and_counter<S: SyncStorage>(store: S) {
    let mut store = setup(store);
    let before = run(store.catalog()).unwrap();
    let mut plan = plan(&store);
    plan.guards.push(RowGuard {
        table: "items".into(),
        id: "missing".into(),
        expected: None,
    });
    plan.mutations.push(Mutation::Replace {
        table: "items".into(),
        row: row("missing", 5),
    });
    assert!(store.apply_sync_plan(plan).is_err());
    assert_eq!(store.load_sync().unwrap(), None);
    assert_eq!(run(store.catalog()).unwrap(), before);
    assert_eq!(run(store.scan("items")).unwrap(), vec![]);
}

pub fn complete_cas_and_initial_guards<S: SyncStorage>(store: S) {
    let mut store = setup(store);
    store.apply_sync_plan(plan(&store)).unwrap();
    let original_sync = store.load_sync().unwrap();
    let state = run(store.catalog()).unwrap().state.unwrap();
    let base = SyncPlan {
        transition: SyncTransition::Normal,
        expected_model: state.clone(),
        next_model: state.clone(),
        expected_sync: original_sync.clone(),
        next_sync: original_sync.clone().unwrap(),
        guards: vec![RowGuard {
            table: "items".into(),
            id: "local:1".into(),
            expected: Some(row("local:1", 1)),
        }],
        mutations: vec![Mutation::Replace {
            table: "items".into(),
            row: row("local:1", 2),
        }],
    };
    let mut variants = vec![];
    let mut bad = base.clone();
    bad.guards[0].expected = Some(row("local:1", 9));
    variants.push(bad);
    let mut bad = base.clone();
    bad.expected_sync = None;
    variants.push(bad);
    let mut bad = base.clone();
    bad.expected_model.next_id += 1;
    variants.push(bad);
    let mut bad = base.clone();
    bad.guards.push(bad.guards[0].clone());
    variants.push(bad);
    let mut bad = base.clone();
    bad.guards.clear();
    variants.push(bad);
    let mut bad = base.clone();
    bad.mutations = vec![Mutation::Remove {
        table: "items".into(),
        id: "local:1".into(),
    }];
    bad.guards[0].expected = Some(row("local:1", 9));
    variants.push(bad);
    let mut bad = base.clone();
    bad.next_model.version = 2;
    variants.push(bad);
    let mut bad = base.clone();
    bad.mutations = vec![Mutation::Replace {
        table: "items".into(),
        row: Row {
            id: "local:1".into(),
            fields: BTreeMap::from([
                ("value".into(), Value::Integer(9)),
                (ROW_VERSION.into(), Value::Integer(2)),
            ]),
        },
    }];
    variants.push(bad);
    for bad in variants {
        assert!(store.apply_sync_plan(bad).is_err());
        assert_eq!(store.load_sync().unwrap(), original_sync);
        assert_eq!(run(store.catalog()).unwrap().state, Some(state.clone()));
        assert_eq!(
            run(store.get("items", "local:1")).unwrap(),
            Some(row("local:1", 1))
        );
    }
    assert_eq!(store.apply_sync_plan(base).unwrap().len(), 1);
}

pub fn reserved_corrupt_and_future_packets_reject<S: SyncStorage>(store: S) {
    let mut store = setup(store);
    let base = plan(&store);
    let before = run(store.catalog()).unwrap();
    let mut variants = vec![];
    let mut bad = base.clone();
    bad.next_sync[0]
        .fields
        .insert("format".into(), Value::Integer(2));
    variants.push(bad);
    let mut bad = base.clone();
    bad.mutations[0] = Mutation::Insert {
        table: MODEL_TABLE.into(),
        row: row("bad", 1),
    };
    bad.guards[0].table = MODEL_TABLE.into();
    bad.guards[0].id = "bad".into();
    variants.push(bad);
    let mut bad = base;
    bad.next_sync[0].fields.insert("extra".into(), Value::Null);
    variants.push(bad);
    for bad in variants {
        assert!(store.apply_sync_plan(bad).is_err());
        assert_eq!(store.load_sync().unwrap(), None);
        assert_eq!(run(store.catalog()).unwrap(), before);
    }
    run(store.create_table(SYNC_TABLE)).unwrap();
    assert!(run(store.catalog()).is_err());
    assert!(store.load_sync().is_err());
}

pub fn live_lifecycle_guard_and_resolved_history<S: SyncStorage>(store: S) {
    let mut store = setup(store);
    store.apply_sync_plan(plan(&store)).unwrap();
    let old = store.load_sync().unwrap().unwrap();
    assert!(run(store.catalog()).is_ok());
    let mut store = match run(Database::open_recoverable(store, schema(2), vec![])) {
        Err((_, store)) => store,
        Ok(_) => panic!("live migration succeeded"),
    };
    assert_eq!(store.load_sync().unwrap(), Some(old.clone()));
    assert_eq!(
        run(store.get("items", "local:1")).unwrap(),
        Some(row("local:1", 1))
    );
    let mut resolved = SyncLedger::from_rows(&old).unwrap();
    let record = resolved.records.values_mut().next().unwrap();
    record.state = SyncState::Cancelled;
    record.rolled_back = true;
    record.reconciled = true;
    let state = run(store.catalog()).unwrap().state.unwrap();
    store
        .apply_sync_plan(SyncPlan {
            transition: SyncTransition::Normal,
            expected_model: state.clone(),
            next_model: state,
            expected_sync: Some(old),
            next_sync: resolved.to_rows().unwrap(),
            guards: vec![],
            mutations: vec![],
        })
        .unwrap();
    let store = run(Database::open(store, schema(2), vec![]))
        .unwrap()
        .into_storage();
    assert!(store.load_sync().unwrap().is_some());
    assert_eq!(run(store.catalog()).unwrap().state.unwrap().version, 2);
}

pub fn identity_and_frozen_transition_guards<S: SyncStorage>(store: S) {
    let mut store = setup(store);
    let mut initial = plan(&store);
    let mut ledger = SyncLedger::from_rows(&initial.next_sync).unwrap();
    ledger.header.sequence = 100;
    ledger.header.last_ms = 200;
    initial.next_sync = ledger.to_rows().unwrap();
    store.apply_sync_plan(initial).unwrap();
    let original = store.load_sync().unwrap().unwrap();
    let state = run(store.catalog()).unwrap().state.unwrap();
    let base = SyncPlan {
        transition: SyncTransition::Normal,
        expected_model: state.clone(),
        next_model: state,
        expected_sync: Some(original.clone()),
        next_sync: original.clone(),
        guards: vec![],
        mutations: vec![],
    };
    let mut variants = vec![];
    let mut bad = ledger.clone();
    bad.header.sequence -= 1;
    variants.push(bad);
    let mut bad = ledger.clone();
    bad.header.last_ms -= 1;
    variants.push(bad);
    let mut bad = ledger.clone();
    bad.header.namespace = "a".repeat(32);
    let mut record = bad.records.pop_first().unwrap().1;
    record.key = format!(
        "{:016x}-{}-{:016x}",
        100, bad.header.namespace, record.order
    );
    bad.records.insert(record.key.clone(), record);
    variants.push(bad);
    let mut bad = ledger.clone();
    bad.records.values_mut().next().unwrap().schema_version = 2;
    variants.push(bad);
    for bad in variants {
        let mut plan = base.clone();
        plan.next_sync = bad.to_rows().unwrap();
        assert!(store.apply_sync_plan(plan).is_err());
        assert_eq!(store.load_sync().unwrap(), Some(original.clone()));
    }
    let record = ledger.records.values_mut().next().unwrap();
    record.state = SyncState::Sent;
    record.attempts = 1;
    record.frozen = Some(record.envelope().canonical().unwrap());
    let mut sent_plan = base;
    sent_plan.next_sync = ledger.to_rows().unwrap();
    store.apply_sync_plan(sent_plan.clone()).unwrap();
    sent_plan.expected_sync = Some(sent_plan.next_sync.clone());
    let record = ledger.records.values_mut().next().unwrap();
    record.args = Json::Object(BTreeMap::from([("changed".into(), Json::Bool(true))]));
    record.frozen = Some(record.envelope().canonical().unwrap());
    sent_plan.next_sync = ledger.to_rows().unwrap();
    assert!(store.apply_sync_plan(sent_plan.clone()).is_err());
    assert_eq!(
        SyncLedger::from_rows(&store.load_sync().unwrap().unwrap()).unwrap(),
        SyncLedger::from_rows(&sent_plan.expected_sync.unwrap()).unwrap()
    );
}

pub fn live_undo_uses_public_rows_and_checked_model_types<S: SyncStorage>(store: S) {
    let mut store = setup(store);
    let base = plan(&store);
    for (table, fields) in [
        (
            "items",
            BTreeMap::from([("value".into(), Value::String("wrong type".into()))]),
        ),
        (
            "missing",
            BTreeMap::from([("value".into(), Value::Integer(1))]),
        ),
        (
            "items",
            BTreeMap::from([
                ("value".into(), Value::Integer(1)),
                ("future".into(), Value::Integer(1)),
            ]),
        ),
    ] {
        let mut ledger = ledger();
        ledger.records.values_mut().next().unwrap().undo = vec![CommittedChange {
            table: table.into(),
            id: "local:1".into(),
            before: None,
            after: Some(Row {
                id: "local:1".into(),
                fields,
            }),
        }];
        let mut bad = base.clone();
        bad.next_sync = ledger.to_rows().unwrap();
        assert!(store.apply_sync_plan(bad).is_err());
        assert_eq!(store.load_sync().unwrap(), None);
        assert_eq!(run(store.scan("items")).unwrap(), vec![]);
    }
    let mut ledger = ledger();
    ledger.records.values_mut().next().unwrap().undo = vec![CommittedChange {
        table: "items".into(),
        id: "local:1".into(),
        before: None,
        after: Some(Row {
            id: "local:1".into(),
            fields: BTreeMap::from([("value".into(), Value::Integer(1))]),
        }),
    }];
    let mut good = base;
    good.next_sync = ledger.to_rows().unwrap();
    store.apply_sync_plan(good).unwrap();
}

pub fn live_created_ownership_requires_current_model_tables<S: SyncStorage>(store: S) {
    let mut store = setup(store);
    let before = run(store.catalog()).unwrap();
    let mut bad = plan(&store);
    let mut malformed = SyncLedger::from_rows(&bad.next_sync).unwrap();
    malformed
        .records
        .values_mut()
        .next()
        .unwrap()
        .created
        .insert("local:a".into(), "undeclared_table".into());
    bad.next_sync = malformed.to_rows().unwrap();
    assert!(store.apply_sync_plan(bad).is_err());
    assert_eq!(store.load_sync().unwrap(), None);
    assert_eq!(run(store.catalog()).unwrap(), before);
    assert_eq!(run(store.scan("items")).unwrap(), vec![]);

    let mut history = malformed;
    let record = history.records.values_mut().next().unwrap();
    record.state = SyncState::Cancelled;
    record.rolled_back = true;
    record.reconciled = true;
    let state = run(store.catalog()).unwrap().state.unwrap();
    store
        .apply_sync_plan(SyncPlan {
            transition: SyncTransition::Normal,
            expected_model: state.clone(),
            next_model: state,
            expected_sync: None,
            next_sync: history.to_rows().unwrap(),
            guards: vec![],
            mutations: vec![],
        })
        .unwrap();
    assert!(
        !SyncLedger::from_rows(&store.load_sync().unwrap().unwrap())
            .unwrap()
            .has_live_recovery()
    );
}
