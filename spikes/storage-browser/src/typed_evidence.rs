//! SPEC-0025 / TC-0025 P11/P12: fixture-only physical and cost evidence.
use super::*;
use hoyadb_core::storage::{
    ColumnType, Direction, Operator, OrderBy, TypedStorage, TypedTransactionalStorage,
};
use typed_conformance_cases::{column, float, insert, predicate, query, replace, row, schema};

fn connection(name: &str) -> rusqlite::Connection {
    rusqlite::Connection::open_with_flags_and_vfs(
        name,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_CREATE,
        "opfs-sahpool",
    )
    .unwrap()
}

fn persisted_rows() -> Vec<Row> {
    vec![Row {
        id: "typed-owned".into(),
        fields: BTreeMap::from([
            ("float".into(), float(f64::MAX)),
            ("integer".into(), Value::Integer(i64::MIN)),
            ("timestamp".into(), Value::Timestamp(i64::MAX)),
            (
                "reference".into(),
                Value::Reference("引用\0'; DROP TABLE x;--".into()),
            ),
            ("null".into(), Value::Null),
        ]),
    }]
}

fn persisted_schema() -> hoyadb_core::storage::Schema {
    hoyadb_core::storage::Schema {
        tables: BTreeMap::from([(
            "phase-one".into(),
            hoyadb_core::storage::TableSchema {
                columns: BTreeMap::from([
                    ("float".into(), column(ColumnType::Float, false)),
                    ("integer".into(), column(ColumnType::Integer, false)),
                    ("timestamp".into(), column(ColumnType::Timestamp, false)),
                    ("reference".into(), column(ColumnType::Reference, false)),
                    ("null".into(), column(ColumnType::String, true)),
                ]),
                relations: BTreeMap::new(),
            },
        )]),
    }
}

#[wasm_bindgen]
impl BrowserStore {
    /// Fixed fixture schema, deliberately not a consumer schema API.
    pub fn typed_fixture_write(&mut self) -> Result<(), JsValue> {
        run(self.storage.create_table("phase-one")).map_err(error)?;
        let changes = run(self.storage.apply_typed_batch(
            &persisted_schema(),
            persisted_rows()
                .into_iter()
                .map(|row| insert("phase-one", row))
                .collect(),
        ))
        .map_err(error)?;
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].before, None);
        assert_eq!(changes[0].after, Some(persisted_rows().remove(0)));
        Ok(())
    }

    pub fn typed_fixture_read(&self) -> Result<JsValue, JsValue> {
        let mut q = query();
        q.table = "phase-one".into();
        let result = run(self.storage.query(&persisted_schema(), &q)).map_err(error)?;
        assert_eq!(result, persisted_rows());
        self.get("phase-one", "typed-owned")
    }
}

