//! SPEC-0049 / TC-0053: durable alias runtime cases.
#[path = "../../../tests/conformance/durable_aliases.rs"]
mod cases;
use cases::*;
use hoyadb_core::{command::*, model::Write, storage::*};
use hoyadb_storage_memory::MemoryStorage;
use std::collections::BTreeMap;

#[test]
fn d01_durable_alias_survives_forget_and_restart() {
    let mut db = open(MemoryStorage::default(), 100);
    configure_durable(&mut db);
    let (original, key) = accepted_create(&mut db);
    assert_eq!(
        db.normalize_entity_target("notes", &original).unwrap(),
        "server-note"
    );
    db.forget(&key).unwrap();
    let mut reopened = open(db.into_storage(), 100);
    configure_durable(&mut reopened);
    assert_eq!(
        run(reopened.query("notes").fetch(&reopened)).unwrap()[0].id,
        "server-note"
    );
    assert_eq!(
        reopened
            .normalize_entity_target("notes", &original)
            .unwrap(),
        "server-note",
        "external IDs must resolve after their completed producer is forgotten"
    );
}

#[test]
fn d01_durable_alias_survives_zero_history_restart() {
    let mut db = open(MemoryStorage::default(), 0);
    configure_durable(&mut db);
    let (original, _) = accepted_create(&mut db);
    assert!(db.queue_status().commands.is_empty());
    let mut reopened = open(db.into_storage(), 0);
    configure_durable(&mut reopened);
    assert_eq!(
        reopened
            .normalize_entity_target("notes", &original)
            .unwrap(),
        "server-note",
        "automatic history eviction must not erase accepted aliases"
    );
}

#[test]
fn d03_ordinary_configuration_keeps_public_format_three_reader() {
    let mut db = open(MemoryStorage::default(), 100);
    db.configure_entities(config(), provider).unwrap();
    let store = db.into_storage();
    let packets = store.load_sync().unwrap().unwrap();
    let ledger = hoyadb_core::command::records::SyncLedger::from_rows(&packets).unwrap();
    assert_eq!(ledger.entity.as_ref().unwrap().ledger_format, 3);
    assert_eq!(ledger.to_rows().unwrap(), packets);
}

#[test]
fn d04_seeded_rows_are_allowed_and_reset_rebind_isolates_aliases() {
    let mut definition = schema();
    definition.tables.insert(
        "migration_markers".into(),
        definition.tables["notes"].clone(),
    );
    let mut db = run(CommandDatabase::open(
        MemoryStorage::default(),
        definition.clone(),
        vec![],
        [0x53; 16],
        SyncOptions {
            history: 0,
            ..Default::default()
        },
    ))
    .unwrap();
    db.set_id_generator(|| "01JA0000000000000000000000".into())
        .unwrap();
    let legacy = run(db.insert(
        "notes",
        BTreeMap::from([("body".into(), Value::String("legacy".into()))]),
    ))
    .unwrap();
    run(db.insert(
        "migration_markers",
        BTreeMap::from([("body".into(), Value::String("cutover-seeded".into()))]),
    ))
    .unwrap();
    configure_durable(&mut db);
    assert_eq!(
        run(db.query("migration_markers").fetch(&db)).unwrap().len(),
        1
    );
    assert_eq!(
        db.resolve_entity_id("notes", &legacy.id).unwrap(),
        legacy.id
    );
    let (original, _) = accepted_create(&mut db);
    assert_eq!(
        db.resolve_entity_id("notes", &original).unwrap(),
        "server-note"
    );
    run(db.reset(ResetMode::DiscardRecovery)).unwrap();
    assert_eq!(db.resolve_entity_id("notes", &original).unwrap(), original);
    let mut next_config = config();
    next_config.scope = "another-account".into();
    run(db.reconfigure_entities(next_config.clone(), provider)).unwrap();
    let mut reopened = run(CommandDatabase::open(
        db.into_storage(),
        definition,
        vec![],
        [0x53; 16],
        SyncOptions {
            history: 0,
            ..Default::default()
        },
    ))
    .unwrap();
    reopened
        .configure_entities_durable(next_config, provider)
        .unwrap();
    assert_eq!(
        reopened.resolve_entity_id("notes", &original).unwrap(),
        original
    );
    assert!(
        run(reopened.query("notes").fetch(&reopened))
            .unwrap()
            .is_empty()
    );
    assert!(reopened.resolve_entity_id("missing", &original).is_err());
}

#[test]
fn d03_existing_format_three_cannot_opt_in_or_silently_rewrite() {
    let mut db = open(MemoryStorage::default(), 100);
    db.configure_entities(config(), provider).unwrap();
    let store = db.into_storage();
    let before = store.load_sync().unwrap().unwrap();
    let mut db = open(store, 100);
    assert_eq!(
        db.configure_entities_durable(config(), provider),
        Err(CommandError::InvalidCommand)
    );
    assert_eq!(db.into_storage().load_sync().unwrap().unwrap(), before);
}

#[test]
fn d03_public_legacy_reader_explicitly_rejects_opt_in_format_four() {
    let mut db = open(MemoryStorage::default(), 100);
    configure_durable(&mut db);
    let rows = db.into_storage().load_sync().unwrap().unwrap();
    assert!(hoyadb_core::command::records::SyncLedger::from_rows(&rows).is_err());
}

