//! SPEC-0049 / TC-0053: raw full-plan validation retains format and alias authority.
use hoyadb_core::{
    command::{
        records::{SyncHeader, SyncLedger},
        *,
    },
    model::*,
    storage::*,
};
use hoyadb_storage_memory::MemoryStorage;
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    task::{Context, Poll, Waker},
};

fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("memory fixture unexpectedly suspended"),
    }
}

fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([(
            "notes".into(),
            TableDefinition {
                columns: BTreeMap::from([(
                    "body".into(),
                    Column {
                        kind: ColumnType::String,
                        optional: false,
                    },
                )]),
                ..Default::default()
            },
        )]),
    }
}

fn config() -> EntityConfig {
    EntityConfig {
        scope: "format-account".into(),
        tables: BTreeSet::from(["notes".into()]),
    }
}

fn provider() -> Result<EntityContext, CommandError> {
    Ok(EntityContext {
        identity: Json::Null,
        now_ms: 1000,
    })
}

fn open(store: MemoryStorage, history: usize) -> CommandDatabase<MemoryStorage> {
    run(CommandDatabase::open(
        store,
        schema(),
        vec![],
        [0x64; 16],
        SyncOptions {
            history,
            ..Default::default()
        },
    ))
    .unwrap()
}

fn fields() -> BTreeMap<String, Value> {
    BTreeMap::from([("body".into(), Value::String("kept".into()))])
}

fn accepted_fixture(history: usize, after_reset: bool) -> MemoryStorage {
    let mut db = open(MemoryStorage::default(), history);
    db.configure_entities_durable(config(), provider).unwrap();
    if after_reset {
        run(db.reset(ResetMode::DiscardRecovery)).unwrap();
    }
    let row = run(db.insert("notes", fields())).unwrap();
    let batch = db.prepare_push(1000, 0).unwrap();
    let Json::Object(envelope) = Json::parse(&batch.envelopes[0]).unwrap() else {
        panic!("entity envelope");
    };
    let Json::String(key) = &envelope["writeId"] else {
        panic!("write ID")
    };
    db.receive_entity(
        "notes",
        &row.id,
        key,
        Some(RemoteOutcome::Applied(Json::Null)),
    )
    .unwrap();
    run(db.reconcile(
        key,
        ReconcilePlan {
            mappings: BTreeMap::from([(row.id, "server-note".into())]),
            writes: vec![],
        },
    ))
    .unwrap();
    db.into_storage()
}

fn unchanged(store: &MemoryStorage) -> SyncPlan {
    let model = run(store.catalog()).unwrap().state.unwrap();
    let rows = store.load_sync().unwrap().unwrap();
    SyncPlan {
        transition: SyncTransition::Normal,
        expected_model: model.clone(),
        next_model: model,
        expected_sync: Some(rows.clone()),
        next_sync: rows,
        guards: vec![],
        mutations: vec![],
    }
}

fn data(row: &Row) -> BTreeMap<String, Json> {
    let Value::String(data) = &row.fields["data"] else {
        panic!("packet data")
    };
    let Json::Object(data) = Json::parse(data).unwrap() else {
        panic!("packet object")
    };
    data
}

fn edit_data(row: &mut Row, edit: impl FnOnce(&mut BTreeMap<String, Json>)) {
    let mut value = data(row);
    edit(&mut value);
    row.fields.insert(
        "data".into(),
        Value::String(Json::Object(value).canonical().unwrap()),
    );
}

fn header(rows: &mut [Row]) -> &mut Row {
    rows.iter_mut()
        .find(|row| row.id == "entity:state")
        .unwrap()
}

fn advance_revision(rows: &mut [Row]) {
    edit_data(header(rows), |data| {
        let Json::Integer(revision) = data.get_mut("revision").unwrap() else {
            panic!("revision")
        };
        *revision += 1;
    });
}

fn state(store: &MemoryStorage) -> (Option<Vec<Row>>, Option<ModelState>, Vec<Row>) {
    (
        store.load_sync().unwrap(),
        run(store.catalog()).unwrap().state,
        run(store.scan("notes")).unwrap(),
    )
}

#[track_caller]
fn refused_unchanged(store: &mut MemoryStorage, plan: SyncPlan, expected_message: Option<&str>) {
    let before = state(store);
    let error = store.apply_sync_plan(plan).unwrap_err();
    if let Some(expected) = expected_message {
        let StorageError::SchemaConflict {
            message,
        } = error
        else {
            panic!("expected schema conflict")
        };
        assert_eq!(message, expected);
    } else {
        assert!(matches!(error, StorageError::InvalidRow { table, id, .. }
            if table == "notes" && id == "server-note"));
    }
    assert_eq!(
        state(store),
        before,
        "rejected plan changed model, packets or application rows"
    );
}

