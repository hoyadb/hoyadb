//! TC-0029 R11: identical narrow projections on memory/native/browser engines.
use super::fresh;
use hoyadb_core::storage::*;
use std::{
    collections::BTreeMap,
    future::Future,
    task::{Context, Poll, Waker},
};

fn run<F: Future + Send>(future: F) -> F::Output {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("projection must return a ready Send future"),
    }
}

fn schema(kind: ColumnType) -> Schema {
    let column = |kind, optional| Column {
        kind,
        optional,
    };
    Schema {
        tables: BTreeMap::from([
            (
                "items".into(),
                TableSchema {
                    columns: BTreeMap::from([
                        ("value".into(), column(kind.clone(), true)),
                        ("noise".into(), column(ColumnType::String, true)),
                        ("customer_id".into(), column(ColumnType::Reference, true)),
                    ]),
                    relations: BTreeMap::from([(
                        "customer".into(),
                        Relation::ToOne {
                            table: "customers".into(),
                            foreign_key: "customer_id".into(),
                        },
                    )]),
                },
            ),
            (
                "customers".into(),
                TableSchema {
                    columns: BTreeMap::from([
                        ("value".into(), column(kind.clone(), true)),
                        ("required".into(), column(kind, false)),
                    ]),
                    relations: BTreeMap::new(),
                },
            ),
        ]),
    }
}

