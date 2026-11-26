//! TC-0034 E04/E07/E13/E17: independent OS-process persistence driver.
use hoyadb_core::{
    command::{entity::*, records::SyncLedger, *},
    model::*,
    storage::*,
};
use hoyadb_storage_sqlite::SqliteStorage;
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    task::{Context, Poll, Waker},
};
fn run<F: Future>(f: F) -> F::Output {
    match std::pin::pin!(f).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("SQLite suspended"),
    }
}

fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([(
            "items".into(),
            TableDefinition {
                columns: BTreeMap::from([(
                    "label".into(),
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
        scope: "process/backend".into(),
        tables: BTreeSet::from(["items".into()]),
    }
}

fn page(checkpoint: &str, complete: bool, label: &str, time: i64) -> PullPage {
    PullPage {
        checkpoint: checkpoint.into(),
        complete,
        changes: vec![EntityChange {
            table: "items".into(),
            record_id: "remote".into(),
            op: EntityOperation::Update,
            changes: BTreeMap::from([("label".into(), Value::String(label.into()))]),
            removed_fields: BTreeSet::new(),
            timestamp_ms: time,
            field_timestamps: BTreeMap::from([("label".into(), time)]),
        }],
    }
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    assert_eq!(args.len(), 3);
    let path = &args[2];
    let store = SqliteStorage::open_unprotected(path).unwrap();
    let mut db = run(CommandDatabase::open(
        store,
        schema(),
        vec![],
        [0x31; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    match args[1].as_str() {
        "write" => {
            db.configure_entities(config(), || {
                Ok(EntityContext {
                    identity: Json::String("trusted".into()),
                    now_ms: 100,
                })
            })
            .unwrap();
            let request = db.begin_pull().unwrap().request;
            run(db.apply_pull(request, page("opaque:one", false, "first", 50))).unwrap();
            let row = run(db.insert(
                "items",
                BTreeMap::from([("label".into(), Value::String("offline".into()))]),
            ))
            .unwrap();
            assert!(row.id.ends_with(":0000000000000001"));
            let batch = db.prepare_push(100, 0).unwrap();
            assert_eq!(batch.envelopes.len(), 1);
            let key = db.queue_status().commands[0].key.clone();
            db.receive_entity("items", &row.id, &key, None).unwrap();
            assert_eq!(db.queue_status().awaiting_outcome, 1);
        }
        "read" => {
            assert_eq!(db.sync_progress().pages, 1);
            assert!(!db.sync_progress().ready);
            assert!(db.prepare_push(900_000, 0).unwrap().envelopes.is_empty());
            let key = db.queue_status().commands[0].key.clone();
            let id = format!("entity:{}:0000000000000001", "31".repeat(16));
            db.receive_entity("items", &id, &key, Some(RemoteOutcome::Applied(Json::Null)))
                .unwrap();
            run(db.reconcile(
                &key,
                ReconcilePlan {
                    mappings: BTreeMap::from([(id.clone(), id)]),
                    writes: vec![],
                },
            ))
            .unwrap();
            let request = db.begin_pull().unwrap();
            assert_eq!(request.since, Some("opaque:one".into()));
            run(db.apply_pull(request.request, page("opaque:two", true, "second", 200))).unwrap();
            assert!(db.sync_progress().ready);
            assert_eq!(run(db.fetch(&db.query("items"))).unwrap().len(), 2);
        }
        "reset" => {
            assert_eq!(db.sync_progress().pages, 2);
            assert!(db.sync_progress().ready);
            run(db.reset(ResetMode::PreserveRecovery)).unwrap();
            assert!(run(db.fetch(&db.query("items"))).unwrap().is_empty());
            assert!(db.queue_status().commands.is_empty());
        }
        "read-reset" => {
            assert_eq!(db.sync_progress().pages, 0);
            assert!(!db.sync_progress().ready);
            let request = db.begin_pull().unwrap();
            assert_eq!(request.since, None);
            assert_eq!(request.generation, 2);
            db.abort_pull(request.request).unwrap();
            assert!(run(db.fetch(&db.query("items"))).unwrap().is_empty());
            db.configure_entities(config(), || {
                Ok(EntityContext {
                    identity: Json::Null,
                    now_ms: 300,
                })
            })
            .unwrap();
            let row = run(db.insert(
                "items",
                BTreeMap::from([("label".into(), Value::String("after reset".into()))]),
            ))
            .unwrap();
            assert!(row.id.ends_with(":0000000000000002"));
        }
        _ => panic!("unknown entity process mode"),
    }
    drop(db);
    let store = SqliteStorage::open_unprotected(path).unwrap();
    let ledger = SyncLedger::from_rows(&store.load_sync().unwrap().unwrap()).unwrap();
    ledger.entity.as_ref().unwrap().validate(&schema()).unwrap();
    println!("PHASE5_PROCESS_{}_GREEN", args[1]);
}
