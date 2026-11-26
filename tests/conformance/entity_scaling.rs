//! TC-0041 SI-O03/SI-O04/SI-O05: entity staging, the reconfigure emptiness
//! check and merge preparation do work independent of unrelated rows.
use hoyadb_core::{command::entity::*, command::*, model::*, storage::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    task::{Context, Poll, Waker},
};

pub fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("synchronous adapter unexpectedly suspended"),
    }
}

fn column(kind: ColumnType, optional: bool) -> Column {
    Column {
        kind,
        optional,
    }
}

/// `items` is the entity table; `local` holds unrelated filler rows.
pub fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([
            (
                "items".into(),
                TableDefinition {
                    columns: BTreeMap::from([
                        ("body".into(), column(ColumnType::String, false)),
                        ("rank".into(), column(ColumnType::Integer, true)),
                    ]),
                    ..Default::default()
                },
            ),
            (
                "local".into(),
                TableDefinition {
                    columns: BTreeMap::from([("note".into(), column(ColumnType::String, false))]),
                    ..Default::default()
                },
            ),
        ]),
    }
}

pub fn change(body: &str, time: i64) -> EntityChange {
    EntityChange {
        table: "items".into(),
        record_id: "record".into(),
        op: EntityOperation::Update,
        changes: BTreeMap::from([("body".into(), Value::String(body.into()))]),
        removed_fields: BTreeSet::new(),
        timestamp_ms: time,
        field_timestamps: BTreeMap::from([("body".into(), time)]),
    }
}

fn configure<S: SyncStorage>(db: &mut CommandDatabase<S>) {
    db.configure_entities(
        EntityConfig {
            scope: "scaling".into(),
            tables: BTreeSet::from(["items".into()]),
        },
        || {
            Ok(EntityContext {
                identity: Json::Null,
                now_ms: 10,
            })
        },
    )
    .unwrap();
}

pub fn open<S: SyncStorage>(store: S) -> CommandDatabase<S> {
    run(CommandDatabase::open(
        store,
        schema(),
        vec![],
        [0x5c; 16],
        SyncOptions::default(),
    ))
    .unwrap()
}

fn filler<S: SyncStorage>(db: &mut CommandDatabase<S>, n: usize) -> Vec<String> {
    (0..n)
        .map(|index| {
            run(db.insert(
                "local",
                BTreeMap::from([("note".into(), Value::String(format!("filler {index}")))]),
            ))
            .unwrap()
            .id
        })
        .collect()
}

/// An entity owner with one pulled entity row `record` and n unrelated rows.
pub fn configured<S: SyncStorage>(store: S, n: usize) -> (CommandDatabase<S>, Vec<String>) {
    let mut db = open(store);
    configure(&mut db);
    let request = db.begin_pull().unwrap().request;
    run(db.apply_pull(
        request,
        PullPage {
            checkpoint: "cp1".into(),
            complete: true,
            changes: vec![change("base", 10)],
        },
    ))
    .unwrap();
    let ids = filler(&mut db, n);
    (db, ids)
}

/// SI-O03: entity patch, insert and remove; staging-site work only.
pub fn entity_staging_work<S: SyncStorage>(store: S, n: usize) -> Vec<(u64, u64)> {
    let (mut db, _) = configured(store, n);
    let mut work = Vec::new();
    let mut measure = |db: &mut CommandDatabase<S>, step: &dyn Fn(&mut CommandDatabase<S>)| {
        counters::reset();
        step(db);
        let snapshot = counters::snapshot();
        work.push((snapshot.rows_cloned, snapshot.inventory_rows_validated));
    };
    measure(&mut db, &|db| {
        run(db.patch(
            "items",
            "record",
            BTreeMap::from([("body".into(), Value::String("patched".into()))]),
        ))
        .unwrap();
    });
    measure(&mut db, &|db| {
        let row = run(db.insert(
            "items",
            BTreeMap::from([("body".into(), Value::String("new".into()))]),
        ))
        .unwrap();
        assert!(run(db.remove("items", &row.id)).unwrap());
    });
    work
}

/// SI-O04: a post-reset entity shell reconfigures only if the store is empty;
/// the check's work on a non-empty store. Also returns the empty-store result.
pub fn reconfigure_emptiness_work<S: SyncStorage>(
    store: S,
    empty: S,
    n: usize,
) -> (
    Result<(), CommandError>,
    (u64, u64),
    Result<(), CommandError>,
) {
    let shell = |store: S| {
        let mut db = open(store);
        configure(&mut db);
        run(db.reset(ResetMode::DiscardRecovery)).unwrap();
        db
    };
    let config = || EntityConfig {
        scope: "reconfigured".into(),
        tables: BTreeSet::from(["items".into()]),
    };
    let provider = || {
        Ok(EntityContext {
            identity: Json::Null,
            now_ms: 10,
        })
    };
    let mut db = shell(store);
    filler(&mut db, n);
    counters::reset();
    let refused = run(db.reconfigure_entities(config(), provider));
    let snapshot = counters::snapshot();
    let mut empty = shell(empty);
    let accepted = run(empty.reconfigure_entities(config(), provider));
    (
        refused,
        (snapshot.memory_rows_visited, snapshot.sqlite_rows_read),
        accepted,
    )
}

/// SI-O05: merge prepare and the host reply; their work. Leaves the session
/// ready to commit.
pub fn merge_prepare_work<S: SyncStorage>(
    db: &mut CommandDatabase<S>,
) -> (MergeSession, counters::WorkSnapshot) {
    let request = db.begin_pull().unwrap().request;
    counters::reset();
    let MergeStep::Offer {
        session,
        step,
        ..
    } = run(db.merge_prepare(
        request,
        PullPage {
            checkpoint: "cp2".into(),
            complete: true,
            changes: vec![change("merged", 20)],
        },
        BTreeSet::from(["items".into()]),
    ))
    .unwrap()
    else {
        panic!("expected an offer")
    };
    let default = db.merge_input(session, step).unwrap().default.row;
    assert!(matches!(
        run(db.merge_reply(session, step, default)).unwrap(),
        MergeStep::Ready { .. }
    ));
    (session, counters::snapshot())
}
