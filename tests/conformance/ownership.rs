//! TC-0041 SI-A01–A05: per-database admission with process-global identities.
use hoyadb_core::{
    command::{entity::*, *},
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
        Poll::Pending => panic!("synchronous adapter unexpectedly suspended"),
    }
}

fn text(kind: ColumnType) -> Column {
    Column {
        kind,
        optional: false,
    }
}

pub fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([
            (
                "parents".into(),
                TableDefinition {
                    columns: BTreeMap::from([("name".into(), text(ColumnType::String))]),
                    ..Default::default()
                },
            ),
            (
                "items".into(),
                TableDefinition {
                    columns: BTreeMap::from([("label".into(), text(ColumnType::String))]),
                    ..Default::default()
                },
            ),
        ]),
    }
}

fn definition() -> CommandDefinition {
    CommandDefinition {
        name: "create-parent-v1".into(),
        arguments: BTreeMap::from([(
            "label".into(),
            Argument {
                kind: ArgumentType::String,
                optional: false,
            },
        )]),
        reference_paths: vec![],
    }
}

fn args(label: &str) -> Json {
    Json::Object(BTreeMap::from([(
        "label".into(),
        Json::String(label.into()),
    )]))
}

fn field(name: &str, value: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([(name.into(), Value::String(value.into()))])
}

fn identity() -> Option<Json> {
    Some(Json::String("trusted-user".into()))
}

fn names<S: SyncStorage>(db: &CommandDatabase<S>) -> BTreeSet<String> {
    run(db.fetch(&db.query("parents")))
        .unwrap()
        .into_iter()
        .map(|row| match &row.fields["name"] {
            Value::String(name) => name.clone(),
            other => panic!("unexpected name {other:?}"),
        })
        .collect()
}

pub fn open<S: SyncStorage>(store: S, namespace: u8) -> CommandDatabase<S> {
    let mut db = run(CommandDatabase::open(
        store,
        schema(),
        vec![],
        [namespace; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    db.configure_entities(
        EntityConfig {
            scope: "backend:ownership".into(),
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
    db
}

/// SI-A01: a requested, unbegun command on database X does not block Y.
pub fn queued_request_does_not_block<S: SyncStorage>(mut fresh: impl FnMut() -> S) {
    let a = open(fresh(), 0x41);
    let mut b = open(fresh(), 0x42);
    let queued = a.request().unwrap();
    let direct = run(b.insert("parents", field("name", "b-direct")));
    assert!(
        direct.is_ok(),
        "SI-A01: B insert behind A's queued request: {direct:?}"
    );
    assert_eq!(names(&b), BTreeSet::from(["b-direct".into()]));
    a.cancel_request(queued).unwrap();
}

/// SI-A02: an admitted handler on X does not block Y's handler or writes.
pub fn active_handler_does_not_block<S: SyncStorage>(mut fresh: impl FnMut() -> S) {
    let mut a = open(fresh(), 0x41);
    let mut b = open(fresh(), 0x42);
    let ar = a.request().unwrap();
    assert!(run(a.begin(ar, definition(), args("a"), identity(), 1000)).unwrap());
    run(a.scope_insert(ar, "parents", field("name", "a-staged"))).unwrap();
    let br = b.request().unwrap();
    assert!(
        run(b.begin(br, definition(), args("b"), identity(), 1000)).unwrap(),
        "SI-A02: B's command must be admitted while A's handler is active"
    );
    run(b.scope_insert(br, "parents", field("name", "b-staged"))).unwrap();
    run(b.finish(br, Json::Null)).unwrap();
    let item = run(b.insert("items", field("label", "one"))).unwrap();
    run(b.patch("items", &item.id, field("label", "two"))).unwrap();
    let pull = b.begin_pull().unwrap();
    b.abort_pull(pull.request).unwrap();
    assert_eq!(names(&b), BTreeSet::from(["b-staged".into()]));
    assert!(names(&a).is_empty());
    run(a.finish(ar, Json::Null)).unwrap();
    assert_eq!(names(&a), BTreeSet::from(["a-staged".into()]));
}

/// SI-A03: one database keeps FIFO admission, Busy writes and cancellation.
pub fn same_database_serialization<S: SyncStorage>(store: S) {
    let mut a = open(store, 0x43);
    let item = run(a.insert("items", field("label", "one"))).unwrap();
    let first = a.request().unwrap();
    let second = a.request().unwrap();
    assert!(!run(a.begin(second, definition(), args("second"), identity(), 1000)).unwrap());
    assert_eq!(
        run(a.insert("parents", field("name", "queued"))),
        Err(CommandError::Busy)
    );
    a.cancel_request(first).unwrap();
    assert!(matches!(
        a.cancel_request(first),
        Err(CommandError::InvalidTicket)
    ));
    assert!(run(a.begin(second, definition(), args("second"), identity(), 1000)).unwrap());
    assert_eq!(
        run(a.insert("parents", field("name", "active"))),
        Err(CommandError::Busy)
    );
    assert_eq!(
        run(a.patch("items", &item.id, field("label", "active"))),
        Err(CommandError::Busy)
    );
    assert!(matches!(a.begin_pull(), Err(CommandError::Busy)));
    run(a.finish(second, Json::Null)).unwrap();
    run(a.insert("parents", field("name", "after"))).unwrap();
}

/// SI-A04 and SI-A05: tickets never cross owners or survive a reopen.
pub fn tickets_stay_with_their_owner<S: SyncStorage>(mut fresh: impl FnMut() -> S) {
    let a = open(fresh(), 0x44);
    let mut b = open(fresh(), 0x45);
    let ar = a.request().unwrap();
    assert!(matches!(
        run(b.begin(ar, definition(), args("forged"), identity(), 1000)),
        Err(CommandError::InvalidTicket)
    ));
    assert!(matches!(
        b.cancel_request(ar),
        Err(CommandError::InvalidTicket)
    ));
    a.cancel_request(ar).unwrap();
    run(b.insert("parents", field("name", "b-after"))).unwrap();
    let stale = a.request().unwrap();
    let store = a.into_storage();
    let mut reopened = run(CommandDatabase::open(
        store,
        schema(),
        vec![],
        [0x44; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    let fresh_request = reopened.request().unwrap();
    assert_ne!(fresh_request, stale);
    assert!(matches!(
        run(reopened.begin(stale, definition(), args("stale"), identity(), 1000)),
        Err(CommandError::InvalidTicket)
    ));
    assert!(matches!(
        reopened.cancel_request(stale),
        Err(CommandError::InvalidTicket)
    ));
    assert!(
        run(reopened.begin(fresh_request, definition(), args("ok"), identity(), 1000)).unwrap()
    );
    run(reopened.finish(fresh_request, Json::Null)).unwrap();
}
