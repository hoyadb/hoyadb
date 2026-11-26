//! TC-0041 SI-Y06: pre-conversion entrypoint order. Corruption is injected only
//! into private task fixtures, never advertised as reachable through a public owner.
use super::*;
use crate::{model::*, storage::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    task::{Context, Poll, Waker},
};

pub(super) fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("synchronous fixture suspended"),
    }
}

pub(super) fn owner() -> (CommandDatabase<SnapshotStorage>, String) {
    let mut db = run(CommandDatabase::open(
        SnapshotStorage::default(),
        SchemaDefinition {
            version: 1,
            tables: BTreeMap::from([(
                "items".into(),
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
        },
        vec![],
        [0x6d; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    db.configure_entities(
        EntityConfig {
            scope: "phase3-characterization".into(),
            tables: BTreeSet::from(["items".into()]),
        },
        || {
            Ok(EntityContext {
                identity: Json::Null,
                now_ms: 100,
            })
        },
    )
    .unwrap();
    let request = db.request().unwrap();
    assert!(
        run(db.begin(
            request,
            CommandDefinition {
                name: "characterize-v1".into(),
                arguments: BTreeMap::new(),
                reference_paths: vec![],
            },
            Json::Object(BTreeMap::new()),
            Some(Json::Null),
            100
        ))
        .unwrap()
    );
    let key = run(db.finish(request, Json::Null)).unwrap().key;
    db.prepare_push(100, 0).unwrap();
    db.receive(&key, Some(RemoteOutcome::Rejected("rule".into())))
        .unwrap();
    run(db.reconcile(&key, ReconcilePlan::default())).unwrap();
    (db, key)
}

pub(super) fn corrupt_record(ledger: &mut records::SyncLedger, key: &str) -> CommandError {
    ledger.records.get_mut(key).unwrap().schema_version = 0;
    CommandError::Storage(ledger.validate().unwrap_err())
}

pub(super) fn corrupt_metadata(ledger: &mut records::SyncLedger) -> CommandError {
    ledger.entity.as_mut().unwrap().metadata.insert(
        ("items".into(), "metadata-fault".into()),
        RowMetadata {
            fields: BTreeMap::from([(
                "body".into(),
                FieldStamp {
                    timestamp_ms: -1,
                    authoritative: true,
                },
            )]),
            tombstone: None,
        },
    );
    CommandError::Storage(ledger.validate().unwrap_err())
}

#[test]
fn si_y06_reconcile_and_inverse_record_error_precedes_snapshot() {
    let _owners = TEST_OWNERS.lock().unwrap();
    let (db, key) = owner();
    let valid = run(db.snapshot()).unwrap();
    let invalid = BTreeMap::new(); // Missing the schema's items table.
    let mut ledger = db.ledger.clone();
    let ledger_error = corrupt_record(&mut ledger, &key);
    for inverse in [false, true] {
        let call = |ledger: &records::SyncLedger, rows: &reconcile::Snapshot| {
            if inverse {
                reconcile::inverse(ledger, rows, &db.db.db.schema, &key, false).err()
            } else {
                reconcile::reconcile(
                    ledger,
                    rows,
                    &db.db.db.schema,
                    &key,
                    ReconcilePlan::default(),
                )
                .err()
            }
        };
        assert_eq!(
            call(&db.ledger, &invalid),
            Some(CommandError::InvalidCommand)
        );
        assert_eq!(call(&ledger, &valid), Some(ledger_error.clone()));
        assert_eq!(call(&ledger, &invalid), Some(ledger_error.clone()));
    }
}

#[test]
fn si_y06_unproven_full_reconcile_and_inverse_metadata_error_precedes_snapshot() {
    let _owners = TEST_OWNERS.lock().unwrap();
    let (db, key) = owner();
    let mut ledger = db.ledger.clone();
    let error = corrupt_metadata(&mut ledger);
    // These pure entrypoints are the full-validation path retained for Unproven.
    // No requirement to detect untouched in-memory metadata on the future
    // validated Exclusive fast path is added by this characterization.
    assert_eq!(
        reconcile::reconcile(
            &ledger,
            &BTreeMap::new(),
            &db.db.db.schema,
            &key,
            ReconcilePlan::default()
        )
        .err(),
        Some(error.clone())
    );
    assert_eq!(
        reconcile::inverse(&ledger, &BTreeMap::new(), &db.db.db.schema, &key, false).err(),
        Some(error)
    );
}

#[test]
fn si_y06_plan_receive_ledger_precedes_key_and_push_input_precedes_ledger() {
    let _owners = TEST_OWNERS.lock().unwrap();
    let (db, key) = owner();
    let mut ledger = db.ledger.clone();
    let error = corrupt_record(&mut ledger, &key);
    assert_eq!(
        queue::plan_receive(&db.ledger, "missing", None).err(),
        Some(CommandError::Protocol)
    );
    assert_eq!(
        queue::plan_receive(&ledger, "missing", None).err(),
        Some(error.clone())
    );
    for (now, jitter) in [(-1, 0), (100, 2501)] {
        assert_eq!(
            queue::plan_push(&db.ledger, now, jitter).err(),
            Some(CommandError::InvalidCommand)
        );
        assert_eq!(
            queue::plan_push(&ledger, now, jitter).err(),
            Some(CommandError::InvalidCommand)
        );
    }
    assert_eq!(queue::plan_push(&ledger, 100, 0).err(), Some(error));
}

#[test]
fn si_y06_pull_page_and_row_resolution_precede_persist_ledger_validation() {
    let _owners = TEST_OWNERS.lock().unwrap();
    let (db, key) = owner();
    let before = run(db.snapshot()).unwrap();
    let mut ledger = db.ledger.clone();
    let error = corrupt_record(&mut ledger, &key);
    let valid_page = PullPage {
        checkpoint: "next".into(),
        complete: true,
        changes: vec![EntityChange {
            table: "items".into(),
            record_id: "incoming".into(),
            op: EntityOperation::Update,
            changes: BTreeMap::from([("body".into(), Value::String("remote".into()))]),
            removed_fields: BTreeSet::new(),
            timestamp_ms: 101,
            field_timestamps: BTreeMap::from([("body".into(), 101)]),
        }],
    };
    let bad_page = PullPage {
        checkpoint: "x".repeat(json::MAX_BYTES),
        ..valid_page.clone()
    };
    let page_error = pull::PullPlanner::new(&db.ledger, before.clone(), bad_page.clone())
        .err()
        .unwrap();
    assert_ne!(page_error, error);
    assert_eq!(
        pull::PullPlanner::new(&ledger, before.clone(), bad_page).err(),
        Some(page_error)
    );
    for source in [&db.ledger, &ledger] {
        let mut planner =
            pull::PullPlanner::new(source, BTreeMap::new(), valid_page.clone()).unwrap();
        assert_eq!(
            planner
                .advance(&db.db.db.schema, &BTreeMap::new(), &BTreeSet::new(), source)
                .err(),
            Some(CommandError::InvalidCommand)
        );
    }
    let mut planner = pull::PullPlanner::new(&ledger, before, valid_page).unwrap();
    planner
        .advance(
            &db.db.db.schema,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &ledger,
        )
        .unwrap();
    let planned = planner.finish().unwrap();
    let mut txn = ledger_txn::LedgerTxn::new(&mut ledger, false);
    planned.stage(&mut txn).unwrap();
    assert_eq!(
        txn.serialization_check().err().map(CommandError::Storage),
        Some(error)
    );
}
