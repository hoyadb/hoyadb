//! TC-0041 SI-Q01/SI-Q02/SI-Q04: ordered limits of a validated Exclusive owner
//! return exactly the API-0003 order (value, then ascending id).
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

fn column(kind: ColumnType, optional: bool) -> Column {
    Column {
        kind,
        optional,
    }
}

pub fn schema() -> SchemaDefinition {
    SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([
            (
                "items".into(),
                TableDefinition {
                    columns: BTreeMap::from([
                        ("rank".into(), column(ColumnType::Integer, false)),
                        ("label".into(), column(ColumnType::String, false)),
                        ("score".into(), column(ColumnType::Float, false)),
                        ("flag".into(), column(ColumnType::Boolean, false)),
                        ("at".into(), column(ColumnType::Timestamp, false)),
                        ("link".into(), column(ColumnType::Reference, false)),
                        ("note".into(), column(ColumnType::String, true)),
                    ]),
                    ..Default::default()
                },
            ),
            (
                "empty".into(),
                TableDefinition {
                    columns: BTreeMap::from([("rank".into(), column(ColumnType::Integer, false))]),
                    ..Default::default()
                },
            ),
        ]),
    }
}

pub fn open<S: SyncStorage>(store: S) -> CommandDatabase<S> {
    run(CommandDatabase::open(
        store,
        schema(),
        vec![],
        [0x51; 16],
        SyncOptions::default(),
    ))
    .unwrap()
}

fn float(value: f64) -> Value {
    Value::Float(FiniteFloat::new(value).unwrap())
}

fn item(rank: i64, label: &str, score: f64) -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("rank".into(), Value::Integer(rank)),
        ("label".into(), Value::String(label.into())),
        ("score".into(), float(score)),
        ("flag".into(), Value::Boolean(rank % 3 == 0)),
        ("at".into(), Value::Timestamp(rank * 7)),
        (
            "link".into(),
            Value::Reference(format!("target:{rank:+05}")),
        ),
    ])
}

/// The same top region at every N: rank 500 ×3, 400 ×1, 300 ×4 (a tie group
/// straddling k=5), labels "zzz" ×2 above a giant "aaa" tie group of N filler
/// rows, and integral plus fractional scores. Filler ranks and scores are
/// distinct and below the top region.
pub fn seed<S: SyncStorage>(db: &mut CommandDatabase<S>, n: i64) {
    let mut writes = Vec::new();
    for (rank, count) in [(500, 3), (400, 1), (300, 4)] {
        for index in 0..count {
            let score = [2.5, -0.0, 0.0, 3.0][index as usize % 4];
            writes.push(Write::Insert {
                table: "items".into(),
                fields: item(
                    rank,
                    if rank == 500 && index < 2 {
                        "zzz"
                    } else {
                        "mid"
                    },
                    score,
                ),
            });
        }
    }
    for filler in 0..n {
        writes.push(Write::Insert {
            table: "items".into(),
            fields: item(-1 - filler, "aaa", -1000.0 - filler as f64 * 0.5),
        });
    }
    run(db.apply_batch(writes)).unwrap();
}

pub fn ordered(field: &str, direction: Direction, limit: Option<usize>) -> Query {
    Query {
        table: "items".into(),
        predicates: vec![],
        order: Some(OrderBy {
            field: field.into(),
            direction,
        }),
        limit,
    }
}

fn builder<S: SyncStorage>(db: &CommandDatabase<S>, query: &Query) -> QueryBuilder {
    let mut builder = db.query(&query.table);
    for predicate in &query.predicates {
        assert_eq!(
            predicate.values.len(),
            1,
            "fixture predicates are single-valued"
        );
        builder = builder.where_(
            &predicate.field,
            predicate.operator.clone(),
            predicate.values[0].clone(),
        );
    }
    if let Some(order) = &query.order {
        builder = builder.order_by(&order.field, order.direction.clone());
    }
    if let Some(limit) = query.limit {
        builder = builder.limit(limit);
    }
    builder
}

pub fn fetch<S: SyncStorage>(db: &CommandDatabase<S>, query: &Query) -> Vec<Row> {
    run(db.fetch(&builder(db, query))).unwrap()
}

/// The API-0003 oracle: every row sorted by value then ascending id, truncated.
fn oracle<S: SyncStorage>(db: &CommandDatabase<S>, query: &Query) -> Vec<Row> {
    let mut rows = run(db.fetch(&db.query(&query.table))).unwrap();
    let order = query.order.as_ref().unwrap();
    rows.sort_by(|left, right| {
        let value = left.fields[&order.field].cmp(&right.fields[&order.field]);
        (if order.direction == Direction::Desc {
            value.reverse()
        } else {
            value
        })
        .then_with(|| left.id.cmp(&right.id))
    });
    rows.truncate(query.limit.unwrap_or(usize::MAX));
    rows
}