#[test]
fn d02_stale_alias_cannot_mutate_another_table() {
    let mut definition = schema();
    definition
        .tables
        .insert("other".into(), definition.tables["notes"].clone());
    let mut db = run(CommandDatabase::open(
        MemoryStorage::default(),
        definition,
        vec![],
        [0x53; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    db.set_id_generator(|| "server-note".into()).unwrap();
    run(db.insert(
        "other",
        BTreeMap::from([("body".into(), Value::String("untouched".into()))]),
    ))
    .unwrap();
    let mut configured = config();
    configured.tables.insert("other".into());
    db.configure_entities_durable(configured, provider).unwrap();
    let (original, key) = accepted_create(&mut db);
    db.forget(&key).unwrap();
    assert!(db.resolve_entity_id("other", &original).is_err());
    assert!(
        run(db.patch(
            "other",
            &original,
            BTreeMap::from([("body".into(), Value::String("wrong".into()))])
        ))
        .is_err()
    );
    assert!(
        run(db.apply_entity_batch_aliased(vec![AliasedWrite {
            write: Write::Remove {
                table: "other".into(),
                id: original.clone()
            },
            patch: false,
            alias: None,
            proposed_id: None,
            references: BTreeMap::new(),
        }]))
        .is_err()
    );
    assert_eq!(
        run(db.query("other").fetch(&db)).unwrap()[0].fields["body"],
        Value::String("untouched".into())
    );
    let patched = run(db.patch(
        "notes",
        &original,
        BTreeMap::from([("body".into(), Value::String("updated".into()))]),
    ))
    .unwrap();
    assert_eq!(patched.id, "server-note");
}

#[test]
fn d05_failed_reconcile_rolls_back_full_and_delta() {
    for ownership in [StoreOwnership::Exclusive, StoreOwnership::Unproven] {
        failed_reconcile_is_atomic(MemoryStorage::default(), ownership);
    }
}

#[test]
fn d07_alias_commit_does_not_encode_prior_alias_inventory() {
    fn measured(count: usize) -> counters::WorkSnapshot {
        let mut db = open(MemoryStorage::default(), 0);
        configure_durable(&mut db);
        for i in 0..count {
            accepted_create_as(&mut db, &format!("server-{i:06}"));
        }
        counters::reset();
        accepted_create_as(&mut db, "server-measured");
        counters::snapshot()
    }
    let small = measured(10);
    let large = measured(1000);
    assert_eq!(
        small.packets_encoded, large.packets_encoded,
        "{small:?} {large:?}"
    );
    assert_eq!(
        small.packets_parsed, large.packets_parsed,
        "{small:?} {large:?}"
    );
    assert_eq!(small.rows_cloned, large.rows_cloned, "{small:?} {large:?}");
    assert_eq!(
        small.metadata_entries_visited, large.metadata_entries_visited,
        "{small:?} {large:?}"
    );
    assert_eq!(large.inventory_rows_validated, 0, "{large:?}");
}

#[test]
fn d02_canonical_identity_is_not_reinterpreted_as_another_tables_alias() {
    let mut definition = schema();
    definition
        .tables
        .insert("other".into(), definition.tables["notes"].clone());
    let mut db = run(CommandDatabase::open(
        MemoryStorage::default(),
        definition,
        vec![],
        [0x53; 16],
        SyncOptions {
            history: 0,
            ..Default::default()
        },
    ))
    .unwrap();
    let mut configured = config();
    configured.tables.insert("other".into());
    db.configure_entities_durable(configured, provider).unwrap();
    let canonical_note = format!("entity:{}:0000000000000002", "53".repeat(16));
    let (original_note, _) = accepted_create_as(&mut db, &canonical_note);
    let other = run(db.insert(
        "other",
        BTreeMap::from([("body".into(), Value::String("other".into()))]),
    ))
    .unwrap();
    assert_eq!(other.id, canonical_note);
    let key = db.queue_status().commands[0].key.clone();
    db.prepare_push(1000, 0).unwrap();
    db.receive_entity(
        "other",
        &other.id,
        &key,
        Some(RemoteOutcome::Applied(Json::Null)),
    )
    .unwrap();
    run(db.reconcile(
        &key,
        ReconcilePlan {
            mappings: BTreeMap::from([(other.id.clone(), "server-other".into())]),
            writes: vec![],
        },
    ))
    .unwrap();
    assert!(db.queue_status().commands.is_empty());
    assert_eq!(
        db.resolve_entity_id("other", &other.id).unwrap(),
        "server-other"
    );
    assert_eq!(
        db.resolve_entity_id("notes", &original_note).unwrap(),
        canonical_note
    );
    assert_eq!(
        db.resolve_entity_id("notes", &canonical_note).unwrap(),
        canonical_note
    );
    let patched = run(db.patch(
        "notes",
        &original_note,
        BTreeMap::from([("body".into(), Value::String("updated".into()))]),
    ))
    .unwrap();
    assert_eq!(patched.id, canonical_note);
}

#[test]
fn d03_ordinary_configuration_rejects_retained_durable_mode() {
    ordinary_configuration_rejects_retained_durable_mode(MemoryStorage::default());
}

#[test]
fn d02_reference_cells_resolve_through_their_target_table() {
    reference_cells_resolve_through_their_target_table(MemoryStorage::default());
}

#[test]
fn d04_adoption_refuses_reserved_local_rows() {
    adoption_refuses_reserved_local_rows(MemoryStorage::default());
}