#[test]
fn d05_empty_format_four_cannot_be_downgraded_by_changing_only_its_marker() {
    let mut db = open(MemoryStorage::default(), 100);
    db.configure_entities_durable(config(), provider).unwrap();
    let mut store = db.into_storage();
    let mut plan = unchanged(&store);
    assert!(
        !plan
            .next_sync
            .iter()
            .any(|row| row.id.starts_with("entity:alias:"))
    );
    edit_data(header(&mut plan.next_sync), |data| {
        assert_eq!(
            data.insert("ledger_format".into(), Json::Integer(3)),
            Some(Json::Integer(4))
        );
    });
    // The projected legacy ledger is valid; the stored-format boundary rejects the transition.
    assert!(SyncLedger::from_rows(&plan.next_sync).is_ok());
    refused_unchanged(
        &mut store,
        plan,
        Some("entity ledger format cannot change implicitly"),
    );
}

#[test]
fn d05_accepted_alias_cannot_be_removed_or_retargeted_after_history_eviction() {
    let mut store = accepted_fixture(0, false);
    let original = unchanged(&store);
    assert!(
        !original
            .next_sync
            .iter()
            .any(|row| row.id.starts_with("command:"))
    );
    for retarget in [false, true] {
        let mut plan = original.clone();
        if retarget {
            let alias = plan
                .next_sync
                .iter_mut()
                .find(|row| row.id.starts_with("entity:alias:"))
                .unwrap();
            edit_data(alias, |data| {
                data.insert("canonical_id".into(), Json::String("different".into()));
            });
        } else {
            plan.next_sync
                .retain(|row| !row.id.starts_with("entity:alias:"));
        }
        advance_revision(&mut plan.next_sync);
        refused_unchanged(
            &mut store,
            plan,
            Some("accepted entity aliases are immutable"),
        );
    }
}

#[test]
fn d05_alias_only_state_cannot_be_rebound_to_another_account() {
    let mut store = accepted_fixture(0, true);
    let row = run(store.get("notes", "server-note")).unwrap().unwrap();
    let mut cleanup = unchanged(&store);
    cleanup
        .next_sync
        .retain(|row| !row.id.starts_with("entity:metadata:"));
    advance_revision(&mut cleanup.next_sync);
    cleanup.guards.push(RowGuard {
        table: "notes".into(),
        id: row.id.clone(),
        expected: Some(row.clone()),
    });
    cleanup.mutations.push(Mutation::Remove {
        table: "notes".into(),
        id: row.id,
    });
    store.apply_sync_plan(cleanup).unwrap();
    assert!(run(store.scan("notes")).unwrap().is_empty());
    let mut plan = unchanged(&store);
    assert!(
        plan.next_sync
            .iter()
            .any(|row| row.id.starts_with("entity:alias:"))
    );
    // Apart from the alias, this is the empty post-reset shell accepted by legacy rebinding.
    assert_eq!(
        data(header(&mut plan.next_sync))["generation"],
        Json::Integer(2)
    );
    store.apply_sync_plan(plan.clone()).unwrap();
    edit_data(header(&mut plan.next_sync), |data| {
        let Json::Object(config) = data.get_mut("config").unwrap() else {
            panic!("config")
        };
        config.insert("scope".into(), Json::String("another-account".into()));
    });
    refused_unchanged(
        &mut store,
        plan,
        Some("entity reconfiguration cannot retain aliases"),
    );
}

#[test]
fn d05_alias_source_cannot_claim_an_unallocated_model_identity() {
    use std::fmt::Write;
    let mut store = accepted_fixture(0, false);
    let mut plan = unchanged(&store);
    let unallocated = format!(
        "entity:{}:{:016x}",
        "64".repeat(16),
        plan.next_model.next_id
    );
    let alias = plan
        .next_sync
        .iter_mut()
        .find(|row| row.id.starts_with("entity:alias:"))
        .unwrap();
    edit_data(alias, |data| {
        data.insert("local_id".into(), Json::String(unallocated.clone()));
    });
    // Keep the packet's identity valid so the model-allocation check is the rejecting rule.
    let mut identity = String::from("entity:alias:");
    for byte in (5_u32.to_be_bytes())
        .into_iter()
        .chain("notes".bytes())
        .chain(unallocated.bytes())
    {
        write!(identity, "{byte:02x}").unwrap();
    }
    alias.id = identity;
    advance_revision(&mut plan.next_sync);
    refused_unchanged(
        &mut store,
        plan,
        Some("durable alias exceeds allocated model identities"),
    );
}

