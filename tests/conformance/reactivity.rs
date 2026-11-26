//! TC-0029 R01-R10/R12/R15: shared production-core observation assertions.
use super::fresh;
use super::model_conformance::{fields, order, run, schema};
use hoyadb_core::{model::*, storage::*};
use std::{
    collections::BTreeMap,
    future::{Future, ready},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

fn rows(events: &[Notification], token: Subscription) -> Vec<Vec<Row>> {
    events
        .iter()
        .filter(|event| event.subscription == token)
        .map(|event| match &event.value {
            Ok(ObservationValue::Rows(rows)) => rows.clone(),
            value => panic!("expected owned rows, got {value:?}"),
        })
        .collect()
}

fn counts(events: &[Notification], token: Subscription) -> Vec<usize> {
    events
        .iter()
        .filter(|event| event.subscription == token)
        .map(|event| match &event.value {
            Ok(ObservationValue::Count(count)) => *count,
            value => panic!("expected count, got {value:?}"),
        })
        .collect()
}

fn pending<S: SchemaStorage>(db: &Database<S>) -> QueryBuilder {
    db.query("orders")
        .where_("status", Operator::Eq, Value::String("pending".into()))
}

fn changed(row: &Row, field: &str, value: Value) -> BTreeMap<String, Value> {
    let mut result = row.fields.clone();
    result.insert(field.into(), value);
    result
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn r01_owned_initial_and_membership_only() {
    let mut db = run(Database::open(fresh(), schema(1), vec![]))
        .unwrap()
        .into_reactive();
    let a = run(db.insert("orders", order(1))).unwrap();
    let b = run(db.insert("orders", order(2))).unwrap();
    let c = run(db.insert("orders", order(3))).unwrap();
    let query = pending(&db);
    assert_eq!(
        run(query.fetch(&db)).unwrap(),
        vec![a.clone(), b.clone(), c.clone()]
    );
    let token = run(query.observe(&mut db)).unwrap();
    let mut initial = db.drain_notifications(0);
    assert_eq!(
        rows(&initial, token),
        vec![vec![a.clone(), b.clone(), c.clone()]]
    );
    if let Ok(ObservationValue::Rows(snapshot)) = &mut initial[0].value {
        snapshot[0].fields.clear();
    }
    assert_eq!(
        run(query.fetch(&db)).unwrap(),
        vec![a.clone(), b.clone(), c.clone()]
    );
    let a2 = run(db.replace(
        "orders",
        &a.id,
        changed(&a, "notes", Value::String("edited".into())),
    ))
    .unwrap();
    assert!(db.drain_notifications(1).is_empty());
    let d = run(db.insert("orders", order(4))).unwrap();
    assert_eq!(
        rows(&db.drain_notifications(2), token),
        vec![vec![a2.clone(), b.clone(), c.clone(), d.clone()]]
    );
    run(db.replace(
        "orders",
        &b.id,
        changed(&b, "status", Value::String("shipped".into())),
    ))
    .unwrap();
    assert_eq!(
        rows(&db.drain_notifications(3), token),
        vec![vec![a2.clone(), c.clone(), d.clone()]]
    );
    run(db.apply_batch(vec![
        Write::Remove {
            table: "orders".into(),
            id: c.id,
        },
        Write::Insert {
            table: "orders".into(),
            fields: order(5),
        },
    ]))
    .unwrap();
    let final_rows = run(query.fetch(&db)).unwrap();
    assert_eq!(final_rows.len(), 3);
    assert_eq!(rows(&db.drain_notifications(4), token), vec![final_rows]);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn r02_cancel_close_and_cross_owner_handles() {
    let mut db = run(Database::open(fresh(), schema(1), vec![]))
        .unwrap()
        .into_reactive();
    let query = pending(&db);
    let first = run(query.observe(&mut db)).unwrap();
    let second = run(query.observe(&mut db)).unwrap();
    assert_ne!(first, second);
    assert_eq!(db.observation_stats().groups, 1);
    assert_eq!(db.observation_stats().observers, 2);
    assert!(db.unsubscribe(first));
    assert!(!db.unsubscribe(first));
    let initial = db.drain_notifications(0);
    assert!(rows(&initial, first).is_empty());
    assert_eq!(rows(&initial, second), vec![Vec::<Row>::new()]);
    run(db.insert("orders", order(1))).unwrap();
    assert!(db.unsubscribe(second));
    assert!(db.drain_notifications(16).is_empty());
    assert_eq!(db.observation_stats().groups, 0);
    assert_eq!(db.observation_stats().observers, 0);
    let before = db.observation_stats();
    run(db.insert("orders", order(2))).unwrap();
    let after = db.observation_stats();
    assert_eq!(after.projected_queries, before.projected_queries);
    assert_eq!(after.candidate_updates, before.candidate_updates);
    assert_eq!(after.materialized_rows, before.materialized_rows);
    let live = run(query.observe_count(&mut db)).unwrap();
    db.drain_notifications(32);
    run(db.insert("orders", order(3))).unwrap();
    let store = db.into_storage();
    let mut reopened = run(Database::open(store, schema(1), vec![]))
        .unwrap()
        .into_reactive();
    assert!(reopened.drain_notifications(100).is_empty());
    let next = run(pending(&reopened).observe_count(&mut reopened)).unwrap();
    assert_ne!(next, live);
    assert!(!reopened.unsubscribe(live));
    assert_eq!(counts(&reopened.drain_notifications(100), next), vec![3]);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn r03_net_batches_rollback_and_commit_snapshots() {
    let mut db = run(Database::open(fresh(), schema(1), vec![]))
        .unwrap()
        .with_id_generator(|| "fixed".into())
        .into_reactive();
    let a = run(db.insert("orders", order(1))).unwrap();
    let query = pending(&db);
    let membership = run(query.observe(&mut db)).unwrap();
    let finer = run(query.observe_with_columns(&mut db, vec!["total_cents".into()])).unwrap();
    db.drain_notifications(0);
    run(db.apply_batch(vec![
        Write::Replace {
            table: "orders".into(),
            id: a.id.clone(),
            fields: order(2),
        },
        Write::Replace {
            table: "orders".into(),
            id: a.id.clone(),
            fields: a.fields.clone(),
        },
    ]))
    .unwrap();
    assert!(db.drain_notifications(1).is_empty());
    run(db.apply_batch(vec![
        Write::Insert {
            table: "products".into(),
            fields: fields([("name", Value::String("temporary".into()))]),
        },
        Write::Remove {
            table: "products".into(),
            id: "fixed".into(),
        },
    ]))
    .unwrap();
    assert!(db.drain_notifications(2).is_empty());
    let failed = run(db.apply_batch(vec![
        Write::Replace {
            table: "orders".into(),
            id: a.id.clone(),
            fields: order(9),
        },
        Write::Insert {
            table: "orders".into(),
            fields: order(10),
        },
    ]));
    assert_eq!(
        failed,
        Err(StorageError::RowAlreadyExists {
            table: "orders".into(),
            id: a.id.clone()
        })
    );
    assert!(db.drain_notifications(3).is_empty());
    assert_eq!(run(query.fetch(&db)).unwrap(), vec![a.clone()]);
    let a2 = run(db.replace("orders", &a.id, order(2))).unwrap();
    let a3 = run(db.replace("orders", &a.id, order(3))).unwrap();
    let events = db.drain_notifications(4);
    assert!(rows(&events, membership).is_empty());
    assert_eq!(rows(&events, finer), vec![vec![a2], vec![a3]]);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn r04_own_columns_missing_null_and_registration_validation() {
    let mut db = run(Database::open(fresh(), schema(1), vec![]))
        .unwrap()
        .into_reactive();
    let a = run(db.insert("orders", order(1))).unwrap();
    let query = pending(&db);
    let ordinary = run(query.observe(&mut db)).unwrap();
    let finer = run(query.observe_with_columns(
        &mut db,
        vec!["notes".into(), "total_cents".into(), "notes".into()],
    ))
    .unwrap();
    let empty = run(query.observe_with_columns(&mut db, vec![])).unwrap();
    db.drain_notifications(0);
    let a2 = run(db.replace("orders", &a.id, changed(&a, "notes", Value::Null))).unwrap();
    let events = db.drain_notifications(1);
    assert_eq!(rows(&events, finer), vec![vec![a2.clone()]]);
    assert!(rows(&events, ordinary).is_empty());
    assert!(rows(&events, empty).is_empty());
    run(db.replace("orders", &a.id, a.fields.clone())).unwrap();
    assert_eq!(
        rows(&db.drain_notifications(2), finer),
        vec![vec![a.clone()]]
    );
    let a3 = run(db.replace(
        "orders",
        &a.id,
        changed(&a, "total_cents", Value::Integer(42)),
    ))
    .unwrap();
    assert_eq!(
        rows(&db.drain_notifications(3), finer),
        vec![vec![a3.clone()]]
    );
    run(db.replace(
        "orders",
        &a.id,
        changed(&a3, "placed_at", Value::Timestamp(99)),
    ))
    .unwrap();
    assert!(db.drain_notifications(4).is_empty());
    for column in ["missing", "__hoya_schema_version", "customer.country"] {
        let before = db.observation_stats();
        assert!(matches!(
            run(query.observe_with_columns(&mut db, vec![column.into()])),
            Err(StorageError::InvalidQuery { .. })
        ));
        assert_eq!(db.observation_stats(), before);
        assert!(db.drain_notifications(5).is_empty());
    }
    let invalid = db
        .query("orders")
        .where_("missing", Operator::Eq, Value::Integer(0))
        .limit(0);
    let before = db.observation_stats();
    assert!(matches!(
        run(invalid.observe_count(&mut db)),
        Err(StorageError::InvalidQuery { .. })
    ));
    assert_eq!(db.observation_stats(), before);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn r05_count_throttle_deadline_return_and_backwards_clock() {
    let trace = Arc::new(Trace::default());
    let wrapped = Traced {
        inner: fresh(),
        trace: trace.clone(),
    };
    let mut db = run(Database::open(wrapped, schema(1), vec![]))
        .unwrap()
        .into_reactive();
    let query = pending(&db);
    trace.clear();
    let token = run(query.observe_count(&mut db)).unwrap();
    assert_eq!(counts(&db.drain_notifications(100), token), vec![0]);
    for i in 0..10 {
        run(db.insert("orders", order(i))).unwrap();
    }
    assert!(db.drain_notifications(115).is_empty());
    assert_eq!(counts(&db.drain_notifications(116), token), vec![10]);
    run(db.insert("orders", order(11))).unwrap();
    assert!(db.drain_notifications(90).is_empty());
    assert!(db.drain_notifications(131).is_empty());
    assert_eq!(counts(&db.drain_notifications(132), token), vec![11]);
    let temporary = run(db.insert("orders", order(12))).unwrap();
    run(db.remove("orders", &temporary.id)).unwrap();
    assert!(db.drain_notifications(148).is_empty());
    assert_eq!(
        trace.full_queries.load(Ordering::SeqCst),
        0,
        "count must not materialize Rows via TypedStorage::query"
    );
    assert_eq!(db.observation_stats().materialized_rows, 0);
    let empty = db.query("orders").limit(0);
    let zero = run(empty.observe_count(&mut db)).unwrap();
    assert_eq!(counts(&db.drain_notifications(149), zero), vec![0]);
    run(db.insert("orders", order(13))).unwrap();
    let events = db.drain_notifications(164);
    assert!(counts(&events, zero).is_empty());
    assert_eq!(counts(&events, token), vec![12]);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn r06_normalized_equivalent_queries_share_across_modes() {
    let mut db = run(Database::open(fresh(), schema(1), vec![]))
        .unwrap()
        .into_reactive();
    let a = run(db.insert("orders", order(1))).unwrap();
    let first = db
        .query("orders")
        .where_("status", Operator::Eq, Value::String("pending".into()))
        .where_in(
            "total_cents",
            vec![Value::Integer(2), Value::Integer(1), Value::Integer(2)],
        );
    let second = db
        .query("orders")
        .where_in("total_cents", vec![Value::Integer(1), Value::Integer(2)])
        .where_("status", Operator::Eq, Value::String("pending".into()))
        .where_("status", Operator::Eq, Value::String("pending".into()));
    let rows_token = run(first.observe(&mut db)).unwrap();
    let before = db.observation_stats();
    let count_token = run(second.observe_count(&mut db)).unwrap();
    let finer = run(second.observe_with_columns(&mut db, vec!["notes".into()])).unwrap();
    let after = db.observation_stats();
    assert_eq!(after.groups, 1);
    assert_eq!(after.observers, 3);
    assert_eq!(
        after.candidate_updates, before.candidate_updates,
        "registration must reuse existing membership cache"
    );
    let initial = db.drain_notifications(0);
    assert_eq!(rows(&initial, rows_token), vec![vec![a.clone()]]);
    assert_eq!(rows(&initial, finer), vec![vec![a.clone()]]);
    assert_eq!(counts(&initial, count_token), vec![1]);
    let b = run(db.insert("orders", order(2))).unwrap();
    let updated = db.observation_stats();
    assert!(
        updated.candidate_updates - after.candidate_updates <= 1,
        "identical observers must share candidate maintenance"
    );
    let events = db.drain_notifications(16);
    assert_eq!(rows(&events, rows_token), vec![vec![a.clone(), b.clone()]]);
    assert_eq!(rows(&events, finer), vec![vec![a, b]]);
    assert_eq!(counts(&events, count_token), vec![2]);
    assert!(db.unsubscribe(rows_token));
    assert_eq!(db.observation_stats().groups, 1);
    assert!(db.unsubscribe(count_token));
    assert!(db.unsubscribe(finer));
    assert_eq!(db.observation_stats().groups, 0);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn r07_ordered_limit_refill_ties_and_reorder_suppression() {
    let mut db = run(Database::open(fresh(), schema(1), vec![]))
        .unwrap()
        .into_reactive();
    let a = run(db.insert("orders", order(1))).unwrap();
    let b = run(db.insert("orders", order(2))).unwrap();
    let c = run(db.insert("orders", order(2))).unwrap();
    let d = run(db.insert("orders", order(4))).unwrap();
    let query = pending(&db).order_by("placed_at", Direction::Desc).limit(2);
    let token = run(query.observe(&mut db)).unwrap();
    assert_eq!(
        rows(&db.drain_notifications(0), token),
        vec![vec![d.clone(), b.clone()]]
    );
    let b3 = run(db.replace(
        "orders",
        &b.id,
        changed(&b, "placed_at", Value::Timestamp(5)),
    ))
    .unwrap();
    assert!(
        db.drain_notifications(1).is_empty(),
        "same visible IDs reordered only"
    );
    run(db.remove("orders", &d.id)).unwrap();
    assert_eq!(
        rows(&db.drain_notifications(2), token),
        vec![vec![b3.clone(), c.clone()]]
    );
    run(db.replace(
        "orders",
        &a.id,
        changed(&a, "placed_at", Value::Timestamp(6)),
    ))
    .unwrap();
    let expected = run(query.fetch(&db)).unwrap();
    assert_eq!(
        expected.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec![a.id.as_str(), b.id.as_str()]
    );
    assert_eq!(rows(&db.drain_notifications(3), token), vec![expected]);
    let null_order = db
        .query("orders")
        .order_by("notes", Direction::Asc)
        .limit(2);
    let optional = run(null_order.observe(&mut db)).unwrap();
    assert_eq!(
        rows(&db.drain_notifications(4), optional),
        vec![run(null_order.fetch(&db)).unwrap()]
    );
    let zero = run(db.query("orders").limit(0).observe(&mut db)).unwrap();
    let oversized = run(db.query("orders").limit(100).observe(&mut db)).unwrap();
    let initial = db.drain_notifications(5);
    assert_eq!(rows(&initial, zero), vec![Vec::<Row>::new()]);
    assert_eq!(
        rows(&initial, oversized),
        vec![run(db.query("orders").fetch(&db)).unwrap()]
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn r08_to_one_dependencies_dangling_and_unrelated_writes() {
    // TC-0030 / DEF-0005: the ordering role must normalize invalid target
    // cells without applying strict requested-cell validation to them.
    let mut ordinary = run(Database::open(fresh(), schema(1), vec![])).unwrap();
    let customer = run(ordinary.insert(
        "customers",
        fields([("country", Value::String("FR".into()))]),
    ))
    .unwrap();
    let mut input = order(1);
    input.insert("customer_id".into(), Value::Reference(customer.id.clone()));
    let source = run(ordinary.insert("orders", input)).unwrap();
    let mut storage = ordinary.into_storage();
    let mut raw = run(storage.get("customers", &customer.id))
        .unwrap()
        .unwrap();
    raw.fields.insert("country".into(), Value::Integer(5));
    run(storage.replace("customers", raw)).unwrap();
    let mut normalized = run(Database::open(storage, schema(1), vec![]))
        .unwrap()
        .into_reactive();
    let q = normalized
        .query("orders")
        .order_by("customer.country", Direction::Asc)
        .limit(1);
    assert_eq!(run(q.fetch(&normalized)).unwrap(), vec![source.clone()]);
    let count = run(q.observe_count(&mut normalized)).unwrap();
    let token = run(q.observe(&mut normalized)).unwrap();
    let events = normalized.drain_notifications(0);
    assert_eq!(counts(&events, count), vec![1]);
    assert_eq!(rows(&events, token), vec![vec![source]]);
    let trace = Arc::new(Trace::default());
    let mut db = run(Database::open(
        Traced {
            inner: fresh(),
            trace: trace.clone(),
        },
        schema(1),
        vec![],
    ))
    .unwrap()
    .into_reactive();
    let fr = run(db.insert(
        "customers",
        fields([("country", Value::String("FR".into()))]),
    ))
    .unwrap();
    let de = run(db.insert(
        "customers",
        fields([("country", Value::String("DE".into()))]),
    ))
    .unwrap();
    let mut related = order(1);
    related.insert("customer_id".into(), Value::Reference(fr.id.clone()));
    let a = run(db.insert("orders", related)).unwrap();
    let mut related = order(2);
    related.insert("customer_id".into(), Value::Reference(fr.id.clone()));
    let b = run(db.insert("orders", related)).unwrap();
    let mut unrelated = order(3);
    unrelated.insert("customer_id".into(), Value::Reference(de.id.clone()));
    let c = run(db.insert("orders", unrelated)).unwrap();
    run(db.insert("orders", order(4))).unwrap();
    let q = db
        .query("orders")
        .where_("customer.country", Operator::Eq, Value::String("FR".into()))
        .order_by("customer.country", Direction::Asc);
    let token = run(q.observe(&mut db)).unwrap();
    assert_eq!(
        rows(&db.drain_notifications(0), token),
        vec![vec![a.clone(), b.clone()]]
    );
    trace.clear();
    let before = db.observation_stats();
    run(db.insert(
        "products",
        fields([("name", Value::String("unrelated".into()))]),
    ))
    .unwrap();
    assert!(db.drain_notifications(1).is_empty());
    assert!(trace.projects.lock().unwrap().is_empty());
    assert_eq!(
        db.observation_stats().candidate_updates,
        before.candidate_updates
    );
    run(db.replace(
        "customers",
        &fr.id,
        fields([("country", Value::String("DE".into()))]),
    ))
    .unwrap();
    assert_eq!(
        rows(&db.drain_notifications(2), token),
        vec![Vec::<Row>::new()]
    );
    assert!(db.observation_stats().candidate_updates - before.candidate_updates <= 2);
    run(db.replace(
        "customers",
        &de.id,
        fields([("country", Value::String("FR".into()))]),
    ))
    .unwrap();
    assert_eq!(
        rows(&db.drain_notifications(3), token),
        vec![vec![c.clone()]]
    );
    run(db.remove("customers", &de.id)).unwrap();
    assert_eq!(
        rows(&db.drain_notifications(4), token),
        vec![Vec::<Row>::new()]
    );
    let mut definition = schema(1);
    definition
        .tables
        .get_mut("customers")
        .unwrap()
        .relations
        .insert(
            "parent".into(),
            RelationDefinition::ToOne {
                table: "customers".into(),
                foreign_key: "parent_id".into(),
                optional: true,
            },
        );
    let mut cyclic = run(Database::open(fresh(), definition, vec![]))
        .unwrap()
        .with_id_generator({
            let mut n = 0;
            move || {
                n += 1;
                format!("id{n}")
            }
        })
        .into_reactive();
    let first = run(cyclic.insert(
        "customers",
        fields([
            ("country", Value::String("FR".into())),
            ("parent_id", Value::Reference("id2".into())),
        ]),
    ))
    .unwrap();
    let cycle_query = cyclic.query("customers").where_(
        "parent.country",
        Operator::Eq,
        Value::String("FR".into()),
    );
    let cycle_token = run(cycle_query.observe(&mut cyclic)).unwrap();
    assert_eq!(
        rows(&cyclic.drain_notifications(0), cycle_token),
        vec![Vec::<Row>::new()]
    );
    let second = run(cyclic.insert(
        "customers",
        fields([
            ("country", Value::String("FR".into())),
            ("parent_id", Value::Reference(first.id.clone())),
        ]),
    ))
    .unwrap();
    assert_eq!(
        rows(&cyclic.drain_notifications(1), cycle_token),
        vec![vec![first, second]]
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn r09_all_relation_navigations_track_source_target_and_pivots() {
    let trace = Arc::new(Trace::default());
    let mut db = run(Database::open(
        Traced {
            inner: fresh(),
            trace: trace.clone(),
        },
        schema(1),
        vec![],
    ))
    .unwrap()
    .into_reactive();
    let c = run(db.insert(
        "customers",
        fields([("country", Value::String("FR".into()))]),
    ))
    .unwrap();
    let d = run(db.insert(
        "customers",
        fields([("country", Value::String("DE".into()))]),
    ))
    .unwrap();
    let mut input = order(1);
    input.insert("customer_id".into(), Value::Reference(c.id.clone()));
    let source = run(db.insert("orders", input)).unwrap();
    let product =
        run(db.insert("products", fields([("name", Value::String("Fern".into()))]))).unwrap();
    let one = db.related("orders", &source.id, "customer");
    let many = db
        .related("customers", &c.id, "orders")
        .order_by("placed_at", Direction::Desc)
        .limit(2);
    let through = db
        .related("orders", &source.id, "products")
        .where_("name", Operator::Eq, Value::String("Fern".into()))
        .limit(1);
    let one_token = run(one.observe(&mut db)).unwrap();
    let many_token = run(many.observe(&mut db)).unwrap();
    let through_token = run(through.observe(&mut db)).unwrap();
    let initial = db.drain_notifications(0);
    assert_eq!(rows(&initial, one_token), vec![vec![c.clone()]]);
    assert_eq!(rows(&initial, many_token), vec![vec![source.clone()]]);
    assert_eq!(rows(&initial, through_token), vec![Vec::<Row>::new()]);
    let source = run(db.replace(
        "orders",
        &source.id,
        changed(&source, "customer_id", Value::Reference(d.id.clone())),
    ))
    .unwrap();
    let events = db.drain_notifications(1);
    assert_eq!(rows(&events, one_token), vec![vec![d.clone()]]);
    assert_eq!(rows(&events, many_token), vec![Vec::<Row>::new()]);
    run(db.link(
        "orders",
        &source.id,
        "products",
        &product.id,
        BTreeMap::new(),
    ))
    .unwrap();
    assert_eq!(
        rows(&db.drain_notifications(2), through_token),
        vec![vec![product.clone()]]
    );
    run(db.insert(
        "order_products",
        fields([
            ("order_id", Value::Reference(source.id.clone())),
            ("product_id", Value::Reference(product.id.clone())),
        ]),
    ))
    .unwrap();
    assert!(db.drain_notifications(3).is_empty());
    assert_eq!(
        run(db.unlink("orders", &source.id, "products", &product.id)).unwrap(),
        2
    );
    assert_eq!(
        rows(&db.drain_notifications(4), through_token),
        vec![Vec::<Row>::new()]
    );
    run(db.remove("orders", &source.id)).unwrap();
    assert_eq!(
        rows(&db.drain_notifications(5), one_token),
        vec![Vec::<Row>::new()]
    );
    assert!(db.unsubscribe(one_token));
    assert!(db.unsubscribe(many_token));
    assert!(db.unsubscribe(through_token));
    trace.clear();
    let count = run(db
        .related("customers", &d.id, "orders")
        .observe_count(&mut db))
    .unwrap();
    assert_eq!(counts(&db.drain_notifications(16), count), vec![0]);
    let mut input = order(9);
    input.insert("customer_id".into(), Value::Reference(d.id));
    run(db.insert("orders", input)).unwrap();
    assert_eq!(counts(&db.drain_notifications(32), count), vec![1]);
    assert_eq!(
        trace.full_queries.load(Ordering::SeqCst),
        0,
        "count navigation cannot read full source/target rows"
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn r10_postcommit_faults_registration_and_explicit_recovery() {
    // TC-0030 / DEF-0004: recovery invalidates value equality, not time.
    let clock_trace = Arc::new(Trace::default());
    let mut clock_db = run(Database::open(
        Traced {
            inner: fresh(),
            trace: clock_trace.clone(),
        },
        schema(1),
        vec![],
    ))
    .unwrap()
    .into_reactive();
    let clock_query = pending(&clock_db);
    let clock_token = run(clock_query.observe_count(&mut clock_db)).unwrap();
    assert_eq!(
        counts(&clock_db.drain_notifications(100), clock_token),
        vec![0]
    );
    clock_trace.fail_project.store(true, Ordering::SeqCst);
    run(clock_db.insert("orders", order(1))).unwrap();
    assert!(clock_db.drain_notifications(101)[0].value.is_err());
    run(clock_db.refresh_notifications());
    assert!(
        clock_db.drain_notifications(102).is_empty(),
        "recovery must retain last delivery deadline"
    );
    assert!(clock_db.drain_notifications(115).is_empty());
    assert_eq!(
        counts(&clock_db.drain_notifications(116), clock_token),
        vec![1]
    );
    // A failed refresh invalidates undelivered count state. Recovery must
    // publish current state even when cardinality equals the last success.
    let trace = Arc::new(Trace::default());
    let mut db = run(Database::open(
        Traced {
            inner: fresh(),
            trace: trace.clone(),
        },
        schema(1),
        vec![],
    ))
    .unwrap()
    .into_reactive();
    let query = pending(&db);
    let token = run(query.observe_count(&mut db)).unwrap();
    assert_eq!(counts(&db.drain_notifications(0), token), vec![0]);
    let first = run(db.insert("orders", order(1))).unwrap();
    trace.fail_project.store(true, Ordering::SeqCst);
    run(db.remove("orders", &first.id)).unwrap();
    let events = db.drain_notifications(16);
    assert_eq!(
        events.len(),
        1,
        "a dirty group cannot publish its stale pending count"
    );
    assert!(events[0].value.is_err());
    assert!(
        run(query.observe_count(&mut db)).is_err(),
        "dirty registration requires explicit recovery"
    );
    assert_eq!(db.observation_stats().groups, 1);
    assert_eq!(db.observation_stats().observers, 1);
    run(db.refresh_notifications());
    assert_eq!(
        counts(&db.drain_notifications(17), token),
        vec![0],
        "recovery republishes current count after error"
    );
    let next = run(query.observe_count(&mut db)).unwrap();
    assert_eq!(db.observation_stats().groups, 1);
    assert_eq!(counts(&db.drain_notifications(18), next), vec![0]);
    for materialization in [false, true] {
        let trace = Arc::new(Trace::default());
        let mut db = run(Database::open(
            Traced {
                inner: fresh(),
                trace: trace.clone(),
            },
            schema(1),
            vec![],
        ))
        .unwrap()
        .into_reactive();
        let query = pending(&db);
        let token = run(query.observe(&mut db)).unwrap();
        db.drain_notifications(0);
        if materialization {
            trace.fail_query.store(true, Ordering::SeqCst);
        } else {
            trace.fail_project.store(true, Ordering::SeqCst);
        }
        let committed = run(db.insert("orders", order(1)))
            .expect("observation failure must not relabel a successful committed write");
        let failures = db.drain_notifications(1);
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].subscription, token);
        assert!(matches!(
            failures[0].value,
            Err(StorageError::BackendFailure { .. })
        ));
        assert_eq!(run(query.fetch(&db)).unwrap(), vec![committed.clone()]);
        run(db.refresh_notifications());
        assert_eq!(
            rows(&db.drain_notifications(2), token),
            vec![vec![committed.clone()]]
        );
        run(db.refresh_notifications());
        assert!(
            db.drain_notifications(3).is_empty(),
            "clean refresh does not duplicate success"
        );
        trace.fail_project.store(true, Ordering::SeqCst);
        let second = run(db.insert("orders", order(2))).unwrap();
        let failed = db.drain_notifications(4);
        assert_eq!(failed.len(), 1);
        assert!(failed[0].value.is_err());
        let third = run(db.insert("orders", order(3))).unwrap();
        assert_eq!(
            rows(&db.drain_notifications(5), token),
            vec![vec![committed, second, third]]
        );
    }
    let trace = Arc::new(Trace::default());
    let mut db = run(Database::open(
        Traced {
            inner: fresh(),
            trace: trace.clone(),
        },
        schema(1),
        vec![],
    ))
    .unwrap()
    .into_reactive();
    let before = db.observation_stats();
    trace.fail_project.store(true, Ordering::SeqCst);
    assert!(matches!(
        run(pending(&db).observe(&mut db)),
        Err(StorageError::BackendFailure { .. })
    ));
    let after = db.observation_stats();
    assert_eq!(after.groups, before.groups);
    assert_eq!(after.observers, before.observers);
    assert!(db.drain_notifications(0).is_empty());
    let good = run(pending(&db).observe(&mut db)).unwrap();
    assert_eq!(
        rows(&db.drain_notifications(1), good),
        vec![Vec::<Row>::new()]
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn r12_noop_link_unlink_and_reopen() {
    let mut db = run(Database::open(fresh(), schema(1), vec![]))
        .unwrap()
        .into_reactive();
    let source = run(db.insert("orders", order(1))).unwrap();
    let target =
        run(db.insert("products", fields([("name", Value::String("Fern".into()))]))).unwrap();
    let query = db.related("orders", &source.id, "products");
    let token = run(query.observe(&mut db)).unwrap();
    db.drain_notifications(0);
    run(db.replace("orders", &source.id, source.fields.clone())).unwrap();
    assert!(db.drain_notifications(1).is_empty());
    assert_eq!(
        run(db.unlink("orders", &source.id, "products", &target.id)).unwrap(),
        0
    );
    assert!(db.drain_notifications(2).is_empty());
    let link = run(db.link(
        "orders",
        &source.id,
        "products",
        &target.id,
        BTreeMap::new(),
    ))
    .unwrap();
    assert_eq!(
        rows(&db.drain_notifications(3), token),
        vec![vec![target.clone()]]
    );
    assert_eq!(
        run(db.link(
            "orders",
            &source.id,
            "products",
            &target.id,
            BTreeMap::new()
        ))
        .unwrap(),
        link
    );
    assert!(db.drain_notifications(4).is_empty());
    let store = db.into_storage();
    let mut reopened = run(Database::open(store, schema(1), vec![]))
        .unwrap()
        .into_reactive();
    assert!(reopened.drain_notifications(100).is_empty());
    assert_eq!(reopened.observation_stats().observers, 0);
    assert_eq!(run(query.fetch(&reopened)).unwrap(), vec![target.clone()]);
    let next = run(query.observe(&mut reopened)).unwrap();
    assert!(!reopened.unsubscribe(token));
    assert_eq!(
        rows(&reopened.drain_notifications(101), next),
        vec![vec![target]]
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn r15_count_work_scales_with_affected_ids_and_shared_observers() {
    for unrelated in [8, 64] {
        let trace = Arc::new(Trace::default());
        let mut db = run(Database::open(
            Traced {
                inner: fresh(),
                trace: trace.clone(),
            },
            schema(1),
            vec![],
        ))
        .unwrap()
        .into_reactive();
        for _ in 0..unrelated {
            let mut input = order(0);
            input.insert("status".into(), Value::String("shipped".into()));
            run(db.insert("orders", input)).unwrap();
        }
        let a = run(db.insert("orders", order(1))).unwrap();
        let query = pending(&db);
        let first = run(query.observe_count(&mut db)).unwrap();
        let second = run(query.observe_count(&mut db)).unwrap();
        db.drain_notifications(0);
        assert_eq!(db.observation_stats().groups, 1);
        trace.clear();
        let before = db.observation_stats();
        run(db.replace(
            "orders",
            &a.id,
            changed(&a, "status", Value::String("shipped".into())),
        ))
        .unwrap();
        let after = db.observation_stats();
        assert!(
            after.candidate_updates - before.candidate_updates <= 1,
            "candidate work must be independent of unrelated population"
        );
        assert_eq!(trace.full_queries.load(Ordering::SeqCst), 0);
        assert_eq!(trace.scans.load(Ordering::SeqCst), 0);
        for (requested, _) in trace.projects.lock().unwrap().iter() {
            assert!(
                requested
                    .predicates
                    .iter()
                    .any(|p| p.field == "id" && p.values == vec![Value::String(a.id.clone())]),
                "ordinary change must query only its affected ID: {requested:?}"
            );
        }
        let events = db.drain_notifications(16);
        assert_eq!(counts(&events, first), vec![0]);
        assert_eq!(counts(&events, second), vec![0]);
        assert_eq!(after.materialized_rows, 0);
        assert!(db.unsubscribe(first));
        assert!(db.unsubscribe(second));
        trace.clear();
        run(db.insert("orders", order(3))).unwrap();
        assert!(trace.projects.lock().unwrap().is_empty());
    }
}

// SPEC-0028 / TC-0031 Q01-Q03: permanent Claude review reproductions.
#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn review_q01_large_result_sets_and_navigation() {
    let mut db = run(Database::open(fresh(), schema(1), vec![]))
        .unwrap()
        .into_reactive();
    run(db.apply_batch(
        (0..996)
            .map(|n| Write::Insert {
                table: "orders".into(),
                fields: order(n),
            })
            .collect(),
    ))
    .unwrap();
    let q = pending(&db).order_by("total_cents", Direction::Desc);
    let token = run(q.observe(&mut db)).unwrap();
    let count = run(q.observe_count(&mut db)).unwrap();
    assert_eq!(rows(&db.drain_notifications(0), token)[0].len(), 996);
    run(db.apply_batch(
        (996..1105)
            .map(|n| Write::Insert {
                table: "orders".into(),
                fields: order(n),
            })
            .collect(),
    ))
    .unwrap();
    let expected = run(q.fetch(&db)).unwrap();
    let events = db.drain_notifications(16);
    assert_eq!(rows(&events, token), vec![expected.clone()]);
    assert_eq!(counts(&events, count), vec![1105]);
    let other = run(q.observe(&mut db)).unwrap();
    assert_eq!(
        rows(&db.drain_notifications(17), other),
        vec![expected.clone()]
    );
    let ids = expected
        .iter()
        .map(|r| Value::String(r.id.clone()))
        .collect();
    assert_eq!(
        run(db
            .query("orders")
            .where_in("id", ids)
            .order_by("total_cents", Direction::Desc)
            .fetch(&db))
        .unwrap(),
        expected
    );
    assert!(db.unsubscribe(token));
    assert!(db.unsubscribe(count));
    assert!(db.unsubscribe(other));
    // One target per source forces both the reverse dependency query and the
    // affected-ID query past the old expression-depth ceiling in one commit.
    let changes = run(db.apply_batch(
        (0..1105)
            .map(|_| Write::Insert {
                table: "customers".into(),
                fields: fields([("country", Value::String("FR".into()))]),
            })
            .collect(),
    ))
    .unwrap();
    let customers: Vec<_> = changes.into_iter().map(|c| c.after.unwrap()).collect();
    run(db.apply_batch(
        expected
            .iter()
            .zip(&customers)
            .map(|(row, customer)| Write::Replace {
                table: "orders".into(),
                id: row.id.clone(),
                fields: changed(row, "customer_id", Value::Reference(customer.id.clone())),
            })
            .collect(),
    ))
    .unwrap();
    let related =
        db.query("orders")
            .where_("customer.country", Operator::Eq, Value::String("FR".into()));
    let count = run(related.observe_count(&mut db)).unwrap();
    assert_eq!(counts(&db.drain_notifications(32), count), vec![1105]);
    run(db.apply_batch(
        customers
            .iter()
            .map(|row| Write::Replace {
                table: "customers".into(),
                id: row.id.clone(),
                fields: fields([("country", Value::String("DE".into()))]),
            })
            .collect(),
    ))
    .unwrap();
    assert_eq!(counts(&db.drain_notifications(48), count), vec![0]);
    db.unsubscribe(count);
    let source = &expected[0];
    let products = run(db.apply_batch(
        (0..1105)
            .map(|_| Write::Insert {
                table: "products".into(),
                fields: fields([("name", Value::String("Fern".into()))]),
            })
            .collect(),
    ))
    .unwrap();
    run(db.apply_batch(
        products
            .iter()
            .map(|c| Write::Insert {
                table: "order_products".into(),
                fields: fields([
                    ("order_id", Value::Reference(source.id.clone())),
                    ("product_id", Value::Reference(c.id.clone())),
                ]),
            })
            .collect(),
    ))
    .unwrap();
    let nav = db.related("orders", &source.id, "products");
    let fetched = run(nav.fetch(&db)).unwrap();
    assert_eq!(fetched.len(), 1105);
    let token = run(nav.observe(&mut db)).unwrap();
    assert_eq!(rows(&db.drain_notifications(64), token), vec![fetched]);
    // Exercise navigation membership plus affected-id intersection. The
    // separate owner TEMP case crosses the oversized execution threshold.
    run(db.remove("products", &products[0].id)).unwrap();
    let after = run(nav.fetch(&db)).unwrap();
    assert_eq!(after.len(), 1104);
    assert_eq!(rows(&db.drain_notifications(80), token), vec![after]);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn review_q01_owner_temp_window_intersection() {
    let definition = SchemaDefinition {
        version: 1,
        tables: BTreeMap::from([(
            "items".into(),
            TableDefinition {
                columns: BTreeMap::from([(
                    "n".into(),
                    Column {
                        kind: ColumnType::Integer,
                        optional: false,
                    },
                )]),
                relations: BTreeMap::new(),
            },
        )]),
    };
    let mut next = 0;
    let mut db = run(Database::open(fresh(), definition, vec![]))
        .unwrap()
        .with_id_generator(move || {
            next += 1;
            format!("seed:{next}")
        })
        .into_reactive();
    let input: Vec<_> = (0..4105)
        .map(|n| Write::Insert {
            table: "items".into(),
            fields: fields([("n", Value::Integer(n))]),
        })
        .collect();
    let committed = run(db.apply_batch(input)).unwrap();
    let ids = committed
        .iter()
        .map(|c| Value::String(c.id.clone()))
        .collect();
    let q = db
        .query("items")
        .where_in("id", ids)
        .order_by("n", Direction::Desc);
    let row = run(q.observe_with_columns(&mut db, vec!["n".into()])).unwrap();
    let count = run(q.observe_count(&mut db)).unwrap();
    let initial = db.drain_notifications(0);
    assert_eq!(rows(&initial, row)[0].len(), 4105);
    assert_eq!(counts(&initial, count), vec![4105]);
    let id = &committed[0].id;
    run(db.replace("items", id, fields([("n", Value::Integer(99999))]))).unwrap();
    let expected = run(q.fetch(&db)).unwrap();
    assert_eq!(expected.len(), 4105);
    assert_eq!(&expected[0].id, id);
    let events = db.drain_notifications(16);
    assert_eq!(rows(&events, row), vec![expected]);
    assert!(counts(&events, count).is_empty());
    assert_eq!(db.observation_stats().groups, 1);
    assert!(db.unsubscribe(row));
    assert!(db.unsubscribe(count));
    assert_eq!(db.observation_stats().groups, 0);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn review_q02_explicit_retry_errors_are_observable() {
    let trace = Arc::new(Trace::default());
    let mut db = run(Database::open(
        Traced {
            inner: fresh(),
            trace: trace.clone(),
        },
        schema(1),
        vec![],
    ))
    .unwrap()
    .into_reactive();
    let q = pending(&db);
    let token = run(q.observe_count(&mut db)).unwrap();
    assert_eq!(counts(&db.drain_notifications(100), token), vec![0]);
    trace.fail_project.store(true, Ordering::SeqCst);
    run(db.insert("orders", order(1))).unwrap();
    assert!(db.drain_notifications(101)[0].value.is_err());
    for now in [102, 103] {
        trace.fail_project.store(true, Ordering::SeqCst);
        run(db.refresh_notifications());
        let events = db.drain_notifications(now);
        assert_eq!(
            events.len(),
            1,
            "each failed explicit retry reports failure"
        );
        assert!(events[0].value.is_err());
    }
    for now in [104, 105] {
        trace.fail_project.store(true, Ordering::SeqCst);
        run(db.insert("orders", order(2))).unwrap();
        assert!(
            db.drain_notifications(now).is_empty(),
            "ordinary writes must not flood duplicate errors"
        );
    }
    run(db.refresh_notifications());
    assert!(db.drain_notifications(115).is_empty());
    assert_eq!(counts(&db.drain_notifications(116), token), vec![3]);
    assert_eq!(db.observation_stats().materialized_rows, 0);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn review_q03_row_failure_keeps_count_live() {
    let trace = Arc::new(Trace::default());
    let mut db = run(Database::open(
        Traced {
            inner: fresh(),
            trace: trace.clone(),
        },
        schema(1),
        vec![],
    ))
    .unwrap()
    .into_reactive();
    let q = pending(&db);
    let row = run(q.observe(&mut db)).unwrap();
    let finer = run(q.observe_with_columns(&mut db, vec!["notes".into()])).unwrap();
    let count = run(q.observe_count(&mut db)).unwrap();
    db.drain_notifications(0);
    trace.fail_query.store(true, Ordering::SeqCst);
    let first = run(db.insert("orders", order(1))).unwrap();
    let events = db.drain_notifications(16);
    assert_eq!(
        counts(&events, count),
        vec![1],
        "a valid projected count cannot fail because a row listener failed"
    );
    assert_eq!(events.iter().filter(|e| e.value.is_err()).count(), 2);
    let another = run(q.observe_count(&mut db)).unwrap();
    assert_eq!(db.observation_stats().groups, 1);
    assert_eq!(counts(&db.drain_notifications(17), another), vec![1]);
    let observers = db.observation_stats().observers;
    trace.fail_query.store(true, Ordering::SeqCst);
    assert!(run(q.observe(&mut db)).is_err());
    assert_eq!(db.observation_stats().observers, observers);
    for now in [18, 19] {
        trace.fail_query.store(true, Ordering::SeqCst);
        run(db.refresh_notifications());
        let events = db.drain_notifications(now);
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| e.value.is_err()));
    }
    trace.fail_query.store(true, Ordering::SeqCst);
    let second = run(db.insert("orders", order(2))).unwrap();
    assert!(db.drain_notifications(20).is_empty());
    assert_eq!(counts(&db.drain_notifications(32), count), vec![2]);
    run(db.refresh_notifications());
    let events = db.drain_notifications(33);
    assert_eq!(
        rows(&events, row),
        vec![vec![first.clone(), second.clone()]]
    );
    assert_eq!(rows(&events, finer), vec![vec![first, second]]);
    assert!(counts(&events, count).is_empty());
    trace.fail_query.store(true, Ordering::SeqCst);
    run(db.insert("orders", order(3))).unwrap();
    db.drain_notifications(34);
    for token in [row, finer, count, another] {
        assert!(db.unsubscribe(token));
    }
    assert_eq!(db.observation_stats().groups, 0);
    run(db.refresh_notifications());
    assert!(db.drain_notifications(50).is_empty());
}

#[derive(Default)]
struct Trace {
    fail_project: AtomicBool,
    fail_query: AtomicBool,
    projects: Mutex<Vec<(Query, Vec<String>)>>,
    full_queries: AtomicUsize,
    scans: AtomicUsize,
}

impl Trace {
    fn clear(&self) {
        self.projects.lock().unwrap().clear();
        self.full_queries.store(0, Ordering::SeqCst);
        self.scans.store(0, Ordering::SeqCst);
    }
}

struct Traced<S> {
    inner: S,
    trace: Arc<Trace>,
}

impl<S: Storage> Storage for Traced<S> {
    fn create_table(
        &mut self,
        name: &str,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(run(self.inner.create_table(name)))
    }

    fn insert(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(run(self.inner.insert(table, row)))
    }

    fn get(
        &self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<Option<Row>, StorageError>> + Send {
        ready(run(self.inner.get(table, id)))
    }

    fn replace(
        &mut self,
        table: &str,
        row: Row,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(run(self.inner.replace(table, row)))
    }

    fn remove(
        &mut self,
        table: &str,
        id: &str,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send {
        ready(run(self.inner.remove(table, id)))
    }
}

impl<S: TransactionalStorage> TransactionalStorage for Traced<S> {
    fn apply_batch(
        &mut self,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(run(self.inner.apply_batch(mutations)))
    }
}

impl<S: TypedStorage> TypedStorage for Traced<S> {
    fn query(
        &self,
        schema: &Schema,
        query: &Query,
    ) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.trace.full_queries.fetch_add(1, Ordering::SeqCst);
        if self.trace.fail_query.swap(false, Ordering::SeqCst) {
            ready(Err(StorageError::BackendFailure {
                message: "fixture post-commit row query failure".into(),
            }))
        } else {
            ready(run(self.inner.query(schema, query)))
        }
    }
}

impl<S: TypedTransactionalStorage> TypedTransactionalStorage for Traced<S> {
    fn apply_typed_batch(
        &mut self,
        schema: &Schema,
        mutations: Vec<Mutation>,
    ) -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send {
        ready(run(self.inner.apply_typed_batch(schema, mutations)))
    }
}

impl<S: SchemaStorage> SchemaStorage for Traced<S> {
    fn catalog(&self) -> impl Future<Output = Result<ModelCatalog, StorageError>> + Send {
        ready(run(self.inner.catalog()))
    }

    fn scan(&self, table: &str) -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send {
        self.trace.scans.fetch_add(1, Ordering::SeqCst);
        ready(run(self.inner.scan(table)))
    }

    fn apply_schema_plan(
        &mut self,
        plan: SchemaPlan,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        ready(run(self.inner.apply_schema_plan(plan)))
    }
}

impl<S: ObservationStorage> ObservationStorage for Traced<S> {
    fn project(
        &self,
        schema: &Schema,
        query: &Query,
        fields: &[String],
    ) -> impl Future<Output = Result<Vec<ProjectedRow>, StorageError>> + Send {
        self.trace
            .projects
            .lock()
            .unwrap()
            .push((query.clone(), fields.to_vec()));
        if self.trace.fail_project.swap(false, Ordering::SeqCst) {
            ready(Err(StorageError::BackendFailure {
                message: "fixture post-commit projected read failure".into(),
            }))
        } else {
            ready(run(self.inner.project(schema, query, fields)))
        }
    }
}