/// SI-Q01/SI-Q02 exactness: every eligible shape equals the oracle.
pub fn ordered_limits_match_the_oracle<S: SyncStorage>(store: S, n: i64) {
    let mut db = open(store);
    seed(&mut db, n);
    let total = (n + 8) as usize;
    for field in ["rank", "label", "score", "flag", "at", "link"] {
        for direction in [Direction::Asc, Direction::Desc] {
            for limit in [0, 1, 3, 5, 9, total, total + 7] {
                let query = ordered(field, direction.clone(), Some(limit));
                assert_eq!(
                    fetch(&db, &query),
                    oracle(&db, &query),
                    "{field} {direction:?} limit {limit}"
                );
            }
        }
    }
    let empty = Query {
        table: "empty".into(),
        ..ordered("rank", Direction::Desc, Some(3))
    };
    assert!(fetch(&db, &empty).is_empty());
}

/// SI-Q04: on raw TypedStorage and plain Database, a raw `Float(100)` in the
/// non-optional Integer `rank` column still sorts as Null through CASE, so a
/// DESC limit-1 query keeps returning the valid top row.
pub fn raw_wrong_kind_payload_sorts_as_null<S: SyncStorage>(store: S) {
    let mut db = open(store);
    seed(&mut db, 4);
    let top = fetch(&db, &ordered("rank", Direction::Desc, Some(1)));
    let mut store = db.into_storage();
    let mut wrong = item(0, "wrong", 0.0);
    wrong.insert("rank".into(), float(100.0));
    wrong.insert(ROW_VERSION.into(), Value::Integer(1));
    run(store.insert(
        "items",
        Row {
            id: "raw:wrong".into(),
            fields: wrong,
        },
    ))
    .unwrap();
    let projection = schema().storage_schema().unwrap();
    let query = ordered("rank", Direction::Desc, Some(1));
    let raw = run(store.query(&projection, &query)).unwrap();
    assert_eq!(raw.len(), 1);
    assert_eq!(raw[0].id, top[0].id, "raw TypedStorage");
    let plain = run(Database::open(store, schema(), vec![])).unwrap();
    let rows = run(plain.fetch(
        &plain
            .query("items")
            .order_by("rank", Direction::Desc)
            .limit(1),
    ))
    .unwrap();
    assert_eq!(rows, top, "plain Database");
}

/// Observation surface shared by command owners and plain reactive databases.
pub trait Owner {
    fn observe_rows(&mut self, query: &QueryBuilder) -> Result<Subscription, StorageError>;
    fn drain(&mut self) -> Vec<Notification>;
    fn window(&self, query: &QueryBuilder) -> Result<Vec<Row>, StorageError>;
    fn put(&mut self, id: &str, fields: BTreeMap<String, Value>);
    fn add(&mut self, fields: BTreeMap<String, Value>) -> Row;
    fn drop_row(&mut self, id: &str);
    fn builder(&self, table: &str) -> QueryBuilder;
}

impl<S: SyncStorage> Owner for CommandDatabase<S> {
    fn observe_rows(&mut self, query: &QueryBuilder) -> Result<Subscription, StorageError> {
        run(self.observe(query))
    }
    fn drain(&mut self) -> Vec<Notification> {
        self.drain_notifications(0)
    }
    fn window(&self, query: &QueryBuilder) -> Result<Vec<Row>, StorageError> {
        run(self.fetch(query))
    }
    fn put(&mut self, id: &str, fields: BTreeMap<String, Value>) {
        run(self.replace("items", id, fields)).unwrap();
    }
    fn add(&mut self, fields: BTreeMap<String, Value>) -> Row {
        run(self.insert("items", fields)).unwrap()
    }
    fn drop_row(&mut self, id: &str) {
        assert!(run(self.remove("items", id)).unwrap());
    }
    fn builder(&self, table: &str) -> QueryBuilder {
        self.query(table)
    }
}

impl<S: SchemaStorage + ObservationStorage> Owner for ReactiveDatabase<S> {
    fn observe_rows(&mut self, query: &QueryBuilder) -> Result<Subscription, StorageError> {
        run(query.observe(self))
    }
    fn drain(&mut self) -> Vec<Notification> {
        self.drain_notifications(0)
    }
    fn window(&self, query: &QueryBuilder) -> Result<Vec<Row>, StorageError> {
        run(self.fetch(query))
    }
    fn put(&mut self, id: &str, fields: BTreeMap<String, Value>) {
        run(self.replace("items", id, fields)).unwrap();
    }
    fn add(&mut self, fields: BTreeMap<String, Value>) -> Row {
        run(self.insert("items", fields)).unwrap()
    }
    fn drop_row(&mut self, id: &str) {
        assert!(run(self.remove("items", id)).unwrap());
    }
    fn builder(&self, table: &str) -> QueryBuilder {
        self.query(table)
    }
}