#[wasm_bindgen]
pub fn typed_physical() -> Array {
    let passed = Array::new();
    // Exact preextension v1 format, created independently of adapter initialization.
    const NAME: &str = "typed-legacy.sqlite";
    {
        let c = connection(NAME);
        c.execute_batch("CREATE TABLE __hoya_meta(version INTEGER NOT NULL); INSERT INTO __hoya_meta VALUES(1);
            CREATE TABLE __hoya_tables(name TEXT PRIMARY KEY NOT NULL);
            CREATE TABLE __hoya_rows(table_name TEXT NOT NULL,id TEXT NOT NULL,PRIMARY KEY(table_name,id));
            CREATE TABLE __hoya_fields(table_name TEXT NOT NULL,row_id TEXT NOT NULL,name TEXT NOT NULL,kind INTEGER NOT NULL,text_value TEXT,int_value INTEGER,PRIMARY KEY(table_name,row_id,name));
            INSERT INTO __hoya_tables VALUES('items'); INSERT INTO __hoya_rows VALUES('items','old');
            INSERT INTO __hoya_fields VALUES('items','old','value',2,NULL,42);").unwrap();
    }
    let ddl = |c: &rusqlite::Connection| {
        c.prepare("SELECT name,sql FROM sqlite_schema WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    };
    let old_ddl = ddl(&connection(NAME));
    {
        let mut s = SqliteStorage::open_unprotected(NAME).unwrap();
        assert_eq!(
            run(s.get("items", "old")).unwrap(),
            Some(row("old", Value::Integer(42)))
        );
        run(s.create_table("phase-one")).unwrap();
        let changes = run(s.apply_typed_batch(
            &persisted_schema(),
            persisted_rows()
                .into_iter()
                .map(|r| insert("phase-one", r))
                .collect(),
        ))
        .unwrap();
        assert_eq!(changes.len(), 1);
    }
    {
        let c = connection(NAME);
        assert_eq!(ddl(&c), old_ddl);
        assert_eq!(
            c.query_row("SELECT version FROM __hoya_meta", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    {
        let s = SqliteStorage::open_unprotected(NAME).unwrap();
        assert_eq!(
            run(s.get("items", "old")).unwrap(),
            Some(row("old", Value::Integer(42)))
        );
        assert_eq!(
            run(s.get("phase-one", "typed-owned")).unwrap(),
            Some(persisted_rows().remove(0))
        );
    }
    passed.push(&"P11 v1 DDL marker old row and typed batch retained".into());
    // Every malformed physical payload must fail instead of coercing a partial row.
    const BAD: &str = "typed-malformed.sqlite";
    {
        let mut s = SqliteStorage::open_unprotected(BAD).unwrap();
        run(s.create_table("items")).unwrap();
        run(s.insert("items", row("bad", Value::Integer(1)))).unwrap();
    }
    for payload in [
        "NaN", // Exercise binding too: SQLite converts NaN to NULL.
        "kind=4,text_value=NULL,int_value=1e999",
        "kind=4,text_value=NULL,int_value=9007199254740993",
        "kind=4,text_value='1.5',int_value=NULL",
        "kind=5,text_value=NULL,int_value=1.5",
        "kind=6,text_value=NULL,int_value=1",
        "kind=6,text_value=CAST(X'80' AS TEXT),int_value=NULL",
        "kind=7,text_value='forbidden',int_value=NULL",
        "kind=7,text_value=NULL,int_value=1",
    ] {
        {
            let c = connection(BAD);
            if payload == "NaN" {
                c.execute(
                    "UPDATE __hoya_fields SET kind=4,text_value=NULL,int_value=?1",
                    [f64::NAN],
                )
                .unwrap();
            } else {
                c.execute_batch(&format!("UPDATE __hoya_fields SET {payload}"))
                    .unwrap();
            }
        }
        let s = SqliteStorage::open_unprotected(BAD).unwrap();
        assert!(
            matches!(
                run(s.get("items", "bad")),
                Err(StorageError::BackendFailure { .. })
            ),
            "{payload}"
        );
    }
    passed.push(&"P11 malformed Float Timestamp Reference Null payloads reject strictly".into());
    const FAIL: &str = "typed-trigger.sqlite";
    let definition = schema(ColumnType::Integer, false);
    let original = row("keep", Value::Integer(7));
    {
        let mut s = SqliteStorage::open_unprotected(FAIL).unwrap();
        run(s.create_table("items")).unwrap();
        run(s.apply_typed_batch(&definition, vec![insert("items", original.clone())])).unwrap();
    }
    {
        connection(FAIL).execute_batch("CREATE TRIGGER fixture_abort BEFORE INSERT ON __hoya_rows WHEN NEW.id='fail' BEGIN SELECT RAISE(ABORT,'fixture ordinary write failure'); END;").unwrap();
    }
    {
        let mut s = SqliteStorage::open_unprotected(FAIL).unwrap();
        assert!(matches!(
            run(s.apply_typed_batch(
                &definition,
                vec![
                    replace("items", row("keep", Value::Integer(8))),
                    insert("items", row("fail", Value::Integer(9)))
                ]
            )),
            Err(StorageError::BackendFailure { .. })
        ));
        assert_eq!(
            run(s.query(&definition, &query())).unwrap(),
            vec![original.clone()]
        );
    }
    let s = SqliteStorage::open_unprotected(FAIL).unwrap();
    assert_eq!(run(s.query(&definition, &query())).unwrap(), vec![original]);
    passed.push(
        &"P11 actual SQL trigger abort rolls back prefix and emits no changes after reopen".into(),
    );
    passed
}

fn millis() -> f64 {
    js_sys::Date::now()
}

fn cost_row(id: usize, number: i64) -> Row {
    Row {
        id: format!("{id:08}"),
        fields: BTreeMap::from([
            ("number".into(), Value::Integer(number)),
            ("text".into(), Value::String(format!("key-{number:08}"))),
        ]),
    }
}

fn cost_schema() -> hoyadb_core::storage::Schema {
    let mut result = schema(ColumnType::Integer, false);
    result.tables.get_mut("items").unwrap().columns = BTreeMap::from([
        ("number".into(), column(ColumnType::Integer, false)),
        ("text".into(), column(ColumnType::String, false)),
    ]);
    result
}

#[wasm_bindgen]
pub fn typed_index_evidence() -> Array {
    const NAME: &str = "typed-index.sqlite";
    let evidence = Array::new();
    let definition = cost_schema();
    let mut s = SqliteStorage::open_unprotected(NAME).unwrap();
    run(s.create_table("items")).unwrap();
    let start = millis();
    run(s.apply_batch(
        (0..256)
            .map(|i| insert("items", cost_row(i, i as i64)))
            .collect(),
    ))
    .unwrap();
    let initial_population_ms = millis() - start;
    drop(s);
    // Remove only fixture-owned additive indexes, then measure a populated build.
    connection(NAME)
        .execute_batch("DROP INDEX __hoya_fields_numeric; DROP INDEX __hoya_fields_text")
        .unwrap();
    let start = millis();
    let mut s = SqliteStorage::open_unprotected(NAME).unwrap();
    let populated_index_build_ms = millis() - start;
    let mut numeric = query();
    numeric.predicates = vec![predicate("number", Operator::Eq, vec![Value::Integer(42)])];
    let expected = vec![cost_row(42, 42)];
    for size in [256usize, 2048] {
        let start = millis();
        run(s.apply_typed_batch(
            &definition,
            (256..size)
                .map(|i| insert("items", cost_row(i, i as i64)))
                .collect(),
        ))
        .unwrap();
        let added_population_ms = millis() - start;
        let lookups = Array::new();
        for (field, value, index) in [
            ("number", Value::Integer(42), "__hoya_fields_numeric"),
            (
                "text",
                Value::String("key-00000042".into()),
                "__hoya_fields_text",
            ),
        ] {
            let mut q = query();
            q.predicates = vec![predicate(field, Operator::Eq, vec![value])];
            let plan = s.query_plan(&definition, &q).unwrap();
            assert!(
                plan.iter()
                    .any(|p| p.contains("SEARCH") && p.contains(index)),
                "{plan:?}"
            );
            let start = millis();
            assert_eq!(run(s.query(&definition, &q)).unwrap(), expected);
            let first_lookup_ms = millis() - start;
            let start = millis();
            for _ in 0..25 {
                assert_eq!(run(s.query(&definition, &q)).unwrap(), expected);
            }
            lookups.push(&object(&[
                ("field", field.into()),
                ("first_lookup_ms", first_lookup_ms.into()),
                ("repeated_25_ms", (millis() - start).into()),
                (
                    "plan",
                    plan.into_iter()
                        .map(JsValue::from)
                        .collect::<Array>()
                        .into(),
                ),
            ]));
        }
        let start = millis();
        run(s.apply_typed_batch(&definition, vec![replace("items", cost_row(42, -1))])).unwrap();
        let typed_update_ms = millis() - start;
        assert!(run(s.query(&definition, &numeric)).unwrap().is_empty());
        let mut changed = query();
        changed.predicates = vec![predicate("number", Operator::Eq, vec![Value::Integer(-1)])];
        assert_eq!(
            run(s.query(&definition, &changed)).unwrap(),
            vec![cost_row(42, -1)]
        );
        let start = millis();
        run(s.replace("items", expected[0].clone())).unwrap();
        let raw_update_ms = millis() - start;
        assert_eq!(run(s.query(&definition, &numeric)).unwrap(), expected);
        let start = millis();
        assert!(matches!(
            run(s.apply_typed_batch(
                &definition,
                vec![
                    Mutation::Remove {
                        table: "items".into(),
                        id: "00000042".into()
                    },
                    insert("items", cost_row(0, 0)),
                ]
            )),
            Err(StorageError::RowAlreadyExists { .. })
        ));
        let rollback_ms = millis() - start;
        assert_eq!(run(s.query(&definition, &numeric)).unwrap(), expected);
        let mut ordered = query();
        ordered.order = Some(OrderBy {
            field: "number".into(),
            direction: Direction::Desc,
        });
        ordered.limit = Some(3);
        let start = millis();
        assert_eq!(
            run(s.query(&definition, &ordered)).unwrap(),
            (size - 3..size)
                .rev()
                .map(|i| cost_row(i, i as i64))
                .collect::<Vec<_>>()
        );
        let ordering_ms = millis() - start;
        drop(s);
        let start = millis();
        s = SqliteStorage::open_unprotected(NAME).unwrap();
        let reopen_ms = millis() - start;
        assert_eq!(run(s.query(&definition, &numeric)).unwrap(), expected);
        let reopened_plan = s.query_plan(&definition, &numeric).unwrap();
        assert!(
            reopened_plan
                .iter()
                .any(|p| p.contains("SEARCH") && p.contains("__hoya_fields_numeric")),
            "{reopened_plan:?}"
        );
        evidence.push(&object(&[
            ("size", JsValue::from_f64(size as f64)),
            (
                "initial_256_raw_population_ms",
                initial_population_ms.into(),
            ),
            ("populated_index_build_ms", populated_index_build_ms.into()),
            ("added_population_ms", added_population_ms.into()),
            ("lookups", lookups.into()),
            ("typed_update_ms", typed_update_ms.into()),
            ("raw_update_ms", raw_update_ms.into()),
            ("rollback_ms", rollback_ms.into()),
            ("ordering_ms", ordering_ms.into()),
            ("reopen_ms", reopen_ms.into()),
            (
                "reopened_plan",
                reopened_plan
                    .into_iter()
                    .map(JsValue::from)
                    .collect::<Array>()
                    .into(),
            ),
        ]));
    }
    evidence
}
