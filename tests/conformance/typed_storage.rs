//! TC-0025 P01-P10 / API-0003: identical assertions in memory/native/browser.
use super::fresh;
use hoyadb_core::storage::*;
use std::collections::BTreeMap;
use std::future::Future;
use std::task::{Context, Poll, Waker};

pub fn run<F: Future + Send>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("storage operation must return a ready Send future"),
    }
}

pub fn column(kind: ColumnType, optional: bool) -> Column {
    Column {
        kind,
        optional,
    }
}

pub fn schema(kind: ColumnType, optional: bool) -> Schema {
    Schema {
        tables: BTreeMap::from([(
            "items".into(),
            TableSchema {
                columns: BTreeMap::from([("value".into(), column(kind, optional))]),
                relations: BTreeMap::new(),
            },
        )]),
    }
}

pub fn row(id: &str, value: Value) -> Row {
    Row {
        id: id.into(),
        fields: BTreeMap::from([("value".into(), value)]),
    }
}

pub fn query() -> Query {
    Query {
        table: "items".into(),
        predicates: Vec::new(),
        order: None,
        limit: None,
    }
}

pub fn predicate(field: &str, operator: Operator, values: Vec<Value>) -> Predicate {
    Predicate {
        field: field.into(),
        operator,
        values,
    }
}

pub fn insert(table: &str, row: Row) -> Mutation {
    Mutation::Insert {
        table: table.into(),
        row,
    }
}

pub fn replace(table: &str, row: Row) -> Mutation {
    Mutation::Replace {
        table: table.into(),
        row,
    }
}

pub fn remove(table: &str, id: &str) -> Mutation {
    Mutation::Remove {
        table: table.into(),
        id: id.into(),
    }
}

pub fn float(value: f64) -> Value {
    Value::Float(FiniteFloat::new(value).expect("finite fixture float"))
}

fn selected(rows: &[Row], indices: &[usize]) -> Vec<Row> {
    indices.iter().map(|&i| rows[i].clone()).collect()
}

