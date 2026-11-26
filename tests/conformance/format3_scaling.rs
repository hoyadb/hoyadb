//! SPEC-0037 R10 / TC-0041 SI-Y02 and SI-F03: public behavior, legacy twins.
use hoyadb_core::{
    command::{
        records::{SyncHeader, SyncLedger},
        *,
    },
    model::*,
    storage::*,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    task::{Context, Poll, Waker},
};

fn run<F: Future>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("concrete adapter unexpectedly suspended"),
    }
}
fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([
            (
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
            ),
            (
                "links".into(),
                TableDefinition {
                    columns: BTreeMap::from([(
                        "item".into(),
                        Column {
                            kind: ColumnType::Reference,
                            optional: false,
                        },
                    )]),
                    ..Default::default()
                },
            ),
        ]),
    }
}
fn config() -> EntityConfig {
    EntityConfig {
        scope: "format3-cases".into(),
        tables: BTreeSet::from(["items".into(), "links".into()]),
    }
}
fn provider() -> Result<EntityContext, CommandError> {
    Ok(EntityContext {
        identity: Json::Null,
        now_ms: 100,
    })
}
fn options() -> SyncOptions {
    SyncOptions {
        history: 0,
        ..Default::default()
    }
}
fn object<const N: usize>(items: [(&str, Json); N]) -> Json {
    Json::Object(
        items
            .into_iter()
            .map(|(key, value)| (key.into(), value))
            .collect(),
    )
}
fn text(value: &str) -> Json {
    Json::String(value.into())
}
/// Authored format-2 bytes, independent of EntityLedger::new and future defaults.
fn legacy<S: SyncStorage>(store: S) -> S {
    let mut store = run(Database::open(store, schema(), vec![]))
        .unwrap()
        .into_storage();
    let model = run(store.catalog()).unwrap().state.unwrap();
    let mut rows = SyncLedger {
        header: SyncHeader {
            namespace: "52".repeat(16),
            sequence: 1,
            last_ms: 0,
            options: options(),
        },
        entity: None,
        records: BTreeMap::new(),
    }
    .to_rows()
    .unwrap();
    rows.push(Row {
        id: "entity:state".into(),
        fields: BTreeMap::from([
            ("format".into(), Value::Integer(2)),
            (
                "data".into(),
                Value::String(
                    object([
                        (
                            "config",
                            object([
                                ("scope", text("format3-cases")),
                                ("tables", Json::Array(vec![text("items"), text("links")])),
                            ]),
                        ),
                        ("generation", Json::Integer(1)),
                        ("revision", Json::Integer(0)),
                        ("checkpoint", Json::Null),
                        ("ready", Json::Bool(false)),
                        ("pages", Json::Integer(0)),
                        ("changes", Json::Integer(0)),
                    ])
                    .canonical()
                    .unwrap(),
                ),
            ),
        ]),
    });
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
fn open<S: SyncStorage>(store: S) -> CommandDatabase<S> {
    let mut db = run(CommandDatabase::open(
        store,
        schema(),
        vec![],
        [0x52; 16],
        options(),
    ))
    .unwrap();
    db.configure_entities(config(), provider).unwrap();
    db
}
fn configured<S: SyncStorage>(store: S, old: bool) -> CommandDatabase<S> {
    open(if old {
        legacy(store)
    } else {
        store
    })
}
fn fields(value: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([("body".into(), Value::String(value.into()))])
}
fn change(id: String) -> EntityChange {
    EntityChange {
        table: "items".into(),
        record_id: id,
        op: EntityOperation::Update,
        changes: fields("remote"),
        removed_fields: BTreeSet::new(),
        timestamp_ms: 10,
        field_timestamps: BTreeMap::from([("body".into(), 10)]),
    }
}
fn apply<S: SyncStorage>(
    db: &mut CommandDatabase<S>,
    checkpoint: &str,
    changes: Vec<EntityChange>,
    complete: bool,
) -> counters::WorkSnapshot {
    let request = db.begin_pull().unwrap().request;
    counters::reset();
    run(db.apply_pull(
        request,
        PullPage {
            checkpoint: checkpoint.into(),
            complete,
            changes,
        },
    ))
    .unwrap();
    counters::snapshot()
}
fn packets<S: SyncStorage>(db: CommandDatabase<S>) -> (S, Vec<Row>) {
    let store = db.into_storage();
    let rows = store.load_sync().unwrap().unwrap();
    (store, rows)
}
fn data(row: &Row) -> BTreeMap<String, Json> {
    let Value::String(data) = &row.fields["data"] else {
        panic!("packet data")
    };
    let Json::Object(data) = Json::parse(data).unwrap() else {
        panic!("object data")
    };
    data
}
fn identity(table: &str, id: &str) -> String {
    use std::fmt::Write;
    let mut bytes = u32::try_from(table.len()).unwrap().to_be_bytes().to_vec();
    bytes.extend_from_slice(table.as_bytes());
    bytes.extend_from_slice(id.as_bytes());
    let mut result = String::from("entity:metadata:");
    for byte in bytes {
        write!(result, "{byte:02x}").unwrap();
    }
    result
}
fn assert_format(rows: &[Row], old: bool) {
    let ledger = SyncLedger::from_rows(rows).unwrap();
    let canonical = ledger.to_rows().unwrap();
    let header = canonical
        .iter()
        .find(|row| row.id == "entity:state")
        .unwrap();
    assert_eq!(
        data(header).get("ledger_format"),
        if old {
            None
        } else {
            Some(&Json::Integer(3))
        }
    );
    for (ordinal, row) in canonical
        .iter()
        .filter(|row| row.id.starts_with("entity:metadata:"))
        .enumerate()
    {
        let value = data(row);
        let (Json::String(table), Json::String(id)) = (&value["table"], &value["record_id"]) else {
            panic!("identity")
        };
        assert_eq!(
            row.id,
            if old {
                format!("entity:metadata:{ordinal:016x}")
            } else {
                identity(table, id)
            }
        );
    }
    if !old {
        assert!(canonical.windows(2).all(|pair| pair[0].id < pair[1].id));
    }
    assert_eq!(SyncLedger::from_rows(&canonical).unwrap(), ledger);
}
fn same_work(left: &counters::WorkSnapshot, right: &counters::WorkSnapshot) {
    for (name, a, b) in [
        ("packets_parsed", left.packets_parsed, right.packets_parsed),
        (
            "packets_encoded",
            left.packets_encoded,
            right.packets_encoded,
        ),
        (
            "ledger_entries_cloned",
            left.ledger_entries_cloned,
            right.ledger_entries_cloned,
        ),
        (
            "metadata_entries_visited",
            left.metadata_entries_visited,
            right.metadata_entries_visited,
        ),
        ("rows_cloned", left.rows_cloned, right.rows_cloned),
        (
            "sqlite_rows_read",
            left.sqlite_rows_read,
            right.sqlite_rows_read,
        ),
        (
            "sqlite_rows_written",
            left.sqlite_rows_written,
            right.sqlite_rows_written,
        ),
    ] {
        assert_eq!(a, b, "{name}: {left:?} versus {right:?}");
    }
    assert_eq!(left.sqlite_fullscan_steps, 0);
    assert_eq!(right.sqlite_fullscan_steps, 0);
    assert!(
        left.packets_encoded > 0 && left.ledger_entries_cloned > 0,
        "non-vacuous workload"
    );
}

pub fn ten_pages<S: SyncStorage>(store: S) {
    let mut db = configured(store, false);
    let mut work = vec![];
    for page in 0..10 {
        // Every subsequent page precedes prior identities, exposing ordinal rewrites.
        let changes = (0..100)
            .map(|row| change(format!("p{:02}:r{row:03}", 9 - page)))
            .collect();
        work.push(apply(
            &mut db,
            &format!("page{page:02}"),
            changes,
            page == 9,
        ));
    }
    assert_eq!(run(db.query("items").fetch(&db)).unwrap().len(), 1000);
    assert_eq!(db.sync_progress().pages, 10);
    assert_eq!(db.sync_progress().changes, 1000);
    assert!(db.sync_progress().ready);
    for (page, measured) in work.iter().enumerate().skip(1) {
        eprintln!("page0 {:?}; page{page} {measured:?}", work[0]);
        same_work(&work[0], measured);
    }
    let (_, rows) = packets(db);
    assert_format(&rows, false);
}

pub struct Insertion {
    pub work: counters::WorkSnapshot,
    pub changed: BTreeSet<String>,
    pub rows: Vec<Row>,
}
pub fn middle<S: SyncStorage>(store: S, m: usize, old: bool) -> Insertion {
    let mut db = configured(store, old);
    apply(
        &mut db,
        "initial",
        (0..m).map(|n| change(format!("r{:04}", 2 * n))).collect(),
        true,
    );
    let (store, before) = packets(db);
    let mut db = open(store);
    let work = apply(&mut db, "middle", vec![change("r0009".into())], true);
    assert_eq!(run(db.query("items").fetch(&db)).unwrap().len(), m + 1);
    let (_, rows) = packets(db);
    let prior: BTreeMap<_, _> = before
        .iter()
        .filter(|row| row.id.starts_with("entity:metadata:"))
        .map(|row| (row.id.clone(), row))
        .collect();
    let next: BTreeMap<_, _> = rows
        .iter()
        .filter(|row| row.id.starts_with("entity:metadata:"))
        .map(|row| (row.id.clone(), row))
        .collect();
    assert!(
        prior.keys().all(|id| next.contains_key(id)),
        "insertion deletes no metadata packet"
    );
    let changed = next
        .iter()
        .filter(|(id, row)| prior.get(*id) != Some(*row))
        .map(|(id, _)| id.clone())
        .collect();
    Insertion {
        work,
        changed,
        rows,
    }
}
pub fn assert_middle_new(small: Insertion, large: Insertion) {
    same_work(&small.work, &large.work);
    for result in [small, large] {
        assert_eq!(result.changed, BTreeSet::from([identity("items", "r0009")]));
        assert_format(&result.rows, false);
    }
}
pub fn assert_middle_legacy(small: Insertion, large: Insertion) {
    // r0009 inserts at ordinal5; count only metadata packets, not header writes.
    for (m, result) in [(10, &small), (1000, &large)] {
        assert_eq!(
            result.changed,
            (5..=m)
                .map(|ordinal| format!("entity:metadata:{ordinal:016x}"))
                .collect()
        );
        assert_eq!(result.changed.len(), m - 5 + 1);
        assert_format(&result.rows, true);
    }
    assert!(large.work.packets_encoded >= small.work.packets_encoded + 990);
    assert!(large.work.metadata_entries_visited >= small.work.metadata_entries_visited + 990);
}

pub fn history_and_references<S: SyncStorage>(store: S, old: bool) {
    let mut db = configured(store, old);
    let parent = run(db.insert("items", fields("parent"))).unwrap();
    let producer = db.queue_status().commands[0].key.clone();
    let canonical = "server:accepted-parent";
    let child = run(db.insert(
        "links",
        BTreeMap::from([("item".into(), Value::Reference(parent.id.clone()))]),
    ))
    .unwrap();
    let dependent = db
        .queue_status()
        .commands
        .iter()
        .find(|entry| entry.key != producer)
        .unwrap()
        .key
        .clone();
    let batch = db.prepare_push(100, 0).unwrap();
    assert_eq!(batch.envelopes.len(), 1);
    db.receive(&producer, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    run(db.reconcile(
        &producer,
        ReconcilePlan {
            mappings: BTreeMap::from([(parent.id.clone(), canonical.into())]),
            writes: vec![],
        },
    ))
    .unwrap();
    assert!(
        !db.queue_status()
            .commands
            .iter()
            .any(|entry| entry.key == producer),
        "history0 trims resolved producer"
    );
    let batch = db.prepare_push(100, 0).unwrap();
    assert_eq!(
        batch.envelopes.len(),
        1,
        "dependent releases after producer resolution"
    );
    let Json::Object(envelope) = Json::parse(&batch.envelopes[0]).unwrap() else {
        panic!("envelope")
    };
    let Json::Object(changes) = &envelope["changes"] else {
        panic!("changes")
    };
    assert_eq!(changes["item"], Json::String(canonical.into()));
    db.receive(&dependent, Some(RemoteOutcome::Applied(Json::Null)))
        .unwrap();
    run(db.reconcile(
        &dependent,
        ReconcilePlan {
            mappings: BTreeMap::from([(child.id.clone(), child.id.clone())]),
            writes: vec![],
        },
    ))
    .unwrap();
    assert!(db.queue_status().commands.is_empty());
    let (store, rows) = packets(db);
    assert!(SyncLedger::from_rows(&rows).unwrap().records.is_empty());
    let mut db = open(store);
    assert_eq!(
        run(db.query("links").fetch(&db)).unwrap()[0].fields["item"],
        Value::Reference(canonical.into())
    );
    run(db.patch("items", canonical, fields("after-trim"))).unwrap();
    let patch = db.queue_status().commands[0].key.clone();
    run(db.cancel(&patch)).unwrap();
    assert_eq!(
        run(db.query("items").fetch(&db)).unwrap()[0].fields["body"],
        Value::String("parent".into())
    );
    let before = run(db.query("links").fetch(&db)).unwrap();
    let epoch = db.ledger_work().epoch;
    assert_eq!(
        run(db.patch(
            "links",
            &child.id,
            BTreeMap::from([("item".into(), Value::Reference("local:unknown".into()))])
        )),
        Err(CommandError::UnknownLocalReference)
    );
    assert_eq!(run(db.query("links").fetch(&db)).unwrap(), before);
    assert_eq!(db.ledger_work().epoch, epoch);
    let (_, rows) = packets(db);
    assert_format(&rows, old);
}
