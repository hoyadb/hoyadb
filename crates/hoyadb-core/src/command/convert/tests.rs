use super::*;
use crate::{
    command::{
        EntityConfig, EntityDescriptor, EntityLedger, EntityOperation, FieldStamp, Json,
        MetadataChange, PathPart, RowMetadata, SyncOptions, SyncState,
        records::{SyncHeader, SyncLedger, SyncRecord},
    },
    model::{SchemaDefinition, TableDefinition},
    storage::{Column, ColumnType, CommittedChange, Value, validate_sync_model},
};
use std::collections::{BTreeMap, BTreeSet};

fn model() -> ModelState {
    let definition = SchemaDefinition {
        version: 1,
        tables: ["a", "ab", "items", "é"]
            .into_iter()
            .map(|name| {
                (
                    name.into(),
                    TableDefinition {
                        columns: BTreeMap::from([(
                            "name".into(),
                            Column {
                                kind: ColumnType::String,
                                optional: false,
                            },
                        )]),
                        relations: BTreeMap::new(),
                    },
                )
            })
            .collect(),
    };
    ModelState {
        version: 1,
        definition: definition.canonical().unwrap(),
        next_id: 41,
    }
}

fn metadata() -> RowMetadata {
    RowMetadata {
        fields: BTreeMap::from([(
            "name".into(),
            FieldStamp {
                timestamp_ms: 100,
                authoritative: true,
            },
        )]),
        tombstone: None,
    }
}

fn record(order: i64) -> SyncRecord {
    SyncRecord {
        entity: None,
        metadata_undo: vec![],
        key: format!("{:016x}-{}-{order:016x}", 100, "0".repeat(32)),
        command: "convert-fixture-v1".into(),
        args: Json::Object(BTreeMap::from([(
            "payload".into(),
            Json::String("private-payload\n雪".into()),
        )])),
        context: Json::Object(BTreeMap::from([
            ("identity".into(), Json::String("private-identity".into())),
            (
                "capturedAt".into(),
                Json::String(crate::command::owner::utc_time(100)),
            ),
        ])),
        schema_version: 1,
        order,
        reference_paths: vec![],
        dependencies: BTreeSet::new(),
        created: BTreeMap::new(),
        mappings: BTreeMap::new(),
        undo: vec![],
        state: SyncState::Queued,
        frozen: None,
        attempts: 0,
        due_ms: 0,
        outcome: None,
        rolled_back: false,
        reconciled: false,
    }
}

fn fixture(format: u8) -> (Vec<Row>, ModelState) {
    let model = model();
    let mut entity = EntityLedger::new(EntityConfig {
        scope: "private-scope".into(),
        tables: BTreeSet::from(["a".into(), "ab".into(), "items".into(), "é".into()]),
    });
    entity.ledger_format = format;
    entity.generation = 3;
    entity.revision = 11;
    entity.pages = 2;
    entity.changes = 7;
    entity.checkpoint = Some("checkpoint\n雪".into());
    entity.ready = true;
    for (table, id) in [("a", "bc"), ("ab", "c"), ("items", "remote🙂"), ("é", "雪")] {
        entity
            .metadata
            .insert((table.into(), id.into()), metadata());
    }
    entity
        .metadata
        .get_mut(&("é".into(), "雪".into()))
        .unwrap()
        .tombstone = Some(FieldStamp {
        timestamp_ms: 101,
        authoritative: false,
    });
    let mut producer = record(1);
    producer
        .created
        .insert("local:child".into(), "items".into());
    producer.undo.push(CommittedChange {
        table: "items".into(),
        id: "local:child".into(),
        before: None,
        after: Some(Row {
            id: "local:child".into(),
            fields: BTreeMap::from([("name".into(), Value::String("private-row".into()))]),
        }),
    });
    let mut consumer = record(2);
    consumer.args = Json::Object(BTreeMap::from([(
        "ref".into(),
        Json::String("local:child".into()),
    )]));
    consumer
        .reference_paths
        .push(vec![PathPart::Key("ref".into())]);
    consumer.dependencies.insert("local:child".into());
    let mut sent = record(3);
    sent.state = SyncState::Sent;
    sent.attempts = 2;
    sent.frozen = Some(sent.envelope().canonical().unwrap());
    let mut discarded = record(4);
    discarded.state = SyncState::Discarded;
    discarded.reconciled = true;
    let mut update = record(5);
    update.command.clear();
    update.entity = Some(EntityDescriptor {
        table: "items".into(),
        op: EntityOperation::Update,
        timestamp_ms: 100,
        removed_fields: BTreeSet::new(),
    });
    update.args = Json::Object(BTreeMap::from([
        ("recordId".into(), Json::String("remote".into())),
        (
            "changes".into(),
            Json::Object(BTreeMap::from([(
                "name".into(),
                Json::String("changed".into()),
            )])),
        ),
    ]));
    // Non-create entity diffs must reference their record id (validate_entity_payload).
    update
        .reference_paths
        .push(vec![PathPart::Key("recordId".into())]);
    update.metadata_undo.push(MetadataChange {
        table: "items".into(),
        record_id: "remote".into(),
        before: None,
        after: Some(metadata()),
    });
    let ledger = SyncLedger {
        entity: Some(entity),
        header: SyncHeader {
            namespace: "0".repeat(32),
            sequence: 9,
            last_ms: 100,
            options: SyncOptions::default(),
        },
        records: [producer, consumer, sent, discarded, update]
            .into_iter()
            .map(|r| (r.key.clone(), r))
            .collect(),
    };
    let mut rows = ledger.to_rows().unwrap();
    // The source proof retains the actual input order; target order is by ID.
    rows.reverse();
    let decoded = SyncLedger::from_rows(&rows).unwrap();
    validate_sync_model(&decoded, &model).unwrap();
    (rows, model)
}