#[test]
fn d05_format_four_keeps_frozen_envelopes_immutable() {
    let mut store = accepted_fixture(100, false);
    let mut plan = unchanged(&store);
    let command = plan
        .next_sync
        .iter_mut()
        .find(|row| row.id.starts_with("command:"))
        .unwrap();
    edit_data(command, |data| {
        let Json::Object(args) = data.get_mut("args").unwrap() else {
            panic!("args")
        };
        let Json::Object(changes) = args.get_mut("changes").unwrap() else {
            panic!("changes")
        };
        changes.insert("body".into(), Json::String("rewritten".into()));
        let Json::String(frozen) = &data["frozen"] else {
            panic!("frozen envelope")
        };
        let Json::Object(mut envelope) = Json::parse(frozen).unwrap() else {
            panic!("envelope")
        };
        let Json::Object(changes) = envelope.get_mut("changes").unwrap() else {
            panic!("changes")
        };
        changes.insert("body".into(), Json::String("rewritten".into()));
        data.insert(
            "frozen".into(),
            Json::String(Json::Object(envelope).canonical().unwrap()),
        );
    });
    refused_unchanged(
        &mut store,
        plan,
        Some("frozen command envelope cannot change"),
    );
}

#[test]
fn d05_invalid_application_row_and_stale_guard_leave_all_state_unchanged() {
    let mut store = accepted_fixture(0, false);
    let row = run(store.get("notes", "server-note")).unwrap().unwrap();
    for corrupt_type in [true, false] {
        let mut plan = unchanged(&store);
        advance_revision(&mut plan.next_sync);
        plan.next_model.next_id += 1;
        let mut expected = row.clone();
        let mut replacement = row.clone();
        if corrupt_type {
            replacement.fields.insert("body".into(), Value::Integer(42));
        } else {
            expected
                .fields
                .insert("body".into(), Value::String("stale image".into()));
            replacement
                .fields
                .insert("body".into(), Value::String("attempted write".into()));
        }
        plan.guards.push(RowGuard {
            table: "notes".into(),
            id: row.id.clone(),
            expected: Some(expected),
        });
        plan.mutations.push(Mutation::Replace {
            table: "notes".into(),
            row: replacement,
        });
        refused_unchanged(
            &mut store,
            plan,
            if corrupt_type {
                None
            } else {
                Some(GUARD_CONFLICT)
            },
        );
    }
}

#[test]
fn d03_legacy_two_and_three_preserve_packet_bytes_on_open_and_refused_opt_in() {
    for format in [2, 3] {
        let mut store = open(MemoryStorage::default(), 100).into_storage();
        let model = run(store.catalog()).unwrap().state.unwrap();
        let mut entity = EntityLedger::new(config());
        entity.ledger_format = format;
        let ledger = SyncLedger {
            entity: Some(entity),
            header: SyncHeader {
                namespace: "64".repeat(16),
                sequence: 1,
                last_ms: 0,
                options: SyncOptions::default(),
            },
            records: BTreeMap::new(),
        };
        store
            .apply_sync_plan(SyncPlan {
                transition: SyncTransition::Normal,
                expected_model: model.clone(),
                next_model: model,
                expected_sync: None,
                next_sync: ledger.to_rows().unwrap(),
                guards: vec![],
                mutations: vec![],
            })
            .unwrap();
        let mut db = open(store, 100);
        db.configure_entities(config(), provider).unwrap();
        run(db.insert("notes", fields())).unwrap();
        let store = db.into_storage();
        let before = state(&store);
        let mut db = open(store, 100);
        db.configure_entities(config(), provider).unwrap();
        assert_eq!(
            db.configure_entities_durable(config(), provider),
            Err(CommandError::InvalidCommand)
        );
        let store = db.into_storage();
        assert_eq!(state(&store), before);
        let decoded = SyncLedger::from_rows(&store.load_sync().unwrap().unwrap()).unwrap();
        assert_eq!(decoded.entity.as_ref().unwrap().ledger_format, format);
    }
}

#[test]
fn d05_new_alias_requires_a_previously_accepted_entity_producer() {
    use std::fmt::Write;
    let mut store = accepted_fixture(0, false);
    let mut plan = unchanged(&store);
    let local = format!(
        "entity:{}:{:016x}",
        "64".repeat(16),
        plan.next_model.next_id
    );
    plan.next_model.next_id += 1;
    let mut alias = plan
        .next_sync
        .iter()
        .find(|row| row.id.starts_with("entity:alias:"))
        .unwrap()
        .clone();
    edit_data(&mut alias, |data| {
        data.insert("local_id".into(), Json::String(local.clone()));
        data.insert("canonical_id".into(), Json::String("forged-target".into()));
    });
    let mut identity = String::from("entity:alias:");
    for byte in 5_u32
        .to_be_bytes()
        .into_iter()
        .chain("notes".bytes())
        .chain(local.bytes())
    {
        write!(identity, "{byte:02x}").unwrap();
    }
    alias.id = identity;
    plan.next_sync.push(alias);
    advance_revision(&mut plan.next_sync);
    refused_unchanged(
        &mut store,
        plan,
        Some("new durable alias requires an accepted entity producer"),
    );
}
