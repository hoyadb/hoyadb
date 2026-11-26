//! TC-0041 SI-O01/SI-O02: command staging touches only what a handler touches,
//! and its observable results equal snapshot staging.
use hoyadb_core::{command::*, model::*, storage::*};
use std::{
    collections::BTreeMap,
    future::Future,
    task::{Context, Poll, Waker},
};

/// Counts wakes so a test loop re-polls only after a real wake.
#[derive(Default)]
pub struct CountingWaker(std::sync::atomic::AtomicUsize);

impl std::task::Wake for CountingWaker {
    fn wake(self: std::sync::Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &std::sync::Arc<Self>) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

impl CountingWaker {
    pub fn wakes(&self) -> usize {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Drives a future to completion, re-polling only after each wake. A Pending
/// without a wake is a test failure, never a busy-poll.
pub fn run<F: Future>(future: F) -> F::Output {
    let counter = std::sync::Arc::new(CountingWaker::default());
    let waker = Waker::from(counter.clone());
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        let before = counter.wakes();
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => assert!(counter.wakes() > before, "suspended without a wake"),
        }
    }
}

// The TC-0026 data-model schema (tests/conformance/data_model.rs), copied
// because that module expects its includer's `fresh` adapter factory.
fn column(kind: ColumnType, optional: bool) -> Column {
    Column {
        kind,
        optional,
    }
}

fn schema(version: u32) -> SchemaDefinition {
    let mut tables = BTreeMap::new();
    tables.insert(
        "customers".into(),
        TableDefinition {
            columns: BTreeMap::from([("country".into(), column(ColumnType::String, false))]),
            relations: BTreeMap::from([(
                "orders".into(),
                RelationDefinition::HasMany {
                    table: "orders".into(),
                    foreign_key: "customer_id".into(),
                },
            )]),
        },
    );
    tables.insert(
        "orders".into(),
        TableDefinition {
            columns: BTreeMap::from([
                ("status".into(), column(ColumnType::String, false)),
                ("total_cents".into(), column(ColumnType::Integer, false)),
                ("placed_at".into(), column(ColumnType::Timestamp, false)),
                ("price".into(), column(ColumnType::Float, true)),
                ("active".into(), column(ColumnType::Boolean, true)),
                ("notes".into(), column(ColumnType::String, true)),
            ]),
            relations: BTreeMap::from([
                (
                    "customer".into(),
                    RelationDefinition::ToOne {
                        table: "customers".into(),
                        foreign_key: "customer_id".into(),
                        optional: true,
                    },
                ),
                (
                    "items".into(),
                    RelationDefinition::HasMany {
                        table: "order_items".into(),
                        foreign_key: "order_id".into(),
                    },
                ),
                (
                    "products".into(),
                    RelationDefinition::ManyToMany {
                        table: "products".into(),
                        through: "order_products".into(),
                        source_relation: "order".into(),
                        target_relation: "product".into(),
                    },
                ),
            ]),
        },
    );
    tables.insert(
        "products".into(),
        TableDefinition {
            columns: BTreeMap::from([("name".into(), column(ColumnType::String, false))]),
            relations: BTreeMap::from([(
                "orders".into(),
                RelationDefinition::ManyToMany {
                    table: "orders".into(),
                    through: "order_products".into(),
                    source_relation: "product".into(),
                    target_relation: "order".into(),
                },
            )]),
        },
    );
    tables.insert(
        "order_items".into(),
        TableDefinition {
            columns: BTreeMap::from([("quantity".into(), column(ColumnType::Integer, false))]),
            relations: BTreeMap::from([(
                "order".into(),
                RelationDefinition::ToOne {
                    table: "orders".into(),
                    foreign_key: "order_id".into(),
                    optional: false,
                },
            )]),
        },
    );
    tables.insert(
        "order_notes".into(),
        TableDefinition {
            columns: BTreeMap::from([("body".into(), column(ColumnType::String, false))]),
            relations: BTreeMap::new(),
        },
    );
    tables.insert(
        "order_products".into(),
        TableDefinition {
            columns: BTreeMap::new(),
            relations: BTreeMap::from([
                (
                    "order".into(),
                    RelationDefinition::ToOne {
                        table: "orders".into(),
                        foreign_key: "order_id".into(),
                        optional: false,
                    },
                ),
                (
                    "product".into(),
                    RelationDefinition::ToOne {
                        table: "products".into(),
                        foreign_key: "product_id".into(),
                        optional: false,
                    },
                ),
            ]),
        },
    );
    SchemaDefinition {
        version,
        tables,
    }
}

fn text(value: &str) -> Value {
    Value::String(value.into())
}

fn reference(id: &str) -> Value {
    Value::Reference(id.into())
}

pub fn definition() -> CommandDefinition {
    CommandDefinition {
        name: "overlay-script-v1".into(),
        arguments: BTreeMap::new(),
        reference_paths: vec![],
    }
}

pub fn open<S: SyncStorage>(store: S) -> CommandDatabase<S> {
    run(CommandDatabase::open(
        store,
        schema(1),
        vec![],
        [0x0b; 16],
        SyncOptions::default(),
    ))
    .unwrap()
}

pub fn begin<S: SyncStorage>(db: &mut CommandDatabase<S>) -> CommandRequest {
    let request = db.request().unwrap();
    assert!(
        run(db.begin(
            request,
            definition(),
            Json::Object(BTreeMap::new()),
            Some(Json::Null),
            1000,
        ))
        .unwrap()
    );
    request
}

fn order(customer: Option<&str>, status: &str, total: i64) -> BTreeMap<String, Value> {
    let mut fields = BTreeMap::from([
        ("status".into(), text(status)),
        ("total_cents".into(), Value::Integer(total)),
        ("placed_at".into(), Value::Timestamp(total * 10)),
    ]);
    if let Some(customer) = customer {
        fields.insert("customer_id".into(), reference(customer));
    }
    fields
}

fn put<S: SyncStorage>(
    db: &mut CommandDatabase<S>,
    table: &str,
    id: &str,
    fields: BTreeMap<String, Value>,
) {
    run(db.insert_proposed(table, fields, Some(id.into()))).unwrap();
}

/// Customers c1 (FR), c2 (DE), c3 (US); orders o1-o6; products p1, p2; one
/// item and one product link. `filler` unrelated customers add N.
pub fn seed<S: SyncStorage>(db: &mut CommandDatabase<S>, filler: usize) {
    for (id, country) in [("c1", "FR"), ("c2", "DE"), ("c3", "US")] {
        put(
            db,
            "customers",
            id,
            BTreeMap::from([("country".into(), text(country))]),
        );
    }
    for index in 0..filler {
        let id = format!("filler:{index:05}");
        put(
            db,
            "customers",
            &id,
            BTreeMap::from([("country".into(), text("ZZ"))]),
        );
    }
    for (id, customer, status, total) in [
        ("o1", Some("c1"), "open", 500),
        ("o2", Some("c1"), "paid", 300),
        ("o3", Some("c2"), "open", 400),
        ("o4", Some("c3"), "paid", 100),
        ("o5", None, "open", 200),
        ("o6", Some("c2"), "paid", 600),
    ] {
        put(db, "orders", id, order(customer, status, total));
    }
    for id in ["p1", "p2"] {
        put(
            db,
            "products",
            id,
            BTreeMap::from([("name".into(), text(id))]),
        );
    }
    put(
        db,
        "order_items",
        "i1",
        BTreeMap::from([
            ("quantity".into(), Value::Integer(2)),
            ("order_id".into(), reference("o1")),
        ]),
    );
    put(
        db,
        "order_products",
        "op1",
        BTreeMap::from([
            ("order_id".into(), reference("o1")),
            ("product_id".into(), reference("p1")),
        ]),
    );
}

/// SI-O01: a handler that fetches one row by id and replaces it, measured from
/// begin through handler completion (the commit is SI-Y07, Phase 3).
pub fn one_row_handler_work<S: SyncStorage>(store: S, filler: usize) -> counters::WorkSnapshot {
    let mut db = open(store);
    seed(&mut db, filler);
    counters::reset();
    let request = begin(&mut db);
    let found = run(db.scope_fetch(
        request,
        &db.query("customers").where_("id", Operator::Eq, text("c2")),
    ))
    .unwrap();
    assert_eq!(found.len(), 1);
    run(db.scope_replace(
        request,
        "customers",
        "c2",
        BTreeMap::from([("country".into(), text("FR"))]),
    ))
    .unwrap();
    let work = counters::snapshot();
    db.abort(request).unwrap();
    work
}

fn france(db: &CommandDatabase<impl SyncStorage>) -> QueryBuilder {
    db.query("orders")
        .where_("customer.country", Operator::Eq, text("FR"))
}

/// SI-O02: a fixed handler script whose transcript must equal snapshot
/// staging's. Each line is one scope result, then the committed outcome and the
/// post-commit count observation and stored rows.
pub fn handler_script<S: SyncStorage>(store: S) -> Vec<String> {
    let mut db = open(store);
    seed(&mut db, 3);
    let counted = run(db.observe_count(&france(&db))).unwrap();
    db.drain_notifications(0);
    let mut lines = Vec::new();
    let request = begin(&mut db);
    macro_rules! record {
        ($label:expr, $value:expr) => {
            lines.push(format!("{}: {:?}", $label, $value))
        };
    }
    let fetch = |db: &CommandDatabase<S>, query: QueryBuilder| run(db.scope_fetch(request, &query));
    let top_fr =
        |db: &CommandDatabase<S>| france(db).order_by("total_cents", Direction::Desc).limit(2);
    record!("fr top 2", fetch(&db, top_fr(&db)));
    // A target edit changes ToOne membership; one id reaches R through two paths.
    record!(
        "c2 to FR",
        run(db.scope_replace(
            request,
            "customers",
            "c2",
            BTreeMap::from([("country".into(), text("FR"))])
        ))
    );
    record!("fr top 2 after", fetch(&db, top_fr(&db)));
    record!(
        "fr by country",
        fetch(
            &db,
            france(&db).order_by("customer.country", Direction::Asc)
        )
    );
    record!(
        "two predicates on one target",
        fetch(
            &db,
            france(&db).where_("customer.country", Operator::Ne, text("US"))
        )
    );
    // A dangling reference resolved by an in-scope insert.
    record!(
        "dangling order",
        run(db.scope_insert_proposed(
            request,
            "orders",
            order(Some("local:c-new"), "open", 700),
            Some("local:o7".into())
        ))
    );
    let italy = |db: &CommandDatabase<S>| {
        db.query("orders")
            .where_("customer.country", Operator::Eq, text("IT"))
    };
    record!("italy before", fetch(&db, italy(&db)));
    record!(
        "insert c-new",
        run(db.scope_insert_proposed(
            request,
            "customers",
            BTreeMap::from([("country".into(), text("IT"))]),
            Some("local:c-new".into())
        ))
    );
    record!("italy after", fetch(&db, italy(&db)));
    // A delete that leaves references dangling; dangling targets order as Null.
    record!(
        "remove c3",
        run(db.scope_remove(request, "customers", "c3"))
    );
    record!(
        "by country with dangling",
        fetch(
            &db,
            db.query("orders")
                .order_by("customer.country", Direction::Asc)
                .limit(10)
        )
    );
    // Ordered windows gaining and losing delta rows.
    record!(
        "o4 to top",
        run(db.scope_replace(request, "orders", "o4", order(None, "paid", 900)))
    );
    let top3 = |db: &CommandDatabase<S>| {
        db.query("orders")
            .order_by("total_cents", Direction::Desc)
            .limit(3)
    };
    record!("top 3", fetch(&db, top3(&db)));
    record!("remove o6", run(db.scope_remove(request, "orders", "o6")));
    record!("top 3 after", fetch(&db, top3(&db)));
    record!("limit 0", fetch(&db, top3(&db).limit(0)));
    record!(
        "unbounded open",
        fetch(
            &db,
            db.query("orders")
                .where_("status", Operator::Eq, text("open"))
        )
    );
    // Navigation: HasMany and ManyToMany with through-row edits.
    record!(
        "c1 orders",
        fetch(&db, db.related("customers", "c1", "orders"))
    );
    record!(
        "new c1 order",
        run(db.scope_insert_proposed(
            request,
            "orders",
            order(Some("c1"), "open", 50),
            Some("local:o8".into())
        ))
    );
    record!(
        "c1 orders after",
        fetch(&db, db.related("customers", "c1", "orders"))
    );
    record!(
        "o1 products",
        fetch(&db, db.related("orders", "o1", "products"))
    );
    record!(
        "link p2",
        run(db.scope_link(request, "orders", "o1", "products", "p2", BTreeMap::new()))
    );
    record!(
        "o1 products linked",
        fetch(&db, db.related("orders", "o1", "products"))
    );
    record!(
        "unlink p1",
        run(db.scope_unlink(request, "orders", "o1", "products", "p1"))
    );
    record!(
        "o1 products unlinked",
        fetch(&db, db.related("orders", "o1", "products"))
    );
    // Read-your-own-writes by id, and a generated id.
    record!(
        "read own insert",
        fetch(
            &db,
            db.query("orders")
                .where_("id", Operator::Eq, text("local:o8"))
        )
    );
    record!(
        "generated insert",
        run(db.scope_insert(
            request,
            "products",
            BTreeMap::from([("name".into(), text("p3"))])
        ))
    );
    record!("products", fetch(&db, db.query("products")));
    let outcome = run(db.finish(request, Json::Null)).map(|result| (result.key, result.result));
    record!("finish", outcome);
    let counts: Vec<_> = db
        .drain_notifications(10_000)
        .into_iter()
        .filter(|event| event.subscription == counted)
        .map(|event| event.value)
        .collect();
    record!("count observation", counts);
    record!(
        "count query",
        run(db.fetch(&france(&db))).map(|rows| rows.len())
    );
    for table in [
        "customers",
        "orders",
        "products",
        "order_items",
        "order_products",
    ] {
        record!(table, run(db.fetch(&db.query(table))));
    }
    lines
}

/// SI-O02 pinning: `HOYA_PIN_OVERLAY=<dir>` writes the transcript produced by
/// the code under test; otherwise a pinned transcript must match exactly.
pub fn assert_pinned(name: &str, lines: &[String]) {
    let golden = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/overlay-golden")
        .join(format!("{name}.txt"));
    let text = lines.join("\n") + "\n";
    if let Some(directory) = std::env::var_os("HOYA_PIN_OVERLAY") {
        let target = std::path::Path::new(&directory).join(format!("{name}.txt"));
        std::fs::write(target, &text).unwrap();
        return;
    }
    let pinned = std::fs::read_to_string(&golden).expect("pinned SI-O02 transcript");
    assert_eq!(
        text, pinned,
        "{name} differs from the transcript pinned on snapshot staging"
    );
}

/// SI-O02: a predicate-free ordered limit inside a scope reads its base window
/// through the validated ordered path and merges staged rows exactly. Returns
/// the scope result and the work of that one read.
pub fn fast_path_overlay_read<S: SyncStorage>(
    store: S,
    filler: i64,
) -> (Vec<String>, counters::WorkSnapshot) {
    let mut db = open(store);
    seed(&mut db, 0);
    for index in 0..filler {
        put(
            &mut db,
            "orders",
            &format!("filler:{index:05}"),
            order(None, "old", -1 - index),
        );
    }
    let request = begin(&mut db);
    // One staged row enters the window, one leaves it, one is removed.
    run(db.scope_replace(request, "orders", "o4", order(None, "paid", 650))).unwrap();
    run(db.scope_replace(request, "orders", "o6", order(Some("c2"), "paid", 10))).unwrap();
    assert!(run(db.scope_remove(request, "orders", "o1")).unwrap());
    let query = db
        .query("orders")
        .order_by("total_cents", Direction::Desc)
        .limit(3);
    counters::reset();
    let rows = run(db.scope_fetch(request, &query)).unwrap();
    let work = counters::snapshot();
    db.abort(request).unwrap();
    (rows.into_iter().map(|row| row.id).collect(), work)
}

/// A self-referential ToOne shares the candidate table with its relation targets. Scope
/// queries over staged children and parents must equal the committed store's answers.
pub fn self_relation_scope_queries<S: SyncStorage>(store: S) -> Vec<Vec<String>> {
    let nodes = SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([(
            "nodes".into(),
            TableDefinition {
                columns: BTreeMap::from([("name".into(), column(ColumnType::String, false))]),
                relations: BTreeMap::from([(
                    "parent".into(),
                    RelationDefinition::ToOne {
                        table: "nodes".into(),
                        foreign_key: "parent_id".into(),
                        optional: true,
                    },
                )]),
            },
        )]),
    };
    let mut db = run(CommandDatabase::open(
        store,
        nodes,
        vec![],
        [0x0c; 16],
        SyncOptions::default(),
    ))
    .unwrap();
    let node = |name: &str, parent: Option<&str>| {
        let mut fields = BTreeMap::from([("name".into(), text(name))]);
        if let Some(parent) = parent {
            fields.insert("parent_id".into(), reference(parent));
        }
        fields
    };
    put(&mut db, "nodes", "t", node("T", None));
    put(&mut db, "nodes", "u", node("U", Some("t")));
    let request = begin(&mut db);
    // Both ends staged (A -> B), and a staged child of an unstaged parent (C -> t).
    let b = run(db.scope_insert(request, "nodes", node("B", None))).unwrap();
    run(db.scope_insert(request, "nodes", node("A", Some(&b.id)))).unwrap();
    run(db.scope_insert(request, "nodes", node("C", Some("t")))).unwrap();
    let queries = |db: &CommandDatabase<S>| {
        vec![
            db.query("nodes")
                .where_("parent.name", Operator::Eq, text("B")),
            db.query("nodes")
                .where_("parent.name", Operator::Eq, text("T")),
            db.query("nodes").order_by("parent.name", Direction::Asc),
            db.query("nodes")
                .order_by("parent.name", Direction::Asc)
                .limit(2),
        ]
    };
    let ids = |rows: Vec<Row>| {
        rows.into_iter()
            .map(|row| match &row.fields["name"] {
                Value::String(name) => name.clone(),
                other => panic!("node name {other:?}"),
            })
            .collect::<Vec<_>>()
    };
    let staged: Vec<_> = queries(&db)
        .iter()
        .map(|query| ids(run(db.scope_fetch(request, query)).unwrap()))
        .collect();
    run(db.finish(request, Json::Null)).unwrap();
    let committed: Vec<_> = queries(&db)
        .iter()
        .map(|query| ids(run(db.fetch(query)).unwrap()))
        .collect();
    assert_eq!(staged, committed, "staged answers differ from the commit");
    staged
}