fn row(id: &str, fields: impl IntoIterator<Item = (&'static str, Value)>) -> Row {
    Row {
        id: id.into(),
        fields: fields.into_iter().map(|(k, v)| (k.into(), v)).collect(),
    }
}

fn query() -> Query {
    Query {
        table: "items".into(),
        predicates: vec![],
        order: None,
        limit: None,
    }
}

fn projected(id: &str, fields: impl IntoIterator<Item = (&'static str, Value)>) -> ProjectedRow {
    ProjectedRow {
        order_key: None,
        id: id.into(),
        values: fields.into_iter().map(|(k, v)| (k.into(), v)).collect(),
    }
}

fn paths(fields: &[&str]) -> Vec<String> {
    fields.iter().map(|v| (*v).into()).collect()
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn r11_all_values_owned_and_only_requested_cells() {
    for (kind, values) in [
        (
            ColumnType::String,
            vec![
                Value::String("\0'--".into()),
                Value::String("A".into()),
                Value::String("é🌿".into()),
            ],
        ),
        (
            ColumnType::Integer,
            vec![
                Value::Integer(i64::MIN),
                Value::Integer(0),
                Value::Integer(i64::MAX),
            ],
        ),
        (
            ColumnType::Float,
            vec![
                Value::Float(FiniteFloat::new(-f64::MAX).unwrap()),
                Value::Float(FiniteFloat::new(f64::from_bits(1)).unwrap()),
                Value::Float(FiniteFloat::new(f64::MAX).unwrap()),
            ],
        ),
        (
            ColumnType::Boolean,
            vec![Value::Boolean(false), Value::Boolean(true)],
        ),
        (
            ColumnType::Timestamp,
            vec![
                Value::Timestamp(i64::MIN),
                Value::Timestamp(0),
                Value::Timestamp(i64::MAX),
            ],
        ),
        (
            ColumnType::Reference,
            vec![
                Value::Reference("\0'--".into()),
                Value::Reference("A".into()),
                Value::Reference("é🌿".into()),
            ],
        ),
    ] {
        let declared = schema(kind);
        let mut engine = fresh();
        run(engine.create_table("items")).unwrap();
        let mut expected = vec![];
        for (i, value) in values.iter().enumerate() {
            let id = format!("{i}");
            run(engine.insert(
                "items",
                row(
                    &id,
                    [
                        ("value", value.clone()),
                        ("noise", Value::String("unrequested".into())),
                    ],
                ),
            ))
            .unwrap();
            expected.push(projected(
                &id,
                [("id", Value::String(id.clone())), ("value", value.clone())],
            ));
        }
        let fields = paths(&["id", "value", "value"]);
        let mut result = run(engine.project(&declared, &query(), &fields)).unwrap();
        assert_eq!(result, expected);
        result[0].values.clear();
        assert_eq!(
            run(engine.project(&declared, &query(), &fields)).unwrap(),
            expected
        );
        let ids = run(engine.project(&declared, &query(), &[])).unwrap();
        assert!(ids.iter().all(|p| p.values.is_empty()));
        let mut selected = query();
        selected.predicates.push(Predicate {
            field: "value".into(),
            operator: Operator::Eq,
            values: vec![values[0].clone()],
        });
        assert_eq!(
            run(engine.project(&declared, &selected, &fields)).unwrap(),
            vec![expected[0].clone()]
        );
        selected.predicates.clear();
        selected.order = Some(OrderBy {
            field: "value".into(),
            direction: Direction::Desc,
        });
        selected.limit = Some(1);
        let mut last = expected.last().unwrap().clone();
        last.order_key = Some(values.last().unwrap().clone());
        assert_eq!(
            run(engine.project(&declared, &selected, &fields)).unwrap(),
            vec![last]
        );
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn r11_filter_order_limit_and_nulls() {
    let declared = schema(ColumnType::Integer);
    let mut engine = fresh();
    run(engine.create_table("items")).unwrap();
    for r in [
        row("a", []),
        row("b", [("value", Value::Null)]),
        row("c", [("value", Value::Integer(1))]),
        row("d", [("value", Value::Integer(1))]),
        row("e", [("value", Value::Integer(2))]),
    ] {
        run(engine.insert("items", r)).unwrap();
    }
    for (operator, values, expected) in [
        (Operator::Eq, vec![Value::Integer(1)], vec!["c", "d"]),
        (Operator::Ne, vec![Value::Integer(1)], vec!["a", "b", "e"]),
        (Operator::Lt, vec![Value::Integer(2)], vec!["c", "d"]),
        (Operator::Le, vec![Value::Integer(1)], vec!["c", "d"]),
        (Operator::Gt, vec![Value::Integer(1)], vec!["e"]),
        (Operator::Ge, vec![Value::Integer(2)], vec!["e"]),
        (
            Operator::In,
            vec![Value::Null, Value::Integer(2), Value::Integer(2)],
            vec!["a", "b", "e"],
        ),
        (Operator::In, vec![], vec![]),
    ] {
        let mut q = query();
        q.predicates.push(Predicate {
            field: "value".into(),
            operator,
            values,
        });
        let found = run(engine.project(&declared, &q, &paths(&["value"]))).unwrap();
        assert_eq!(
            found.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            expected
        );
    }
    let all = run(engine.project(&declared, &query(), &paths(&["value"]))).unwrap();
    assert_eq!(all[0].values.get("value"), Some(&Value::Null));
    assert_eq!(all[1].values.get("value"), Some(&Value::Null));
    let mut q = query();
    q.order = Some(OrderBy {
        field: "value".into(),
        direction: Direction::Desc,
    });
    q.limit = Some(3);
    let found = run(engine.project(&declared, &q, &[])).unwrap();
    assert_eq!(
        found.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
        vec!["e", "c", "d"]
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn r11_direct_relations_missing_targets_and_cells() {
    let declared = schema(ColumnType::Integer);
    let mut engine = fresh();
    for t in ["items", "customers"] {
        run(engine.create_table(t)).unwrap();
    }
    for r in [
        row("a", [("customer_id", Value::Reference("present".into()))]),
        row("b", [("customer_id", Value::Reference("absent".into()))]),
        row("c", []),
        row("d", [("customer_id", Value::Null)]),
    ] {
        run(engine.insert("items", r)).unwrap();
    }
    run(engine.insert(
        "customers",
        row(
            "present",
            [
                ("value", Value::Integer(9)),
                ("required", Value::Integer(3)),
            ],
        ),
    ))
    .unwrap();
    let fields = paths(&["customer.id", "customer.value", "customer.required"]);
    assert_eq!(
        run(engine.project(&declared, &query(), &fields)).unwrap(),
        vec![
            projected(
                "a",
                [
                    ("customer.id", Value::String("present".into())),
                    ("customer.value", Value::Integer(9)),
                    ("customer.required", Value::Integer(3))
                ]
            ),
            projected(
                "b",
                [
                    ("customer.id", Value::Null),
                    ("customer.value", Value::Null),
                    ("customer.required", Value::Null)
                ]
            ),
            projected(
                "c",
                [
                    ("customer.id", Value::Null),
                    ("customer.value", Value::Null),
                    ("customer.required", Value::Null)
                ]
            ),
            projected(
                "d",
                [
                    ("customer.id", Value::Null),
                    ("customer.value", Value::Null),
                    ("customer.required", Value::Null)
                ]
            ),
        ]
    );
    let mut q = query();
    q.predicates.push(Predicate {
        field: "customer.value".into(),
        operator: Operator::Ne,
        values: vec![Value::Integer(0)],
    });
    assert_eq!(
        run(engine.project(&declared, &q, &[])).unwrap(),
        vec![projected("a", [])]
    );
    run(engine.replace(
        "customers",
        row(
            "present",
            [("value", Value::Null), ("required", Value::Integer(3))],
        ),
    ))
    .unwrap();
    let result = run(engine.project(&declared, &query(), &fields)).unwrap();
    assert_eq!(result[0].values.get("customer.value"), Some(&Value::Null));
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn r11_validation_before_zero_limit_and_unrequested_rows_stay_unchecked() {
    let declared = schema(ColumnType::Integer);
    let mut engine = fresh();
    run(engine.create_table("items")).unwrap();
    let mut q = query();
    q.limit = Some(0);
    for fields in [
        paths(&["unknown"]),
        paths(&["customer.unknown"]),
        paths(&["customer.value.bad"]),
    ] {
        assert!(matches!(
            run(engine.project(&declared, &q, &fields)),
            Err(StorageError::InvalidQuery { .. })
        ));
    }
    assert_eq!(
        run(engine.project(&declared, &q, &paths(&["customer.value"]))),
        Err(StorageError::TableNotFound {
            table: "customers".into()
        })
    );
    q.predicates.push(Predicate {
        field: "value".into(),
        operator: Operator::Eq,
        values: vec![Value::String("wrong".into())],
    });
    assert!(matches!(
        run(engine.project(&declared, &q, &[])),
        Err(StorageError::InvalidQuery { .. })
    ));
    run(engine.insert(
        "items",
        row(
            "bad",
            [
                ("value", Value::String("wrong".into())),
                ("noise", Value::Integer(1)),
            ],
        ),
    ))
    .unwrap();
    assert_eq!(
        run(engine.project(&declared, &query(), &[])),
        Ok(vec![projected("bad", [])])
    );
    assert!(
        matches!(run(engine.project(&declared,&query(),&paths(&["value"]))),Err(StorageError::InvalidRow{table,id,..}) if table=="items"&&id=="bad")
    );
    q = query();
    q.predicates.push(Predicate {
        field: "id".into(),
        operator: Operator::Eq,
        values: vec![Value::String("not-present".into())],
    });
    assert_eq!(
        run(engine.project(&declared, &q, &paths(&["value"]))),
        Ok(vec![])
    );
    let mut ordered = query();
    ordered.order = Some(OrderBy {
        field: "value".into(),
        direction: Direction::Asc,
    });
    assert_eq!(
        run(engine.project(&declared, &ordered, &[])).unwrap()[0].order_key,
        Some(Value::Null)
    );
    assert!(matches!(
        run(engine.project(&declared, &ordered, &paths(&["value"]))),
        Err(StorageError::InvalidRow { .. })
    ));
    q = query();
    q.limit = Some(0);
    assert_eq!(
        run(engine.project(&declared, &q, &paths(&["value"]))),
        Ok(vec![])
    );
    q = query();
    q.predicates.push(Predicate {
        field: "value".into(),
        operator: Operator::Ne,
        values: vec![Value::Integer(0)],
    });
    assert_eq!(run(engine.project(&declared, &q, &[])), Ok(vec![]));
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn r11_bound_names_and_required_cells() {
    let mut declared = schema(ColumnType::String);
    let table = "表\0'--";
    let field = "名\0'; SELECT ?";
    let mut definition = declared.tables.remove("items").unwrap();
    definition.columns.insert(
        field.into(),
        Column {
            kind: ColumnType::String,
            optional: false,
        },
    );
    declared.tables.insert(table.into(), definition);
    let mut engine = fresh();
    run(engine.create_table(table)).unwrap();
    let mut r = row("\0é'", [("value", Value::Null)]);
    r.fields
        .insert(field.into(), Value::String("bound\0🌿".into()));
    run(engine.insert(table, r)).unwrap();
    let mut q = query();
    q.table = table.into();
    q.predicates.push(Predicate {
        field: field.into(),
        operator: Operator::Eq,
        values: vec![Value::String("bound\0🌿".into())],
    });
    assert_eq!(
        run(engine.project(&declared, &q, &[field.into()])).unwrap(),
        vec![ProjectedRow {
            order_key: None,
            id: "\0é'".into(),
            values: BTreeMap::from([(field.into(), Value::String("bound\0🌿".into()))])
        }]
    );
    run(engine.replace(table, row("\0é'", []))).unwrap();
    q.predicates.clear();
    assert!(matches!(
        run(engine.project(&declared, &q, &[field.into()])),
        Err(StorageError::InvalidRow { .. })
    ));
}

// TC-0031 Q01: identical selection below/above the temporary-operand threshold.
#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn review_q01_oversized_operands_preserve_all_types() {
    for (kind, values) in [
        (
            ColumnType::String,
            vec![Value::String("bound\0🌿'--".into())],
        ),
        (
            ColumnType::Integer,
            vec![Value::Integer(i64::MIN), Value::Integer(i64::MAX)],
        ),
        (
            ColumnType::Timestamp,
            vec![Value::Timestamp(i64::MIN), Value::Timestamp(i64::MAX)],
        ),
        (
            ColumnType::Boolean,
            vec![Value::Boolean(true), Value::Boolean(false)],
        ),
        (
            ColumnType::Reference,
            vec![Value::Reference("bound\0🌿'--".into())],
        ),
        (
            ColumnType::Float,
            vec![
                Value::Float(FiniteFloat::new(42.0).unwrap()),
                Value::Float(FiniteFloat::new(1.25).unwrap()),
                Value::Float(FiniteFloat::new(-f64::MAX).unwrap()),
            ],
        ),
    ] {
        let declared = schema(kind);
        let mut engine = fresh();
        run(engine.create_table("items")).unwrap();
        for (i, value) in values.iter().enumerate() {
            run(engine.insert("items", row(&i.to_string(), [("value", value.clone())]))).unwrap();
        }
        run(engine.insert("items", row("null", [("value", Value::Null)]))).unwrap();
        let mut q = query();
        q.order = Some(OrderBy {
            field: "value".into(),
            direction: Direction::Desc,
        });
        q.limit = Some(2);
        let mut small = values.clone();
        small.push(Value::Null);
        q.predicates.push(Predicate {
            field: "value".into(),
            operator: Operator::In,
            values: small.clone(),
        });
        let expected = run(engine.project(&declared, &q, &paths(&["value"]))).unwrap();
        let mut big = Vec::with_capacity(33_005);
        for i in 0..33_005 {
            big.push(small[i % small.len()].clone());
        }
        q.predicates[0].values = big;
        // A second In keeps AND and bound id semantics in the same statement.
        q.predicates.push(Predicate {
            field: "id".into(),
            operator: Operator::In,
            values: vec![
                Value::String("0".into()),
                Value::String("1".into()),
                Value::String("2".into()),
                Value::String("null".into()),
            ],
        });
        assert_eq!(
            run(engine.project(&declared, &q, &paths(&["value"]))).unwrap(),
            expected
        );
        let rows = run(engine.query(&declared, &q)).unwrap();
        assert_eq!(
            rows.iter().map(|r| &r.id).collect::<Vec<_>>(),
            expected.iter().map(|r| &r.id).collect::<Vec<_>>()
        );
        assert_eq!(
            run(engine.project(&declared, &query(), &[])).unwrap().len(),
            values.len() + 1
        );
    }
}