fn invalid_row(error: StorageError, table: &str, id: &str) {
    match error {
        StorageError::InvalidRow {
            table: actual_table,
            id: actual_id,
            ..
        } => {
            assert_eq!(actual_table, table);
            assert_eq!(actual_id, id);
        }
        other => panic!("expected InvalidRow for {table}/{id}, got {other:?}"),
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn p01_all_primitives_roundtrip() {
    assert!(FiniteFloat::new(f64::NAN).is_none());
    assert!(FiniteFloat::new(f64::INFINITY).is_none());
    assert!(FiniteFloat::new(f64::NEG_INFINITY).is_none());
    assert_eq!(FiniteFloat::new(-0.0).unwrap().get().to_bits(), 0);
    assert_eq!(FiniteFloat::new(-0.0), FiniteFloat::new(0.0));
    let values = [
        (ColumnType::String, vec![Value::String("é\0' 🌿".into())]),
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
                float(f64::from_bits(1)),
                float(-0.0),
                float(-1.5),
                float(1.0),
                float(1.5),
                float(f64::MAX),
                float(-f64::MAX),
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
                Value::Timestamp(-1),
                Value::Timestamp(0),
                Value::Timestamp(i64::MAX),
            ],
        ),
        (
            ColumnType::Reference,
            vec![Value::Reference("missing\0' 🌿".into())],
        ),
    ];
    for (kind, values) in values {
        let mut engine = fresh();
        run(engine.create_table("items")).unwrap();
        let declared = schema(kind, true);
        let mut expected: Vec<Row> = values
            .into_iter()
            .enumerate()
            .map(|(i, v)| row(&format!("{i:02}"), v))
            .collect();
        expected.push(row("null", Value::Null));
        expected.push(Row {
            id: "omitted".into(),
            fields: BTreeMap::new(),
        });
        let mut changes = run(engine.apply_typed_batch(
            &declared,
            expected
                .iter()
                .cloned()
                .map(|r| insert("items", r))
                .collect(),
        ))
        .unwrap();
        assert_eq!(changes.len(), expected.len());
        for (change, wanted) in changes.iter().zip(&expected) {
            assert_eq!(change.before, None);
            assert_eq!(change.after.as_ref(), Some(wanted));
        }
        let mut result = run(engine.query(&declared, &query())).unwrap();
        assert_eq!(result, expected);
        result[0].fields.clear();
        changes[0].after.as_mut().unwrap().fields.clear();
        assert_eq!(
            run(engine.get("items", &expected[0].id)).unwrap(),
            Some(expected[0].clone())
        );
        let changed = row(&expected[0].id, Value::Null);
        run(engine.apply_typed_batch(&declared, vec![replace("items", changed.clone())])).unwrap();
        expected[0] = changed;
        assert_eq!(run(engine.query(&declared, &query())).unwrap(), expected);
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn p02_schema_and_row_validation() {
    let mut engine = fresh();
    run(engine.create_table("items")).unwrap();
    let declared = schema(ColumnType::Integer, false);
    let original = row("a", Value::Integer(1));
    run(engine.insert("items", original.clone())).unwrap();
    let mut invalid_schemas = Vec::new();
    for name in ["", "id", "bad.path"] {
        let mut bad = declared.clone();
        bad.tables
            .get_mut("items")
            .unwrap()
            .columns
            .insert(name.into(), column(ColumnType::String, true));
        invalid_schemas.push(bad);
    }
    for (name, target, key) in [
        ("value", "items", "value"),
        ("customer", "missing", "value"),
        ("customer", "items", "missing"),
        ("customer", "items", "value"),
        ("bad.path", "items", "value"),
        ("id", "items", "value"),
        ("", "items", "value"),
    ] {
        let mut bad = declared.clone();
        bad.tables.get_mut("items").unwrap().relations.insert(
            name.into(),
            Relation::ToOne {
                table: target.into(),
                foreign_key: key.into(),
            },
        );
        invalid_schemas.push(bad);
    }
    for bad in invalid_schemas {
        let mut empty_query = query();
        empty_query.limit = Some(0);
        assert!(matches!(
            run(engine.query(&bad, &empty_query)),
            Err(StorageError::InvalidSchema { .. })
        ));
        assert!(matches!(
            run(engine.apply_typed_batch(&bad, vec![])),
            Err(StorageError::InvalidSchema { .. })
        ));
        assert_eq!(
            run(engine.get("items", "a")).unwrap(),
            Some(original.clone())
        );
    }
    let mut unknown = row("a", Value::Integer(2));
    unknown.fields.insert("unknown".into(), Value::Integer(3));
    let mut implicit_id = original.clone();
    implicit_id
        .fields
        .insert("id".into(), Value::String("other".into()));
    for bad in [
        unknown,
        implicit_id,
        Row {
            id: "a".into(),
            fields: BTreeMap::new(),
        },
        row("a", Value::Null),
        row("a", Value::String("2".into())),
        row("a", float(2.0)),
    ] {
        invalid_row(
            run(engine.apply_typed_batch(&declared, vec![replace("items", bad.clone())]))
                .unwrap_err(),
            "items",
            "a",
        );
        let mut new = bad;
        new.id = "new".into();
        invalid_row(
            run(engine.apply_typed_batch(&declared, vec![insert("items", new)])).unwrap_err(),
            "items",
            "new",
        );
        assert_eq!(
            run(engine.get("items", "a")).unwrap(),
            Some(original.clone())
        );
        assert_eq!(run(engine.get("items", "new")).unwrap(), None);
    }
    for physical_exists in [false, true] {
        if physical_exists {
            run(engine.create_table("not_declared")).unwrap();
        }
        let mut missing = query();
        missing.table = "not_declared".into();
        let expected = StorageError::TableNotFound {
            table: "not_declared".into(),
        };
        assert_eq!(
            run(engine.query(&declared, &missing)),
            Err(expected.clone())
        );
        assert_eq!(
            run(engine.apply_typed_batch(&declared, vec![remove("not_declared", "a")])),
            Err(expected)
        );
    }
    let mut not_created = declared.clone();
    not_created
        .tables
        .insert("not_created".into(), declared.tables["items"].clone());
    let mut missing = query();
    missing.table = "not_created".into();
    assert_eq!(
        run(engine.query(&not_created, &missing)),
        Err(StorageError::TableNotFound {
            table: "not_created".into()
        })
    );
}

fn primitive_cases() -> Vec<(ColumnType, [Value; 3])> {
    vec![
        (
            ColumnType::String,
            [
                Value::String("A".into()),
                Value::String("a".into()),
                Value::String("é".into()),
            ],
        ),
        (
            ColumnType::Integer,
            [
                Value::Integer(i64::MIN),
                Value::Integer(0),
                Value::Integer(i64::MAX),
            ],
        ),
        (
            ColumnType::Float,
            [float(-f64::MAX), float(0.25), float(f64::MAX)],
        ),
        (
            ColumnType::Boolean,
            [
                Value::Boolean(false),
                Value::Boolean(true),
                Value::Boolean(true),
            ],
        ),
        (
            ColumnType::Timestamp,
            [
                Value::Timestamp(-1),
                Value::Timestamp(0),
                Value::Timestamp(1),
            ],
        ),
        (
            ColumnType::Reference,
            [
                Value::Reference("A".into()),
                Value::Reference("a".into()),
                Value::Reference("é".into()),
            ],
        ),
    ]
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn p03_every_operator_every_primitive() {
    for (kind, values) in primitive_cases() {
        let boolean = kind == ColumnType::Boolean;
        let declared = schema(kind, false);
        let rows = [
            row("a", values[0].clone()),
            row("b", values[1].clone()),
            row("c", values[2].clone()),
        ];
        let mut engine = fresh();
        run(engine.create_table("items")).unwrap();
        run(engine.apply_typed_batch(
            &declared,
            rows.iter().cloned().map(|r| insert("items", r)).collect(),
        ))
        .unwrap();
        let cases: Vec<(Operator, Vec<usize>)> = if boolean {
            vec![
                (Operator::Eq, vec![1, 2]),
                (Operator::Ne, vec![0]),
                (Operator::Lt, vec![0]),
                (Operator::Le, vec![0, 1, 2]),
                (Operator::Gt, vec![]),
                (Operator::Ge, vec![1, 2]),
            ]
        } else {
            vec![
                (Operator::Eq, vec![1]),
                (Operator::Ne, vec![0, 2]),
                (Operator::Lt, vec![0]),
                (Operator::Le, vec![0, 1]),
                (Operator::Gt, vec![2]),
                (Operator::Ge, vec![1, 2]),
            ]
        };
        for (operator, expected) in cases {
            let mut requested = query();
            requested.predicates = vec![predicate(
                "value",
                operator.clone(),
                vec![values[1].clone()],
            )];
            assert_eq!(
                run(engine.query(&declared, &requested)).unwrap(),
                selected(&rows, &expected),
                "operator {operator:?}"
            );
        }
        for (operands, expected) in [
            (vec![], vec![]),
            (vec![values[0].clone(), values[0].clone()], vec![0]),
            (
                vec![values[0].clone(), values[2].clone()],
                if boolean {
                    vec![0, 1, 2]
                } else {
                    vec![0, 2]
                },
            ),
        ] {
            let mut requested = query();
            requested.predicates = vec![predicate("value", Operator::In, operands)];
            assert_eq!(
                run(engine.query(&declared, &requested)).unwrap(),
                selected(&rows, &expected)
            );
        }
        let mut requested = query();
        requested.predicates = vec![
            predicate("value", Operator::Ge, vec![values[1].clone()]),
            predicate("id", Operator::Ne, vec![Value::String("c".into())]),
        ];
        assert_eq!(
            run(engine.query(&declared, &requested)).unwrap(),
            vec![rows[1].clone()]
        );
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn p04_optional_null_comparison_and_order() {
    let declared = schema(ColumnType::String, true);
    let rows = [
        Row {
            id: "a".into(),
            fields: BTreeMap::new(),
        },
        row("b", Value::Null),
        row("c", Value::String("x".into())),
    ];
    let mut engine = fresh();
    run(engine.create_table("items")).unwrap();
    run(engine.apply_typed_batch(
        &declared,
        rows.iter().cloned().map(|r| insert("items", r)).collect(),
    ))
    .unwrap();
    for (operator, operands, expected) in [
        (Operator::Eq, vec![Value::Null], vec![0, 1]),
        (Operator::Ne, vec![Value::Null], vec![2]),
        (Operator::Ne, vec![Value::String("x".into())], vec![0, 1]),
        (
            Operator::In,
            vec![Value::Null, Value::String("x".into())],
            vec![0, 1, 2],
        ),
        (Operator::Lt, vec![Value::String("z".into())], vec![2]),
        (Operator::Le, vec![Value::Null], vec![]),
        (Operator::Gt, vec![Value::Null], vec![]),
        (Operator::Ge, vec![Value::Null], vec![]),
    ] {
        let mut requested = query();
        requested.predicates = vec![predicate("value", operator, operands)];
        assert_eq!(
            run(engine.query(&declared, &requested)).unwrap(),
            selected(&rows, &expected)
        );
    }
    for (direction, expected) in [
        (Direction::Asc, vec![0, 1, 2]),
        (Direction::Desc, vec![2, 0, 1]),
    ] {
        let mut requested = query();
        requested.order = Some(OrderBy {
            field: "value".into(),
            direction,
        });
        assert_eq!(
            run(engine.query(&declared, &requested)).unwrap(),
            selected(&rows, &expected)
        );
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn p05_filter_order_limit_ties() {
    let mut declared = schema(ColumnType::Integer, false);
    declared
        .tables
        .get_mut("items")
        .unwrap()
        .columns
        .insert("active".into(), column(ColumnType::Boolean, false));
    let rows: Vec<Row> = [
        ("z", 1, true),
        ("a", 2, true),
        ("é", 2, true),
        ("A", 2, true),
        ("a\0", 2, true),
        ("花", 0, false),
    ]
    .into_iter()
    .map(|(id, rank, active)| {
        let mut r = row(id, Value::Integer(rank));
        r.fields.insert("active".into(), Value::Boolean(active));
        r
    })
    .collect();
    let mut engine = fresh();
    run(engine.create_table("items")).unwrap();
    run(engine.apply_typed_batch(
        &declared,
        rows.iter().cloned().map(|r| insert("items", r)).collect(),
    ))
    .unwrap();
    assert_eq!(
        run(engine.query(&declared, &query())).unwrap(),
        selected(&rows, &[3, 1, 4, 0, 2, 5])
    );
    for (direction, ordered) in [
        (Direction::Asc, vec![0, 3, 1, 4, 2]),
        (Direction::Desc, vec![3, 1, 4, 2, 0]),
    ] {
        for limit in [None, Some(0), Some(1), Some(3), Some(20)] {
            let mut requested = query();
            requested.predicates = vec![predicate(
                "active",
                Operator::Eq,
                vec![Value::Boolean(true)],
            )];
            requested.order = Some(OrderBy {
                field: "value".into(),
                direction: direction.clone(),
            });
            requested.limit = limit;
            let count = limit.unwrap_or(ordered.len()).min(ordered.len());
            assert_eq!(
                run(engine.query(&declared, &requested)).unwrap(),
                selected(&rows, &ordered[..count])
            );
        }
    }
    for (kind, values) in primitive_cases() {
        let boolean = kind == ColumnType::Boolean;
        let mut engine = fresh();
        run(engine.create_table("items")).unwrap();
        let declared = schema(kind, false);
        let rows = [
            row("a", values[0].clone()),
            row("b", values[1].clone()),
            row("c", values[2].clone()),
        ];
        run(engine.apply_typed_batch(
            &declared,
            rows.iter().cloned().map(|r| insert("items", r)).collect(),
        ))
        .unwrap();
        for (direction, expected) in [
            (Direction::Asc, vec![0, 1, 2]),
            (
                Direction::Desc,
                if boolean {
                    vec![1, 2, 0]
                } else {
                    vec![2, 1, 0]
                },
            ),
        ] {
            let mut requested = query();
            requested.order = Some(OrderBy {
                field: "value".into(),
                direction,
            });
            assert_eq!(
                run(engine.query(&declared, &requested)).unwrap(),
                selected(&rows, &expected)
            );
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn p06_direct_to_one_missing_and_cycles() {
    let mut declared = schema(ColumnType::Reference, true);
    declared.tables.get_mut("items").unwrap().relations = BTreeMap::from([
        (
            "customer".into(),
            Relation::ToOne {
                table: "customers".into(),
                foreign_key: "value".into(),
            },
        ),
        ("children".into(), Relation::HasMany),
        ("tags".into(), Relation::ManyToMany),
    ]);
    declared.tables.insert(
        "customers".into(),
        TableSchema {
            columns: BTreeMap::from([
                ("country".into(), column(ColumnType::String, true)),
                ("next".into(), column(ColumnType::Reference, true)),
            ]),
            relations: BTreeMap::from([(
                "next_customer".into(),
                Relation::ToOne {
                    table: "customers".into(),
                    foreign_key: "next".into(),
                },
            )]),
        },
    );
    let customer = |id: &str, country: Value, next: &str| Row {
        id: id.into(),
        fields: BTreeMap::from([
            ("country".into(), country),
            ("next".into(), Value::Reference(next.into())),
        ]),
    };
    let rows = [
        row("a", Value::Reference("fr".into())),
        row("b", Value::Reference("null".into())),
        row("c", Value::Reference("dangling".into())),
        Row {
            id: "d".into(),
            fields: BTreeMap::new(),
        },
        row("e", Value::Null),
        row("f", Value::Reference("de".into())),
    ];
    let mut engine = fresh();
    run(engine.create_table("items")).unwrap();
    // Declared relation targets must physically exist even when no result is needed.
    for limit in [None, Some(0)] {
        for ordered in [false, true] {
            let mut requested = query();
            requested.limit = limit;
            if ordered {
                requested.order = Some(OrderBy {
                    field: "customer.country".into(),
                    direction: Direction::Asc,
                });
            } else {
                requested.predicates = vec![predicate(
                    "customer.country",
                    Operator::Eq,
                    vec![Value::String("FR".into())],
                )];
            }
            assert_eq!(
                run(engine.query(&declared, &requested)),
                Err(StorageError::TableNotFound {
                    table: "customers".into(),
                })
            );
        }
    }
    run(engine.create_table("customers")).unwrap();
    let mut mutations = vec![
        insert(
            "customers",
            customer("fr", Value::String("FR".into()), "null"),
        ),
        insert("customers", customer("null", Value::Null, "fr")),
        insert(
            "customers",
            customer("de", Value::String("DE".into()), "de"),
        ),
    ];
    mutations.extend(rows.iter().cloned().map(|r| insert("items", r)));
    run(engine.apply_typed_batch(&declared, mutations)).unwrap();
    for (field, operator, operand, expected) in [
        (
            "customer.country",
            Operator::Eq,
            Value::String("FR".into()),
            vec![0],
        ),
        (
            "customer.country",
            Operator::Ne,
            Value::String("FR".into()),
            vec![1, 5],
        ),
        ("customer.country", Operator::Eq, Value::Null, vec![1]),
        ("customer.country", Operator::Ne, Value::Null, vec![0, 5]),
        (
            "customer.id",
            Operator::Eq,
            Value::String("fr".into()),
            vec![0],
        ),
        (
            "customer.id",
            Operator::Ne,
            Value::String("fr".into()),
            vec![1, 5],
        ),
    ] {
        let mut requested = query();
        requested.predicates = vec![predicate(field, operator, vec![operand])];
        assert_eq!(
            run(engine.query(&declared, &requested)).unwrap(),
            selected(&rows, &expected)
        );
    }
    for (direction, expected) in [
        (Direction::Asc, vec![1, 2, 3, 4, 5, 0]),
        (Direction::Desc, vec![0, 5, 1, 2, 3, 4]),
    ] {
        let mut requested = query();
        requested.order = Some(OrderBy {
            field: "customer.country".into(),
            direction,
        });
        assert_eq!(
            run(engine.query(&declared, &requested)).unwrap(),
            selected(&rows, &expected)
        );
    }
    for field in [
        "customer.next_customer.country",
        "children.country",
        "tags.id",
    ] {
        let mut requested = query();
        requested.predicates = vec![predicate(
            field,
            Operator::Eq,
            vec![Value::String("FR".into())],
        )];
        assert!(matches!(
            run(engine.query(&declared, &requested)),
            Err(StorageError::InvalidQuery { .. })
        ));
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn p07_invalid_queries_before_execution() {
    for populated in [false, true] {
        let mut engine = fresh();
        run(engine.create_table("items")).unwrap();
        let declared = schema(ColumnType::Float, false);
        if populated {
            run(engine.insert("items", row("a", float(1.0)))).unwrap();
        }
        let mut invalid = Vec::new();
        for field in [
            "missing",
            "unknown.field",
            "x.y.z",
            "value.x",
            "",
            ".value",
            "value.",
        ] {
            let mut requested = query();
            requested.predicates = vec![predicate(field, Operator::Eq, vec![float(1.0)])];
            invalid.push(requested);
            let mut requested = query();
            requested.order = Some(OrderBy {
                field: field.into(),
                direction: Direction::Asc,
            });
            invalid.push(requested);
        }
        for operator in [
            Operator::Eq,
            Operator::Ne,
            Operator::Lt,
            Operator::Le,
            Operator::Gt,
            Operator::Ge,
        ] {
            for values in [
                vec![],
                vec![float(1.0), float(2.0)],
                vec![Value::Integer(1)],
                vec![Value::Null],
            ] {
                let mut requested = query();
                requested.predicates = vec![predicate("value", operator.clone(), values)];
                invalid.push(requested);
            }
        }
        for values in [
            vec![float(1.0), Value::Integer(1)],
            vec![Value::Null],
            vec![Value::String("1".into())],
        ] {
            let mut requested = query();
            requested.predicates = vec![predicate("value", Operator::In, values)];
            invalid.push(requested);
        }
        for mut requested in invalid {
            for limit in [None, Some(0)] {
                requested.limit = limit;
                assert!(
                    matches!(
                        run(engine.query(&declared, &requested)),
                        Err(StorageError::InvalidQuery { .. })
                    ),
                    "invalid query {requested:?}"
                );
            }
        }
        let integers = schema(ColumnType::Integer, false);
        let mut requested = query();
        requested.predicates = vec![predicate("value", Operator::Eq, vec![float(1.0)])];
        assert!(matches!(
            run(engine.query(&integers, &requested)),
            Err(StorageError::InvalidQuery { .. })
        ));
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn p08_committed_changes_order_owned_noops() {
    let mut engine = fresh();
    for table in ["items", "other"] {
        run(engine.create_table(table)).unwrap();
    }
    let mut declared = schema(ColumnType::Integer, false);
    declared
        .tables
        .insert("other".into(), declared.tables["items"].clone());
    let a = row("a", Value::Integer(1));
    let a2 = row("a", Value::Integer(2));
    let b = row("b", Value::Integer(3));
    run(engine.insert("items", a.clone())).unwrap();
    let mut changes = run(engine.apply_typed_batch(
        &declared,
        vec![
            insert("other", b.clone()),
            replace("items", a2.clone()),
            replace("items", a2.clone()),
            remove("items", "missing"),
            remove("items", "a"),
            insert("items", a.clone()),
        ],
    ))
    .unwrap();
    assert_eq!(
        changes,
        vec![
            CommittedChange {
                table: "other".into(),
                id: "b".into(),
                before: None,
                after: Some(b.clone())
            },
            CommittedChange {
                table: "items".into(),
                id: "a".into(),
                before: Some(a.clone()),
                after: Some(a2.clone())
            },
            CommittedChange {
                table: "items".into(),
                id: "a".into(),
                before: Some(a2),
                after: None
            },
            CommittedChange {
                table: "items".into(),
                id: "a".into(),
                before: None,
                after: Some(a.clone())
            },
        ]
    );
    changes[0].after.as_mut().unwrap().fields.clear();
    changes[1].before.as_mut().unwrap().id = "mutated".into();
    assert_eq!(run(engine.get("items", "a")).unwrap(), Some(a));
    assert_eq!(run(engine.get("other", "b")).unwrap(), Some(b));
    assert_eq!(
        run(engine.apply_typed_batch(&declared, vec![])).unwrap(),
        Vec::<CommittedChange>::new()
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn p09_rollback_and_invalid_returned_rows() {
    let mut engine = fresh();
    run(engine.create_table("items")).unwrap();
    let declared = schema(ColumnType::Integer, false);
    let original = row("a", Value::Integer(1));
    run(engine.insert("items", original.clone())).unwrap();
    for (failure, category) in [
        (insert("items", original.clone()), "duplicate"),
        (
            replace("items", row("missing", Value::Integer(0))),
            "missing",
        ),
        (
            replace("items", row("a", Value::String("bad".into()))),
            "invalid",
        ),
        (remove("unknown", "a"), "table"),
    ] {
        let error = run(engine.apply_typed_batch(
            &declared,
            vec![
                replace("items", row("a", Value::Integer(2))),
                failure,
                insert("items", row("never", Value::Integer(3))),
            ],
        ))
        .unwrap_err();
        match category {
            "duplicate" => assert_eq!(
                error,
                StorageError::RowAlreadyExists {
                    table: "items".into(),
                    id: "a".into()
                }
            ),
            "missing" => assert_eq!(
                error,
                StorageError::RowNotFound {
                    table: "items".into(),
                    id: "missing".into()
                }
            ),
            "invalid" => invalid_row(error, "items", "a"),
            "table" => assert_eq!(
                error,
                StorageError::TableNotFound {
                    table: "unknown".into()
                }
            ),
            _ => unreachable!(),
        }
        assert_eq!(
            run(engine.query(&declared, &query())).unwrap(),
            vec![original.clone()]
        );
        assert_eq!(run(engine.get("items", "never")).unwrap(), None);
    }
    let malformed = row("z-bad", Value::String("wrong kind".into()));
    run(engine.insert("items", malformed.clone())).unwrap();
    invalid_row(
        run(engine.query(&declared, &query())).unwrap_err(),
        "items",
        "z-bad",
    );
    let mut safe = query();
    safe.predicates = vec![predicate(
        "id",
        Operator::Eq,
        vec![Value::String("a".into())],
    )];
    assert_eq!(
        run(engine.query(&declared, &safe)).unwrap(),
        vec![original.clone()]
    );
    for mutation in [
        replace("items", row("z-bad", Value::Integer(9))),
        remove("items", "z-bad"),
    ] {
        invalid_row(
            run(engine.apply_typed_batch(
                &declared,
                vec![replace("items", row("a", Value::Integer(5))), mutation],
            ))
            .unwrap_err(),
            "items",
            "z-bad",
        );
        assert_eq!(
            run(engine.get("items", "a")).unwrap(),
            Some(original.clone())
        );
        assert_eq!(
            run(engine.get("items", "z-bad")).unwrap(),
            Some(malformed.clone())
        );
    }
    // Wrong-kind raw fields never participate in a typed predicate, even Ne.
    let nullable = schema(ColumnType::Integer, true);
    for (operator, operands, expected) in [
        (
            Operator::Eq,
            vec![Value::Integer(1)],
            vec![original.clone()],
        ),
        (Operator::Ne, vec![Value::Integer(1)], vec![]),
        (
            Operator::Lt,
            vec![Value::Integer(2)],
            vec![original.clone()],
        ),
        (
            Operator::Le,
            vec![Value::Integer(1)],
            vec![original.clone()],
        ),
        (
            Operator::Gt,
            vec![Value::Integer(0)],
            vec![original.clone()],
        ),
        (
            Operator::Ge,
            vec![Value::Integer(1)],
            vec![original.clone()],
        ),
        (
            Operator::In,
            vec![Value::Null, Value::Integer(1)],
            vec![original.clone()],
        ),
        (Operator::Eq, vec![Value::Null], vec![]),
        (Operator::Ne, vec![Value::Null], vec![original.clone()]),
    ] {
        let mut requested = query();
        requested.predicates = vec![predicate("value", operator, operands)];
        assert_eq!(run(engine.query(&nullable, &requested)).unwrap(), expected);
    }
    let mut ordered = query();
    ordered.order = Some(OrderBy {
        field: "value".into(),
        direction: Direction::Desc,
    });
    ordered.limit = Some(1);
    assert_eq!(
        run(engine.query(&nullable, &ordered)).unwrap(),
        vec![original]
    );
    ordered.order.as_mut().unwrap().direction = Direction::Asc;
    invalid_row(
        run(engine.query(&nullable, &ordered)).unwrap_err(),
        "items",
        "z-bad",
    );

    // Related rows need not be wholly valid, but consulted fields obey their declared type.
    let mut engine = fresh();
    run(engine.create_table("items")).unwrap();
    run(engine.create_table("customers")).unwrap();
    let mut declared = schema(ColumnType::Reference, false);
    declared.tables.get_mut("items").unwrap().relations.insert(
        "customer".into(),
        Relation::ToOne {
            table: "customers".into(),
            foreign_key: "value".into(),
        },
    );
    declared.tables.insert(
        "customers".into(),
        TableSchema {
            columns: BTreeMap::from([
                ("country".into(), column(ColumnType::String, true)),
                ("required".into(), column(ColumnType::String, false)),
            ]),
            relations: BTreeMap::new(),
        },
    );
    let rows = [
        row("a", Value::Reference("wrong".into())),
        row("b", Value::Reference("de".into())),
        row("c", Value::Reference("null".into())),
    ];
    for r in &rows {
        run(engine.insert("items", r.clone())).unwrap();
    }
    for (id, country, required) in [
        ("wrong", Value::Integer(5), Some(Value::String("FR".into()))),
        ("de", Value::String("DE".into()), None),
        ("null", Value::Null, Some(Value::Null)),
    ] {
        let mut fields = BTreeMap::from([("country".into(), country)]);
        if let Some(value) = required {
            fields.insert("required".into(), value);
        }
        run(engine.insert(
            "customers",
            Row {
                id: id.into(),
                fields,
            },
        ))
        .unwrap();
    }
    for (operator, operands, expected) in [
        (Operator::Eq, vec![Value::String("DE".into())], vec![1]),
        (Operator::Ne, vec![Value::String("FR".into())], vec![1, 2]),
        (Operator::Lt, vec![Value::String("FR".into())], vec![1]),
        (Operator::Le, vec![Value::String("DE".into())], vec![1]),
        (Operator::Gt, vec![Value::String("AA".into())], vec![1]),
        (Operator::Ge, vec![Value::String("DE".into())], vec![1]),
        (
            Operator::In,
            vec![Value::Null, Value::String("DE".into())],
            vec![1, 2],
        ),
        (Operator::Eq, vec![Value::Null], vec![2]),
        (Operator::Ne, vec![Value::Null], vec![1]),
    ] {
        let mut requested = query();
        requested.predicates = vec![predicate("customer.country", operator, operands)];
        assert_eq!(
            run(engine.query(&declared, &requested)).unwrap(),
            selected(&rows, &expected)
        );
    }
    for operator in [
        Operator::Eq,
        Operator::Ne,
        Operator::Lt,
        Operator::Le,
        Operator::Gt,
        Operator::Ge,
        Operator::In,
    ] {
        let mut requested = query();
        requested.predicates = vec![predicate(
            "customer.required",
            operator.clone(),
            vec![Value::String("FR".into())],
        )];
        let expected: &[usize] = if matches!(
            operator,
            Operator::Eq | Operator::Le | Operator::Ge | Operator::In
        ) {
            &[0]
        } else {
            &[]
        };
        assert_eq!(
            run(engine.query(&declared, &requested)).unwrap(),
            selected(&rows, expected)
        );
    }
    for (direction, expected) in [
        (Direction::Asc, vec![0, 2, 1]),
        (Direction::Desc, vec![1, 0, 2]),
    ] {
        let mut requested = query();
        requested.order = Some(OrderBy {
            field: "customer.country".into(),
            direction,
        });
        assert_eq!(
            run(engine.query(&declared, &requested)).unwrap(),
            selected(&rows, &expected)
        );
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
pub fn p10_all_write_paths_keep_queries_coherent() {
    let mut engine = fresh();
    run(engine.create_table("items")).unwrap();
    let declared = schema(ColumnType::String, false);
    let target = "'\0é; DROP TABLE __hoya_rows;--";
    let a = row("a\0'", Value::String(target.into()));
    let b = row("b", Value::String("other".into()));
    run(engine.apply_typed_batch(
        &declared,
        vec![insert("items", a.clone()), insert("items", b.clone())],
    ))
    .unwrap();
    let mut requested = query();
    requested.predicates = vec![predicate(
        "value",
        Operator::Eq,
        vec![Value::String(target.into())],
    )];
    assert_eq!(
        run(engine.query(&declared, &requested)).unwrap(),
        vec![a.clone()]
    );
    let b2 = row("b", Value::String(target.into()));
    run(engine.replace("items", b2.clone())).unwrap();
    assert_eq!(
        run(engine.query(&declared, &requested)).unwrap(),
        vec![a.clone(), b2.clone()]
    );
    run(engine.remove("items", &a.id)).unwrap();
    assert_eq!(
        run(engine.query(&declared, &requested)).unwrap(),
        vec![b2.clone()]
    );
    run(engine.apply_batch(vec![
        insert("items", a.clone()),
        replace("items", b.clone()),
    ]))
    .unwrap();
    assert_eq!(
        run(engine.query(&declared, &requested)).unwrap(),
        vec![a.clone()]
    );
    let failed = run(engine.apply_typed_batch(
        &declared,
        vec![replace("items", b2.clone()), insert("items", a.clone())],
    ));
    assert_eq!(
        failed,
        Err(StorageError::RowAlreadyExists {
            table: "items".into(),
            id: a.id.clone()
        })
    );
    assert_eq!(
        run(engine.query(&declared, &requested)).unwrap(),
        vec![a.clone()]
    );
    assert_eq!(run(engine.get("items", "b")).unwrap(), Some(b));
    run(engine.apply_typed_batch(
        &declared,
        vec![remove("items", &a.id), replace("items", b2.clone())],
    ))
    .unwrap();
    assert_eq!(run(engine.query(&declared, &requested)).unwrap(), vec![b2]);
    // Logical table/field names are bound values too, not only operands.
    let table = "table\0'; DROP TABLE x;--";
    let field = "field\0'; DROP TABLE x;--";
    run(engine.create_table(table)).unwrap();
    let unusual = Schema {
        tables: BTreeMap::from([(
            table.into(),
            TableSchema {
                columns: BTreeMap::from([(field.into(), column(ColumnType::String, false))]),
                relations: BTreeMap::new(),
            },
        )]),
    };
    let wanted = Row {
        id: "row\0'".into(),
        fields: BTreeMap::from([(field.into(), Value::String(target.into()))]),
    };
    run(engine.apply_typed_batch(&unusual, vec![insert(table, wanted.clone())])).unwrap();
    let mut q = query();
    q.table = table.into();
    q.predicates = vec![predicate(
        field,
        Operator::Eq,
        vec![Value::String(target.into())],
    )];
    assert_eq!(run(engine.query(&unusual, &q)).unwrap(), vec![wanted]);
    assert_eq!(
        run(engine.query(&declared, &requested)).unwrap(),
        vec![row("b", Value::String(target.into()))]
    );
}