fn indexed(rows: &[Row]) -> BTreeMap<&str, &Row> {
    rows.iter().map(|row| (row.id.as_str(), row)).collect()
}

fn edit(row: &mut Row, f: impl FnOnce(&mut BTreeMap<String, Json>)) {
    let Value::String(data) = row.fields.get_mut("data").unwrap() else {
        panic!()
    };
    let Json::Object(mut fields) = Json::parse(data).unwrap() else {
        panic!()
    };
    f(&mut fields);
    *data = Json::Object(fields).canonical().unwrap();
}

#[test]
fn si_x01_fixture_control_decodes_complex_both_formats() {
    for format in [2, 3] {
        let (rows, model) = fixture(format);
        let ledger = SyncLedger::from_rows(&rows).unwrap();
        validate_sync_model(&ledger, &model).unwrap();
        assert_eq!(ledger.entity.unwrap().ledger_format, format);
        assert_eq!(ledger.records.len(), 5);
    }
}

#[test]
fn si_x01_round_trip_restores_exact_packets_model_and_counts() {
    for from in [2, 3] {
        let (rows, model) = fixture(from);
        let source = rows.clone();
        let source_model = model.clone();
        let converted = reencode(&rows, &model, 5 - from).unwrap();
        assert_eq!(
            converted.report(),
            &ConversionReport {
                from,
                to: 5 - from,
                metadata_entries: 4,
                packets_rewritten: 5,
            }
        );
        assert_eq!(converted.source_rows(), rows);
        assert_eq!(converted.model(), &model);
        assert!(converted.rows().windows(2).all(|p| p[0].id < p[1].id));
        let decoded = SyncLedger::from_rows(converted.rows()).unwrap();
        assert_eq!(decoded.entity.unwrap().ledger_format, 5 - from);
        let restored = reencode(converted.rows(), converted.model(), from).unwrap();
        assert_eq!(indexed(restored.rows()), indexed(&rows));
        assert_eq!(restored.model(), &model);
        assert_eq!(
            restored.report(),
            &ConversionReport {
                from: 5 - from,
                to: from,
                metadata_entries: 4,
                packets_rewritten: 5,
            }
        );
        assert_eq!(rows, source);
        assert_eq!(model, source_model);
    }
}

#[test]
fn si_x01_changes_only_marker_and_metadata_ids_in_each_direction() {
    for from in [2, 3] {
        let (rows, model) = fixture(from);
        let converted = reencode(&rows, &model, 5 - from).unwrap();
        let target = indexed(converted.rows());
        for old in &rows {
            if old.id.starts_with("entity:metadata:") {
                let matches: Vec<_> = converted
                    .rows()
                    .iter()
                    .filter(|r| r.id.starts_with("entity:metadata:") && r.fields == old.fields)
                    .collect();
                assert_eq!(matches.len(), 1, "metadata fields must be verbatim");
                assert_ne!(matches[0].id, old.id);
            } else if old.id == "entity:state" {
                let mut expected = old.clone();
                edit(&mut expected, |fields| {
                    if from == 2 {
                        fields.insert("ledger_format".into(), Json::Integer(3));
                    } else {
                        fields.remove("ledger_format");
                    }
                });
                assert_eq!(target[old.id.as_str()], &expected);
            } else {
                assert_eq!(target[old.id.as_str()], old, "untouched packet bytes");
            }
        }
        if from == 2 {
            assert!(target.contains_key("entity:metadata:00000001616263"));
            assert!(target.contains_key("entity:metadata:00000002616263"));
        }
    }
}

