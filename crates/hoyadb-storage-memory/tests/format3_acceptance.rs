//! SPEC-0037 R10 / TC-0041 SI-F01..04. Task-owned public API acceptance only.
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
        Poll::Pending => panic!("memory adapter unexpectedly suspended"),
    }
}
fn object<const N: usize>(items: [(&str, Json); N]) -> Json {
    Json::Object(items.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
fn text(value: &str) -> Json {
    Json::String(value.into())
}
fn packet(id: &str, data: Json) -> Row {
    Row {
        id: id.into(),
        fields: BTreeMap::from([
            ("format".into(), Value::Integer(2)),
            ("data".into(), Value::String(data.canonical().unwrap())),
        ]),
    }
}
fn data(row: &Row) -> BTreeMap<String, Json> {
    let Value::String(value) = &row.fields["data"] else {
        panic!("string data")
    };
    let Json::Object(fields) = Json::parse(value).unwrap() else {
        panic!("object data")
    };
    fields
}
fn edit_data(row: &mut Row, edit: impl FnOnce(&mut BTreeMap<String, Json>)) {
    let mut fields = data(row);
    edit(&mut fields);
    row.fields.insert(
        "data".into(),
        Value::String(Json::Object(fields).canonical().unwrap()),
    );
}
fn header_rows() -> Vec<Row> {
    SyncLedger {
        entity: None,
        header: SyncHeader {
            namespace: "31".repeat(16),
            sequence: 1,
            last_ms: 0,
            options: SyncOptions::default(),
        },
        records: BTreeMap::new(),
    }
    .to_rows()
    .unwrap()
}
fn state(format: u8) -> Row {
    let mut row = packet(
        "entity:state",
        object([
            (
                "config",
                object([
                    ("scope", text("scope-a")),
                    ("tables", Json::Array(vec![text("notes")])),
                ]),
            ),
            ("generation", Json::Integer(1)),
            ("revision", Json::Integer(0)),
            ("checkpoint", Json::Null),
            ("ready", Json::Bool(false)),
            ("pages", Json::Integer(0)),
            ("changes", Json::Integer(0)),
        ]),
    );
    if format == 3 {
        edit_data(&mut row, |d| {
            d.insert("ledger_format".into(), Json::Integer(3));
        });
    }
    row
}
fn identity(table: &str, id: &str) -> String {
    use std::fmt::Write;
    let mut bytes = u32::try_from(table.len()).unwrap().to_be_bytes().to_vec();
    bytes.extend_from_slice(table.as_bytes());
    bytes.extend_from_slice(id.as_bytes());
    let mut out = String::from("entity:metadata:");
    for byte in bytes {
        write!(out, "{byte:02x}").unwrap();
    }
    out
}
fn metadata(table: &str, id: &str, packet_id: String) -> Row {
    packet(
        &packet_id,
        object([
            ("table", text(table)),
            ("record_id", text(id)),
            (
                "metadata",
                object([("fields", object([])), ("tombstone", Json::Null)]),
            ),
        ]),
    )
}
fn fixture(format: u8, identities: &[(&str, &str)]) -> Vec<Row> {
    let mut rows = header_rows();
    rows.push(state(format));
    for (ordinal, (table, id)) in identities.iter().enumerate() {
        let name = if format == 3 {
            identity(table, id)
        } else {
            format!("entity:metadata:{ordinal:016x}")
        };
        rows.push(metadata(table, id, name));
    }
    rows
}
fn assert_format(rows: &[Row], format: u8) {
    let ledger = SyncLedger::from_rows(rows).expect("valid persisted ledger must decode");
    let encoded = ledger.to_rows().unwrap();
    let state = encoded.iter().find(|r| r.id == "entity:state").unwrap();
    assert_eq!(
        data(state).get("ledger_format"),
        if format == 3 {
            Some(&Json::Integer(3))
        } else {
            None
        }
    );
    let metadata_rows: Vec<_> = encoded
        .iter()
        .filter(|r| r.id.starts_with("entity:metadata:"))
        .collect();
    for (ordinal, row) in metadata_rows.iter().enumerate() {
        assert_eq!(
            row.fields["format"],
            Value::Integer(2),
            "R10 retains the entity packet envelope"
        );
        let d = data(row);
        let (Json::String(table), Json::String(id)) = (&d["table"], &d["record_id"]) else {
            panic!("identity text")
        };
        assert_eq!(
            row.id,
            if format == 3 {
                identity(table, id)
            } else {
                format!("entity:metadata:{ordinal:016x}")
            }
        );
    }
    assert!(SyncLedger::from_rows(&encoded).unwrap() == ledger);
    if format == 3 {
        assert!(
            encoded.windows(2).all(|pair| pair[0].id < pair[1].id),
            "format3 to_rows sorts all packet IDs"
        );
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
fn config(scope: &str) -> EntityConfig {
    EntityConfig {
        scope: scope.into(),
        tables: BTreeSet::from(["notes".into()]),
    }
}
fn provider() -> Result<EntityContext, CommandError> {
    Ok(EntityContext {
        identity: Json::Null,
        now_ms: 100,
    })
}
fn open(store: MemoryStorage) -> CommandDatabase<MemoryStorage> {
    run(CommandDatabase::open(
        store,
        schema(),
        vec![],
        [0x99; 16],
        SyncOptions::default(),
    ))
    .unwrap()
}
fn seed(format: Option<u8>) -> MemoryStorage {
    let mut store = run(Database::open(MemoryStorage::default(), schema(), vec![]))
        .unwrap()
        .into_storage();
    let model = run(store.catalog()).unwrap().state.unwrap();
    let rows = format.map_or_else(header_rows, |v| fixture(v, &[]));
    store
        .apply_sync_plan(SyncPlan {
            transition: SyncTransition::Normal,
            expected_model: model.clone(),
            next_model: model,
            expected_sync: None,
            next_sync: rows,
            guards: vec![],
            mutations: vec![],
        })
        .unwrap();
    store
}
fn checkpoint(
    db: CommandDatabase<MemoryStorage>,
    format: u8,
    scope: &str,
) -> CommandDatabase<MemoryStorage> {
    let store = db.into_storage();
    let rows = store.load_sync().unwrap().unwrap();
    assert_format(&rows, format);
    let mut db = open(store);
    db.configure_entities(config(scope), provider).unwrap();
    db
}
fn pull(db: &mut CommandDatabase<MemoryStorage>, checkpoint: &str) {
    let request = db.begin_pull().unwrap().request;
    run(db.apply_pull(
        request,
        PullPage {
            checkpoint: checkpoint.into(),
            complete: true,
            changes: vec![EntityChange {
                table: "notes".into(),
                record_id: "server:note".into(),
                op: EntityOperation::Update,
                changes: BTreeMap::from([("body".into(), Value::String("remote".into()))]),
                removed_fields: BTreeSet::new(),
                timestamp_ms: 10,
                field_timestamps: BTreeMap::from([("body".into(), 10)]),
            }],
        },
    ))
    .unwrap();
}
fn retention(format: u8) {
    let mut db = open(seed(Some(format)));
    db.configure_entities(config("scope-a"), provider).unwrap();
    db = checkpoint(db, format, "scope-a");
    pull(&mut db, "first");
    db = checkpoint(db, format, "scope-a");
    run(db.reset(ResetMode::PreserveRecovery)).unwrap();
    db = checkpoint(db, format, "scope-a");
    assert!(run(db.query("notes").fetch(&db)).unwrap().is_empty());
    run(db.reconfigure_entities(config("scope-b"), provider)).unwrap();
    db = checkpoint(db, format, "scope-b");
    pull(&mut db, "second");
    run(db.patch(
        "notes",
        "server:note",
        BTreeMap::from([("body".into(), Value::String("local".into()))]),
    ))
    .unwrap();
    db = checkpoint(db, format, "scope-b");
    assert_eq!(db.queue_status().pending, 1);
    run(db.reset(ResetMode::DiscardRecovery)).unwrap();
    db = checkpoint(db, format, "scope-b");
    assert_eq!(db.queue_status().pending, 0);
    assert!(run(db.query("notes").fetch(&db)).unwrap().is_empty());
    run(db.reconfigure_entities(config("scope-c"), provider)).unwrap();
    drop(checkpoint(db, format, "scope-c"));
}

#[test]
fn si_f01_first_entity_config_uses_v3_and_reloads_identity_packets() {
    let mut db = open(MemoryStorage::default());
    db.configure_entities(config("scope-a"), provider).unwrap();
    db = checkpoint(db, 3, "scope-a");
    pull(&mut db, "first");
    drop(checkpoint(db, 3, "scope-a"));
}
#[test]
fn si_f01_command_only_persisted_ledger_adopts_v3() {
    let mut db = open(seed(None));
    db.configure_entities(config("scope-a"), provider).unwrap();
    drop(checkpoint(db, 3, "scope-a"));
}
#[test]
fn si_f02_existing_v2_retains_format_across_owner_operations() {
    retention(2);
}
#[test]
fn si_f02_existing_v3_retains_format_across_owner_operations() {
    retention(3);
}
fn comparison_state_cannot_disappear(format: u8) {
    let mut store = seed(Some(format));
    let before = store.load_sync().unwrap();
    let model = run(store.catalog()).unwrap().state.unwrap();
    assert!(
        store
            .apply_sync_plan(SyncPlan {
                transition: SyncTransition::Normal,
                expected_model: model.clone(),
                next_model: model,
                expected_sync: before.clone(),
                next_sync: header_rows(),
                guards: vec![],
                mutations: vec![]
            })
            .is_err()
    );
    assert_eq!(store.load_sync().unwrap(), before);
}
#[test]
fn si_f03_v3_canonical_utf8_identity_roundtrip_and_packet_order() {
    let rows = fixture(3, &[("ab", "c"), ("a", "bc"), ("nötés", "é😀")]);
    assert_format(&rows, 3);
    let mut reversed = rows;
    reversed.reverse();
    assert_format(&reversed, 3);
    let ledger = SyncLedger::from_rows(&reversed).unwrap();
    assert_eq!(ledger.entity.unwrap().metadata.len(), 3);
}
#[test]
fn si_f03_v3_malformed_identity_packets_reject_after_valid_control() {
    let valid = fixture(3, &[("notes", "record")]);
    assert_format(&valid, 3);
    let suffix = valid
        .last()
        .unwrap()
        .id
        .strip_prefix("entity:metadata:")
        .unwrap();
    let invalid = vec![
        suffix.to_uppercase(),
        suffix[..suffix.len() - 1].into(),
        format!("g{}", &suffix[1..]),
        "000000".into(),
        "ffffffff61".into(),
        "00000001ff61".into(),
        "0000000161ff".into(),
        "0000000061".into(),
        "0000000161".into(),
        "0000000000000000".into(),
    ];
    for value in invalid {
        let mut rows = valid.clone();
        rows.last_mut().unwrap().id = format!("entity:metadata:{value}");
        assert!(
            SyncLedger::from_rows(&rows).is_err(),
            "must reject malformed suffix"
        );
    }
    let mut mismatch = valid.clone();
    edit_data(mismatch.last_mut().unwrap(), |d| {
        d.insert("record_id".into(), text("different"));
    });
    assert!(SyncLedger::from_rows(&mismatch).is_err());
    for table in ["", "id", "a.b", "__hoya_reserved"] {
        let rows = fixture(3, &[(table, "record")]);
        assert!(SyncLedger::from_rows(&rows).is_err(), "invalid table name");
    }
    let mut duplicate = valid.clone();
    duplicate.push(valid.last().unwrap().clone());
    assert!(SyncLedger::from_rows(&duplicate).is_err());
    let mut extra = valid.clone();
    edit_data(extra.last_mut().unwrap(), |d| {
        d.insert("extra".into(), Json::Null);
    });
    assert!(SyncLedger::from_rows(&extra).is_err());
}
#[test]
fn si_f03_v3_marker_exact_keys_and_mixed_formats_reject_after_valid_control() {
    let valid = fixture(3, &[("notes", "record")]);
    assert_format(&valid, 3);
    for marker in [Json::Null, text("3"), Json::Integer(2), Json::Integer(4)] {
        let mut rows = valid.clone();
        edit_data(&mut rows[1], |d| {
            d.insert("ledger_format".into(), marker);
        });
        assert!(SyncLedger::from_rows(&rows).is_err());
    }
    let mut extra = valid.clone();
    edit_data(&mut extra[1], |d| {
        d.insert("extra".into(), Json::Null);
    });
    assert!(SyncLedger::from_rows(&extra).is_err());
    let mut envelope = valid.clone();
    envelope[1]
        .fields
        .insert("format".into(), Value::Integer(3));
    assert!(SyncLedger::from_rows(&envelope).is_err());
    let mut mixed = valid.clone();
    mixed.push(metadata(
        "notes",
        "other",
        "entity:metadata:0000000000000000".into(),
    ));
    assert!(SyncLedger::from_rows(&mixed).is_err());
}
#[test]
fn si_f03_v2_rejects_identity_packets_control() {
    let valid = fixture(2, &[("notes", "record")]);
    assert_format(&valid, 2);
    let mut mixed = valid;
    mixed.last_mut().unwrap().id = identity("notes", "record");
    assert!(SyncLedger::from_rows(&mixed).is_err());
}
#[test]
fn si_f03_v3_raw_suffix_boundary_rejects_without_panic_after_valid_control() {
    let valid = fixture(3, &[("notes", "record")]);
    assert_format(&valid, 3);
    // Private json::MAX_BYTES is the documented 1 MiB frame bound, not a new limit.
    let bound = 2 * (4 + 1_048_576usize);
    for size in [bound, bound + 1, bound + 2] {
        let mut rows = valid.clone();
        rows.last_mut().unwrap().id = format!("entity:metadata:{}", "0".repeat(size));
        assert!(SyncLedger::from_rows(&rows).is_err());
    }
}
#[test]
fn si_f04_length_prefixed_identity_is_injective_and_preserves_leading_zeros() {
    assert_eq!(identity("ab", "c"), "entity:metadata:00000002616263");
    assert_eq!(identity("a", "bc"), "entity:metadata:00000001616263");
    assert_ne!(identity("ab", "c"), identity("a", "bc"));
    let rows = fixture(3, &[("ab", "c"), ("a", "bc")]);
    let ledger = SyncLedger::from_rows(&rows).expect("both canonical identities decode");
    assert_eq!(ledger.entity.as_ref().unwrap().metadata.len(), 2);
    assert_format(&ledger.to_rows().unwrap(), 3);
}

#[test]
fn si_f03_legacy_payload_twins_validate_metadata_fixture_controls() {
    let rows = fixture(2, &[("a", "bc"), ("ab", "c"), ("nötés", "é😀")]);
    assert_format(&rows, 2);
    assert_eq!(
        SyncLedger::from_rows(&rows)
            .unwrap()
            .entity
            .unwrap()
            .metadata
            .len(),
        3
    );
}

#[test]
fn si_f02_v2_comparison_state_cannot_disappear_control() {
    comparison_state_cannot_disappear(2);
}

#[test]
fn si_f02_v3_comparison_state_cannot_disappear() {
    comparison_state_cannot_disappear(3);
}

#[test]
fn si_f02_normal_and_reset_transitions_cannot_implicitly_convert_format() {
    for (from, to) in [(2, 3), (3, 2)] {
        let mut store = seed(Some(from));
        let before = store.load_sync().unwrap();
        let model = run(store.catalog()).unwrap().state.unwrap();
        let next = fixture(to, &[]);
        SyncLedger::from_rows(&next).expect("target fixture is a valid complete ledger");
        for transition in [
            SyncTransition::Normal,
            SyncTransition::Reset {
                discard_recovery: false,
            },
            SyncTransition::Reset {
                discard_recovery: true,
            },
        ] {
            let mut target = next.clone();
            if matches!(transition, SyncTransition::Reset { .. }) {
                edit_data(&mut target[1], |state| {
                    state.insert("generation".into(), Json::Integer(2));
                    state.insert("revision".into(), Json::Integer(1));
                });
            }
            assert!(
                store
                    .apply_sync_plan(SyncPlan {
                        transition,
                        expected_model: model.clone(),
                        next_model: model.clone(),
                        expected_sync: before.clone(),
                        next_sync: target,
                        guards: vec![],
                        mutations: vec![],
                    })
                    .is_err(),
                "format changes require the explicit offline converter"
            );
            assert_eq!(store.load_sync().unwrap(), before);
        }
    }
}