/// SI-Q04: after every update in a fixed sequence, each observed ordered
/// window equals a fresh query of the same shape. Includes Null (optional
/// `note`), DESC and ASC, rows leaving and entering the window, inserts and
/// removes. A registration that fails must fail the plain query too.
pub fn reactive_windows_match_queries_after_updates(owner: &mut impl Owner) {
    let shapes = [
        ("rank", Direction::Desc),
        ("rank", Direction::Asc),
        ("note", Direction::Desc),
        ("note", Direction::Asc),
        ("label", Direction::Desc),
    ];
    let mut watched = Vec::new();
    for (field, direction) in shapes {
        let query = owner.builder("items").order_by(field, direction).limit(4);
        match owner.observe_rows(&query) {
            Ok(token) => watched.push((token, query, None)),
            Err(error) => {
                assert!(owner.window(&query).is_err(), "{field}: {error:?}");
            }
        }
    }
    let mut check = |owner: &mut dyn Owner, step: &str| {
        let events = owner.drain();
        for (token, query, last) in &mut watched {
            let delivered = events
                .iter()
                .rev()
                .find(|event| event.subscription == *token)
                .map(|event| event.value.clone());
            let fresh = delivered.is_some();
            if let Some(value) = delivered {
                *last = Some(value);
            }
            let ids = |rows: &[Row]| rows.iter().map(|row| row.id.clone()).collect::<Vec<_>>();
            match (last.as_ref(), owner.window(query)) {
                // A delivery carries the exact ordered window. Between
                // deliveries (a reorder within the same set is not re-sent),
                // the delivered id set still equals a fresh query.
                (Some(Ok(ObservationValue::Rows(rows))), Ok(expected)) => {
                    if fresh {
                        assert_eq!(ids(rows), ids(&expected), "{step}: {query:?}");
                    }
                    let set = |ids: Vec<String>| {
                        ids.into_iter().collect::<std::collections::BTreeSet<_>>()
                    };
                    assert_eq!(set(ids(rows)), set(ids(&expected)), "{step}: {query:?}");
                }
                (Some(Err(_)), Err(_)) => {}
                (other, expected) => panic!("{step}: {query:?}: {other:?} vs {expected:?}"),
            }
        }
    };
    check(owner, "registration");
    let ranked = owner
        .window(
            &owner
                .builder("items")
                .order_by("rank", Direction::Desc)
                .limit(10),
        )
        .unwrap();
    let (top, filler) = (ranked[0].clone(), ranked[ranked.len() - 1].clone());
    let fields = |row: &Row, extra: &[(&str, Value)]| {
        let mut fields: BTreeMap<String, Value> = row
            .fields
            .iter()
            .filter(|(name, _)| name.as_str() != ROW_VERSION)
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        for (name, value) in extra {
            fields.insert((*name).into(), value.clone());
        }
        fields
    };
    owner.put(&top.id, fields(&top, &[("rank", Value::Integer(-500))]));
    check(owner, "top row leaves the DESC window");
    owner.put(
        &filler.id,
        fields(&filler, &[("rank", Value::Integer(1000))]),
    );
    check(owner, "filler enters the DESC window");
    owner.put(
        &top.id,
        fields(
            &top,
            &[
                ("rank", Value::Integer(-500)),
                ("note", Value::String("m".into())),
            ],
        ),
    );
    owner.put(
        &filler.id,
        fields(
            &filler,
            &[
                ("rank", Value::Integer(1000)),
                ("note", Value::String("a".into())),
            ],
        ),
    );
    check(owner, "notes set");
    owner.put(&top.id, fields(&top, &[("rank", Value::Integer(-500))]));
    check(owner, "note cleared to Null");
    let added = owner.add(item(450, "new", 1.5));
    check(owner, "insert into the window");
    owner.drop_row(&added.id);
    owner.drop_row(&filler.id);
    check(owner, "removes from the window");
}

/// A plain reactive database over seeded rows plus one raw wrong-kind row,
/// which plain `Database::open` accepts (SI-V01 pins this).
pub fn plain_with_wrong_kind<S: SyncStorage>(store: S) -> ReactiveDatabase<S> {
    let mut db = open(store);
    seed(&mut db, 6);
    let mut store = db.into_storage();
    let mut wrong = item(0, "wrong", 0.0);
    wrong.insert("rank".into(), float(100.0));
    wrong.insert(ROW_VERSION.into(), Value::Integer(1));
    run(store.insert(
        "items",
        Row {
            id: "raw:wrong".into(),
            fields: wrong,
        },
    ))
    .unwrap();
    run(Database::open(store, schema(), vec![]))
        .unwrap()
        .into_reactive()
}