#[test]
fn si_x01_empty_comparison_shell_rewrites_only_entity_header() {
    for from in [2, 3] {
        let (rows, model) = fixture(from);
        let mut ledger = SyncLedger::from_rows(&rows).unwrap();
        ledger.records.clear();
        ledger.entity.as_mut().unwrap().metadata.clear();
        let rows = ledger.to_rows().unwrap();
        let converted = reencode(&rows, &model, 5 - from).unwrap();
        assert_eq!(converted.report().metadata_entries, 0);
        assert_eq!(converted.report().packets_rewritten, 1);
        assert_eq!(indexed(converted.rows())["state"], indexed(&rows)["state"]);
    }
}

#[test]
fn si_x02_no_entity_and_same_or_invalid_target_are_refused() {
    for from in [2, 3] {
        let (rows, model) = fixture(from);
        for target in [0, 1, from, 4, u8::MAX] {
            assert!(reencode(&rows, &model, target).is_err(), "target={target}");
        }
        let mut ledger = SyncLedger::from_rows(&rows).unwrap();
        ledger.records.clear();
        ledger.entity = None;
        let command_only = ledger.to_rows().unwrap();
        validate_sync_model(&SyncLedger::from_rows(&command_only).unwrap(), &model).unwrap();
        assert!(reencode(&command_only, &model, 5 - from).is_err());
    }
}

#[test]
fn si_x02_complete_source_decode_precedes_model_and_target_refusals() {
    let (mut rows, mut model) = fixture(2);
    rows.push(rows[0].clone());
    model.version = 0;
    let expected = SyncLedger::from_rows(&rows).err().unwrap();
    assert_eq!(reencode(&rows, &model, 0).err().unwrap(), expected);
}

#[test]
fn si_x02_full_model_validation_rejects_undeclared_metadata_and_live_schema() {
    for fault in 0..3 {
        let (rows, mut model) = fixture(2);
        let mut ledger = SyncLedger::from_rows(&rows).unwrap();
        match fault {
            0 => {
                ledger
                    .entity
                    .as_mut()
                    .unwrap()
                    .metadata
                    .values_mut()
                    .next()
                    .unwrap()
                    .fields
                    .insert(
                        "undeclared".into(),
                        FieldStamp {
                            timestamp_ms: 100,
                            authoritative: true,
                        },
                    );
            }
            1 => {
                ledger.records.values_mut().next().unwrap().schema_version = 2;
            }
            _ => {
                model.next_id = 0;
            }
        }
        let rows = ledger.to_rows().unwrap();
        let decoded = SyncLedger::from_rows(&rows).unwrap();
        let expected = validate_sync_model(&decoded, &model).err().unwrap();
        assert_eq!(
            reencode(&rows, &model, 3).err().unwrap(),
            expected,
            "fault={fault}"
        );
    }
}

#[test]
fn si_x02_rejects_noncanonical_unknown_and_mixed_packets() {
    for from in [2, 3] {
        let (rows, model) = fixture(from);
        for fault in 0..3 {
            let mut invalid = rows.clone();
            let row = invalid
                .iter_mut()
                .find(|r| r.id.starts_with("entity:metadata:"))
                .unwrap();
            match fault {
                0 => {
                    let Value::String(data) = row.fields.get_mut("data").unwrap() else {
                        panic!()
                    };
                    data.push(' ');
                }
                1 => {
                    row.id = "unknown:packet".into();
                }
                _ => {
                    row.id = if from == 2 {
                        "entity:metadata:00000001616263".into()
                    } else {
                        "entity:metadata:0000000000000000".into()
                    };
                }
            }
            let saved = invalid.clone();
            let expected = SyncLedger::from_rows(&invalid).err().unwrap();
            assert_eq!(
                reencode(&invalid, &model, 5 - from).err().unwrap(),
                expected
            );
            assert_eq!(invalid, saved);
        }
    }
}
