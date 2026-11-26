//! TC-0041 SI-V01/SI-V02: command-capable open validates every stored row once.
use hoyadb_core::{command::*, model::*, storage::*};
use std::{
    collections::BTreeMap,
    future::Future,
    task::{Context, Poll, Waker},
};

pub fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("synchronous adapter unexpectedly suspended"),
    }
}

pub fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([(
            "parents".into(),
            TableDefinition {
                columns: BTreeMap::from([(
                    "name".into(),
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

pub fn definition() -> CommandDefinition {
    CommandDefinition {
        name: "create-parent-v1".into(),
        arguments: BTreeMap::new(),
        reference_paths: vec![],
    }
}

pub fn name(value: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([("name".into(), Value::String(value.into()))])
}

pub fn open<S: SyncStorage>(store: S) -> Result<CommandDatabase<S>, CommandError> {
    run(CommandDatabase::open(
        store,
        schema(),
        vec![],
        [0x56; 16],
        SyncOptions::default(),
    ))
}

/// A store with a committed command ledger, one row to corrupt and one valid row.
pub fn seeded<S: SyncStorage>(store: S) -> (S, Row) {
    let mut db = open(store).unwrap();
    let target = run(db.insert("parents", name("target"))).unwrap();
    run(db.insert("parents", name("untouched"))).unwrap();
    let request = db.request().unwrap();
    assert!(
        run(db.begin(
            request,
            definition(),
            Json::Object(BTreeMap::new()),
            Some(Json::Null),
            10,
        ))
        .unwrap()
    );
    run(db.scope_insert(request, "parents", name("command"))).unwrap();
    run(db.finish(request, Json::Null)).unwrap();
    let store = db.into_storage();
    // The stored image carries the managed row version.
    let stored = run(store.get("parents", &target.id)).unwrap().unwrap();
    (store, stored)
}

/// Label, edit, and whether plain `Database::open` accepts it today (pinned).
type Corruption = (&'static str, fn(Row) -> Row, bool);

pub fn corruptions() -> [Corruption; 3] {
    [
        (
            "wrong kind",
            |mut row| {
                row.fields.insert("name".into(), Value::Integer(7));
                row
            },
            true,
        ),
        (
            "missing required field",
            |mut row| {
                row.fields.remove("name");
                row
            },
            true,
        ),
        (
            "wrong row version",
            |mut row| {
                row.fields.insert(ROW_VERSION.into(), Value::Integer(2));
                row
            },
            false,
        ),
    ]
}

/// SI-V01: a corrupt row no command touches makes command-capable open fail,
/// while plain `Database::open` keeps today's behavior.
pub fn untouched_corruption_fails_open<S: SyncStorage>(mut fresh: impl FnMut() -> S) {
    for (label, corrupt, plain_accepts) in corruptions() {
        let (mut store, stored) = seeded(fresh());
        // A raw adapter write below the model, as an external edit would be.
        run(store.replace("parents", corrupt(stored))).unwrap();
        let store = match run(Database::open_recoverable(store, schema(), vec![])) {
            Ok(db) => {
                assert!(
                    plain_accepts,
                    "{label}: plain Database::open must stay unchanged"
                );
                db.into_storage()
            }
            Err((_, store)) => {
                assert!(
                    !plain_accepts,
                    "{label}: plain Database::open must stay unchanged"
                );
                store
            }
        };
        assert!(
            open(store).is_err(),
            "{label}: CommandDatabase::open accepted an untouched corrupt row"
        );
    }
}

/// SI-V02: a ledger packet edited while the store is closed fails open.
pub fn closed_ledger_edit_fails_open<S: SyncStorage>(store: S) {
    let (mut store, _) = seeded(store);
    let packets = store.load_sync().unwrap().unwrap();
    let mut packet = packets[0].clone();
    packet.fields = BTreeMap::from([("data".into(), Value::String("tampered".into()))]);
    run(store.replace(SYNC_TABLE, packet)).unwrap();
    assert!(open(store).is_err());
}
