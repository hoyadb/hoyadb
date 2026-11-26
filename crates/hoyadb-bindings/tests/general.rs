//! TC-0036 G01/G20: real caller-defined schemas, never a fixture schema.
use hoyadb_bindings::Facade;
use hoyadb_core::command::Json;
use std::collections::BTreeMap;
use std::sync::Mutex;

static OWNERS: Mutex<()> = Mutex::new(());

fn object<const N: usize>(values: [(&str, Json); N]) -> Json {
    Json::Object(values.into_iter().map(|(k, v)| (k.into(), v)).collect())
}

fn text(value: &str) -> Json {
    Json::String(value.into())
}

fn frame(op: &str, owner: Json, args: Json) -> String {
    object([
        ("abi", text("1")),
        ("op", text(op)),
        ("owner", owner),
        ("args", args),
    ])
    .canonical()
    .unwrap()
}

#[track_caller]
fn success(output: String) -> Json {
    let Json::Object(response) = Json::parse(&output).unwrap() else {
        panic!("facade must return an object");
    };
    assert_eq!(response["abi"], text("1"));
    assert_eq!(response["ok"], Json::Bool(true), "{output}");
    response["value"].clone()
}

fn schema(table: &str, field: &str) -> Json {
    object([
        ("version", text("1")),
        (
            "tables",
            object([(
                table,
                object([
                    (
                        "columns",
                        object([(
                            field,
                            object([("kind", text("String")), ("optional", Json::Bool(false))]),
                        )]),
                    ),
                    ("relations", Json::Object(BTreeMap::new())),
                ]),
            )]),
        ),
    ])
}

fn memory_args(schema: Json, namespace: &str) -> Json {
    object([
        (
            "engine",
            object([("kind", text("memory")), ("store", Json::Null)]),
        ),
        ("schema", schema),
        ("migrations", Json::Array(vec![])),
        ("namespace", text(namespace)),
        (
            "options",
            object([
                ("capacity", text("1000")),
                ("history", text("100")),
                ("initial_delay_ms", text("1000")),
                ("multiplier", text("2")),
                ("max_delay_ms", text("60000")),
            ]),
        ),
    ])
}

fn tagged(kind: &str, value: Json) -> Json {
    object([("kind", text(kind)), ("value", value)])
}

fn query(table: &str) -> Json {
    object([
        ("table", text(table)),
        ("predicates", Json::Array(vec![])),
        ("order", Json::Null),
        ("limit", Json::Null),
        ("source", Json::Null),
    ])
}

fn failure(output: String, kind: &str) {
    assert_eq!(
        Json::parse(&output).unwrap(),
        object([
            ("abi", text("1")),
            ("ok", Json::Bool(false)),
            ("error", object([("kind", text(kind))])),
        ])
    );
}

#[test]
fn g03_lossless_business_codec_keeps_i64_float_kind_and_unicode() {
    let mut facade = Facade::new();
    let body = r#"{"large":9223372036854775807,"small":-9223372036854775808,"float":42.0,"label":"東京\u0000🌿"}"#;
    let decoded = success(facade.call(&frame(
        "json_decode",
        Json::Null,
        object([("body", text(body))]),
    )));
    let expected = tagged(
        "object",
        object([
            ("large", tagged("integer", text("9223372036854775807"))),
            ("small", tagged("integer", text("-9223372036854775808"))),
            ("float", tagged("float", text("42"))),
            ("label", tagged("string", text("東京\0🌿"))),
        ]),
    );
    assert_eq!(decoded, expected);
    let encoded = success(facade.call(&frame(
        "json_encode",
        Json::Null,
        object([("value", decoded)]),
    )));
    let Json::String(encoded) = encoded else {
        panic!("canonical string");
    };
    assert_eq!(Json::parse(&encoded).unwrap(), Json::parse(body).unwrap());
    for invalid in [" 42", "42 ", "NaN", "Infinity", "01", "1e999"] {
        failure(
            facade.call(&frame(
                "json_encode",
                Json::Null,
                object([("value", tagged("float", text(invalid)))]),
            )),
            "InvalidRequest",
        );
    }
}

#[test]
fn g01_g03_g04_arbitrary_typed_crud_queries_and_atomic_batch_use_real_core() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let definition = Json::parse(r#"{"version":"1","tables":{"readings":{"columns":{"label":{"kind":"String","optional":false},"count":{"kind":"Integer","optional":false},"score":{"kind":"Float","optional":false},"at":{"kind":"Timestamp","optional":false},"ready":{"kind":"Boolean","optional":false},"note":{"kind":"String","optional":true}},"relations":{}}}}"#).unwrap();
    let owner = success(facade.open(
        &frame(
            "open",
            Json::Null,
            memory_args(definition, "05050505050505050505050505050505"),
        ),
        vec![],
    ));
    let fields = object([
        ("label", tagged("string", text("東京\0🌿"))),
        ("count", tagged("integer", text("9223372036854775807"))),
        ("score", tagged("float", text("42.0"))),
        ("at", tagged("timestamp", text("-9223372036854775808"))),
        ("ready", tagged("boolean", Json::Bool(true))),
        ("note", object([("kind", text("null"))])),
    ]);
    let row = success(facade.call(&frame(
        "insert",
        owner.clone(),
        object([
            ("table", text("readings")),
            ("fields", fields.clone()),
            ("proposed_id", text("test-fixed-id-001")),
        ]),
    )));
    let Json::Object(ref row_map) = row else {
        panic!("row");
    };
    assert_eq!(row_map["id"], text("test-fixed-id-001"));
    let Json::Object(mut normalized) = fields else {
        unreachable!()
    };
    normalized.insert("score".into(), tagged("float", text("42")));
    assert_eq!(row_map["fields"], Json::Object(normalized.clone()));
    let mut q = query("readings");
    let Json::Object(ref mut qmap) = q else {
        unreachable!()
    };
    qmap.insert(
        "predicates".into(),
        Json::Array(vec![object([
            ("field", text("count")),
            ("operator", text("Ge")),
            (
                "values",
                Json::Array(vec![tagged("integer", text("9223372036854775807"))]),
            ),
        ])]),
    );
    qmap.insert(
        "order".into(),
        object([("field", text("label")), ("direction", text("Desc"))]),
    );
    qmap.insert("limit".into(), text("1"));
    assert_eq!(
        success(facade.call(&frame("fetch", owner.clone(), object([("query", q)])))),
        Json::Array(vec![row.clone()])
    );
    normalized.insert("label".into(), tagged("string", text("changed")));
    let changed = success(facade.call(&frame(
        "replace",
        owner.clone(),
        object([
            ("table", text("readings")),
            ("id", text("test-fixed-id-001")),
            ("fields", Json::Object(normalized.clone())),
        ]),
    )));
    let writes = Json::Array(vec![
        object([
            ("op", text("remove")),
            ("table", text("readings")),
            ("id", text("test-fixed-id-001")),
        ]),
        object([
            ("op", text("replace")),
            ("table", text("readings")),
            ("id", text("missing")),
            ("fields", Json::Object(normalized)),
        ]),
    ]);
    failure(
        facade.call(&frame("batch", owner.clone(), object([("writes", writes)]))),
        "RowNotFound",
    );
    assert_eq!(
        success(facade.call(&frame(
            "fetch",
            owner.clone(),
            object([("query", query("readings"))])
        ))),
        Json::Array(vec![changed])
    );
    assert_eq!(
        success(facade.call(&frame(
            "remove",
            owner.clone(),
            object([
                ("table", text("readings")),
                ("id", text("test-fixed-id-001"))
            ])
        ))),
        Json::Bool(true)
    );
    assert_eq!(
        success(facade.call(&frame(
            "fetch",
            owner.clone(),
            object([("query", query("readings"))])
        ))),
        Json::Array(vec![])
    );
    success(facade.call(&frame("close", owner, object([]))));
}

#[test]
fn g11_batch_reply_overflow_rejects_before_committing_a_large_before_image() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = success(facade.open(
        &frame(
            "open",
            Json::Null,
            memory_args(
                schema("documents", "body"),
                "06060606060606060606060606060606",
            ),
        ),
        vec![],
    ));
    let original = object([("body", tagged("string", text(&"a".repeat(600_000))))]);
    let row = success(facade.call(&frame(
        "insert",
        owner.clone(),
        object([
            ("table", text("documents")),
            ("fields", original),
            ("proposed_id", text("large")),
        ]),
    )));
    let changes = Json::Array(vec![object([
        ("op", text("replace")),
        ("table", text("documents")),
        ("id", text("large")),
        (
            "fields",
            object([("body", tagged("string", text(&"b".repeat(600_000))))]),
        ),
    ])]);
    failure(
        facade.call(&frame(
            "batch",
            owner.clone(),
            object([("writes", changes)]),
        )),
        "LimitExceeded",
    );
    assert_eq!(
        success(facade.call(&frame(
            "fetch",
            owner.clone(),
            object([("query", query("documents"))])
        ))),
        Json::Array(vec![row])
    );
    success(facade.call(&frame("close", owner, object([]))));
}

fn keyed_batch_budget_case(mode: &str) {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let definition_schema = Json::parse(r#"{"version":"1","tables":{"documents":{"columns":{"a":{"kind":"String","optional":false},"b":{"kind":"String","optional":false},"c":{"kind":"String","optional":false}},"relations":{}}}}"#).unwrap();
    let owner = success(facade.open(
        &frame(
            "open",
            Json::Null,
            memory_args(definition_schema, "09090909090909090909090909090909"),
        ),
        vec![],
    ));
    let fields = |value: &str| {
        object([
            ("a", tagged("string", text(value))),
            ("b", tagged("string", text(value))),
            ("c", tagged("string", text(value))),
        ])
    };
    let row = |id: &str, value: &str| object([("id", text(id)), ("fields", fields(value))]);
    let inserts = Json::Array(
        (0..128)
            .map(|i| {
                object([
                    ("op", text("insert")),
                    ("table", text("documents")),
                    ("fields", fields("old")),
                    ("proposed_id", text(&format!("key:{i}"))),
                ])
            })
            .collect(),
    );
    success(facade.call(&frame(
        "batch",
        owner.clone(),
        object([("writes", inserts)]),
    )));
    let request = if mode == "scope_batch" {
        let request = success(facade.call(&frame("request", owner.clone(), object([]))));
        assert_eq!(
            success(facade.call(&frame(
                "begin",
                owner.clone(),
                object([
                    ("request", request.clone()),
                    ("definition", definition()),
                    (
                        "value",
                        object([
                            ("kind", text("object")),
                            (
                                "value",
                                object([("title", tagged("string", text("budget")))])
                            )
                        ])
                    ),
                    (
                        "context",
                        object([
                            ("identity", object([("kind", text("null"))])),
                            ("now_ms", text("1000"))
                        ])
                    )
                ])
            ))),
            Json::Bool(true)
        );
        Some(request)
    } else {
        None
    };
    if mode == "entity_batch" {
        success(facade.call(&frame(
            "entity_configure",
            owner.clone(),
            object([(
                "config",
                object([
                    ("scope", text("scope")),
                    ("tables", Json::Array(vec![text("documents")])),
                ]),
            )]),
        )));
    }
    let writes = Json::Array(
        (0..128)
            .map(|i| {
                object([
                    ("op", text("replace")),
                    ("table", text("documents")),
                    ("id", text(&format!("key:{i}"))),
                    ("fields", fields("new")),
                ])
            })
            .collect(),
    );
    let arguments = |writes: Json| {
        let mut args = object([("writes", writes)]);
        let Json::Object(map) = &mut args else {
            unreachable!()
        };
        if let Some(request) = &request {
            map.insert("request".into(), request.clone());
        }
        if mode == "entity_batch" {
            map.insert(
                "context".into(),
                object([
                    ("identity", object([("kind", text("null"))])),
                    ("now_ms", text("1000")),
                ]),
            );
        }
        args
    };
    let expected = Json::Array(
        (0..128)
            .map(|i| {
                let id = format!("key:{i}");
                object([
                    ("table", text("documents")),
                    ("id", text(&id)),
                    ("before", row(&id, "old")),
                    ("after", row(&id, "new")),
                ])
            })
            .collect(),
    );
    assert!(expected.canonical().unwrap().len() < 100_000);
    assert_eq!(
        success(facade.call(&frame(mode, owner.clone(), arguments(writes)))),
        expected
    );
    let repeated = Json::Array(
        (0..128)
            .map(|i| {
                object([
                    ("op", text("replace")),
                    ("table", text("documents")),
                    ("id", text("key:0")),
                    ("fields", fields(&format!("v{i}"))),
                ])
            })
            .collect(),
    );
    let expected = Json::Array(
        (0..128)
            .map(|i| {
                object([
                    ("table", text("documents")),
                    ("id", text("key:0")),
                    (
                        "before",
                        row(
                            "key:0",
                            &if i == 0 {
                                "new".into()
                            } else {
                                format!("v{}", i - 1)
                            },
                        ),
                    ),
                    ("after", row("key:0", &format!("v{i}"))),
                ])
            })
            .collect(),
    );
    assert_eq!(
        success(facade.call(&frame(mode, owner.clone(), arguments(repeated)))),
        expected
    );
    if mode != "entity_batch" {
        let writes = Json::Array(vec![
            object([
                ("op", text("remove")),
                ("table", text("documents")),
                ("id", text("key:0")),
            ]),
            object([
                ("op", text("remove")),
                ("table", text("documents")),
                ("id", text("key:0")),
            ]),
            object([
                ("op", text("insert")),
                ("table", text("documents")),
                ("fields", fields("again")),
                ("proposed_id", text("key:0")),
            ]),
            object([
                ("op", text("replace")),
                ("table", text("documents")),
                ("id", text("key:0")),
                ("fields", fields("last")),
            ]),
        ]);
        let change = |before, after| {
            object([
                ("table", text("documents")),
                ("id", text("key:0")),
                ("before", before),
                ("after", after),
            ])
        };
        assert_eq!(
            success(facade.call(&frame(mode, owner.clone(), arguments(writes)))),
            Json::Array(vec![
                change(row("key:0", "v127"), Json::Null),
                change(Json::Null, row("key:0", "again")),
                change(row("key:0", "again"), row("key:0", "last")),
            ])
        );
    }
    if let Some(request) = request {
        success(facade.call(&frame(
            "abort",
            owner.clone(),
            object([("request", request)]),
        )));
    }
    success(facade.call(&frame("close", owner, object([]))));
}

#[test]
fn g11_keyed_batch_budget_excludes_unrelated_rows() {
    keyed_batch_budget_case("batch");
}

#[test]
fn g11_keyed_scope_budget_excludes_unrelated_rows() {
    keyed_batch_budget_case("scope_batch");
}

#[test]
fn g11_keyed_entity_budget_excludes_unrelated_rows() {
    keyed_batch_budget_case("entity_batch");
}

#[test]
fn g11_unknown_allocated_before_image_stays_bounded_and_counter_unchanged() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = success(facade.open(
        &frame(
            "open",
            Json::Null,
            memory_args(
                schema("documents", "body"),
                "10101010101010101010101010101010",
            ),
        ),
        vec![],
    ));
    let fields = |value: &str| object([("body", tagged("string", text(value)))]);
    let writes = Json::Array(vec![
        object([
            ("op", text("insert")),
            ("table", text("documents")),
            ("fields", fields(&"a".repeat(600_000))),
            ("proposed_id", Json::Null),
        ]),
        object([
            ("op", text("replace")),
            ("table", text("documents")),
            ("id", text("local:1")),
            ("fields", fields("small")),
        ]),
    ]);
    failure(
        facade.call(&frame("batch", owner.clone(), object([("writes", writes)]))),
        "LimitExceeded",
    );
    assert_eq!(
        success(facade.call(&frame(
            "fetch",
            owner.clone(),
            object([("query", query("documents"))])
        ))),
        Json::Array(vec![])
    );
    let row = success(facade.call(&frame(
        "insert",
        owner.clone(),
        object([
            ("table", text("documents")),
            ("fields", fields("small")),
            ("proposed_id", Json::Null),
        ]),
    )));
    let Json::Object(row) = row else {
        unreachable!()
    };
    assert_eq!(row["id"], text("local:1"));
    success(facade.call(&frame("close", owner, object([]))));
}

#[test]
fn g11_fitting_large_keyed_batch_does_not_accumulate_other_row_images() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = success(facade.open(
        &frame(
            "open",
            Json::Null,
            memory_args(
                schema("documents", "body"),
                "12121212121212121212121212121212",
            ),
        ),
        vec![],
    ));
    let fields = |value: &str| object([("body", tagged("string", text(&value.repeat(30_000))))]);
    let row = |id: &str, value: &str| object([("id", text(id)), ("fields", fields(value))]);
    let inserts = Json::Array(
        (0..10)
            .map(|i| {
                object([
                    ("op", text("insert")),
                    ("table", text("documents")),
                    ("fields", fields("a")),
                    ("proposed_id", text(&format!("doc:{i}"))),
                ])
            })
            .collect(),
    );
    success(facade.call(&frame(
        "batch",
        owner.clone(),
        object([("writes", inserts)]),
    )));
    let writes = Json::Array(
        (0..10)
            .map(|i| {
                object([
                    ("op", text("replace")),
                    ("table", text("documents")),
                    ("id", text(&format!("doc:{i}"))),
                    ("fields", fields("b")),
                ])
            })
            .collect(),
    );
    let expected = Json::Array(
        (0..10)
            .map(|i| {
                let id = format!("doc:{i}");
                object([
                    ("table", text("documents")),
                    ("id", text(&id)),
                    ("before", row(&id, "a")),
                    ("after", row(&id, "b")),
                ])
            })
            .collect(),
    );
    assert!(expected.canonical().unwrap().len() < 610_000);
    assert_eq!(
        success(facade.call(&frame("batch", owner.clone(), object([("writes", writes)])))),
        expected
    );
    success(facade.call(&frame("close", owner, object([]))));
}

#[test]
fn g11_byte_exact_terminal_prefix_makes_progress_without_dropping_events() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = success(facade.open(
        &frame(
            "open",
            Json::Null,
            memory_args(
                schema("documents", "body"),
                "11111111111111111111111111111111",
            ),
        ),
        vec![],
    ));
    let subscription = success(facade.call(&frame(
        "observe",
        owner.clone(),
        object([
            ("query", query("documents")),
            ("mode", text("membership")),
            ("columns", Json::Array(vec![])),
        ]),
    )));
    let event = |rows: Json| {
        object([
            ("kind", text("data")),
            ("subscription", subscription.clone()),
            ("value", object([("kind", text("rows")), ("rows", rows)])),
        ])
    };
    let row = |body: &str| {
        object([
            ("id", text("s")),
            ("fields", object([("body", tagged("string", text(body)))])),
        ])
    };
    let complete = |body: &str, has_more| {
        object([
            ("abi", text("1")),
            ("ok", Json::Bool(true)),
            (
                "value",
                object([
                    (
                        "events",
                        Json::Array(vec![
                            event(Json::Array(vec![])),
                            event(Json::Array(vec![row(body)])),
                        ]),
                    ),
                    ("has_more", Json::Bool(has_more)),
                ]),
            ),
        ])
    };
    let body = "x".repeat(1_048_576 - complete("", true).canonical().unwrap().len());
    assert_eq!(complete(&body, true).canonical().unwrap().len(), 1_048_576);
    assert_eq!(
        complete(&body, false).canonical(),
        Err(hoyadb_core::command::JsonError::LimitExceeded)
    );
    success(facade.call(&frame(
        "insert",
        owner.clone(),
        object([
            ("table", text("documents")),
            ("fields", object([("body", tagged("string", text(&body)))])),
            ("proposed_id", text("s")),
        ]),
    )));
    let first = success(facade.call(&frame(
        "drain",
        owner.clone(),
        object([("now_ms", text("0"))]),
    )));
    assert_eq!(
        first,
        object([
            ("events", Json::Array(vec![event(Json::Array(vec![]))])),
            ("has_more", Json::Bool(true))
        ])
    );
    let second = success(facade.call(&frame(
        "drain",
        owner.clone(),
        object([("now_ms", text("0"))]),
    )));
    assert_eq!(
        second,
        object([
            (
                "events",
                Json::Array(vec![event(Json::Array(vec![row(&body)]))])
            ),
            ("has_more", Json::Bool(false))
        ])
    );
    assert_eq!(
        success(facade.call(&frame(
            "drain",
            owner.clone(),
            object([("now_ms", text("0"))])
        ))),
        object([
            ("events", Json::Array(vec![])),
            ("has_more", Json::Bool(false))
        ])
    );
    success(facade.call(&frame("close", owner, object([]))));
}

/// SPEC-0036 / TC-0040 supersede the former "remains pending until cancellation"
/// behavior: the undeliverable snapshot terminates only its subscription.
#[test]
fn g11_undeliverable_single_snapshot_terminates_only_its_subscription() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = success(facade.open(
        &frame(
            "open",
            Json::Null,
            memory_args(
                schema("documents", "body"),
                "13131313131313131313131313131313",
            ),
        ),
        vec![],
    ));
    let subscription = success(facade.call(&frame(
        "observe",
        owner.clone(),
        object([
            ("query", query("documents")),
            ("mode", text("membership")),
            ("columns", Json::Array(vec![])),
        ]),
    )));
    for id in ["a", "b"] {
        success(facade.call(&frame(
            "insert",
            owner.clone(),
            object([
                ("table", text("documents")),
                (
                    "fields",
                    object([("body", tagged("string", text(&"x".repeat(600_000))))]),
                ),
                ("proposed_id", text(id)),
            ]),
        )));
    }
    let first = success(facade.call(&frame(
        "drain",
        owner.clone(),
        object([("now_ms", text("0"))]),
    )));
    let Json::Object(first) = first else {
        unreachable!()
    };
    assert_eq!(first["has_more"], Json::Bool(true));
    let Json::Array(events) = &first["events"] else {
        unreachable!()
    };
    assert_eq!(events.len(), 2);
    assert_eq!(
        success(facade.call(&frame(
            "drain",
            owner.clone(),
            object([("now_ms", text("0"))]),
        ))),
        object([
            (
                "events",
                Json::Array(vec![object([
                    ("kind", text("error")),
                    ("subscription", subscription.clone()),
                    ("value", object([("kind", text("LimitExceeded"))])),
                ])])
            ),
            ("has_more", Json::Bool(false))
        ])
    );
    assert_eq!(
        success(facade.call(&frame(
            "unsubscribe",
            owner.clone(),
            object([("subscription", subscription)])
        ))),
        Json::Bool(false)
    );
    assert_eq!(
        success(facade.call(&frame(
            "drain",
            owner.clone(),
            object([("now_ms", text("0"))])
        ))),
        object([
            ("events", Json::Array(vec![])),
            ("has_more", Json::Bool(false))
        ])
    );
    success(facade.call(&frame("close", owner, object([]))));
}

#[test]
fn g02_g20_retained_memory_reopen_runs_all_seven_migration_steps_and_preserves_failure() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let store = success(facade.call(&frame("memory_store", Json::Null, object([]))));
    let args = |definition: Json, migrations: Json| {
        let mut args = memory_args(definition, "07070707070707070707070707070707");
        let Json::Object(ref mut f) = args else {
            unreachable!()
        };
        f.insert(
            "engine".into(),
            object([("kind", text("memory")), ("store", store.clone())]),
        );
        f.insert("migrations".into(), migrations);
        args
    };
    let v1 = schema("notes", "title");
    let owner = success(facade.open(
        &frame("open", Json::Null, args(v1.clone(), Json::Array(vec![]))),
        vec![],
    ));
    failure(
        facade.open(
            &frame("open", Json::Null, args(v1.clone(), Json::Array(vec![]))),
            vec![],
        ),
        "Busy",
    );
    failure(
        facade.call(&frame(
            "memory_drop",
            Json::Null,
            object([("store", store.clone())]),
        )),
        "Busy",
    );
    success(facade.call(&frame(
        "insert",
        owner.clone(),
        object([
            ("table", text("notes")),
            (
                "fields",
                object([("title", tagged("string", text("kept")))]),
            ),
            ("proposed_id", text("n1")),
        ]),
    )));
    success(facade.call(&frame("close", owner.clone(), object([]))));
    failure(
        facade.call(&frame("schema", owner, object([]))),
        "InvalidHandle",
    );
    let v2=Json::parse(r#"{"version":"2","tables":{"notes":{"columns":{"name":{"kind":"String","optional":false},"score":{"kind":"Integer","optional":false}},"relations":{}}}}"#).unwrap();
    let steps=Json::parse(r#"[
      {"op":"RenameColumn","table":"notes","from":"title","to":"name"},
      {"op":"AddColumn","table":"notes","name":"score","column":{"kind":"Integer","optional":false},"default":{"kind":"some","value":{"kind":"integer","value":"7"}}},
      {"op":"Backfill","table":"notes","column":"score","value":{"kind":"integer","value":"9"}},
      {"op":"AddTable","table":"scratch","definition":{"columns":{"label":{"kind":"String","optional":true}},"relations":{}}},
      {"op":"DropTable","table":"scratch"},
      {"op":"AddColumn","table":"notes","name":"unused","column":{"kind":"String","optional":true},"default":{"kind":"none"}},
      {"op":"DropColumn","table":"notes","name":"unused"},
      {"op":"SetRelations","table":"notes","relations":{}}
    ]"#).unwrap();
    let migration = |steps| {
        Json::Array(vec![object([
            ("from_version", text("1")),
            ("to", v2.clone()),
            ("steps", steps),
        ])])
    };
    let mut bad_steps = steps.clone();
    let Json::Array(ref mut bad) = bad_steps else {
        unreachable!()
    };
    bad[2] = object([
        ("op", text("Backfill")),
        ("table", text("notes")),
        ("column", text("score")),
        ("value", tagged("string", text("wrong type"))),
    ]);
    failure(
        facade.open(
            &frame("open", Json::Null, args(v2.clone(), migration(bad_steps))),
            vec![],
        ),
        "MigrationFailure",
    );
    let owner = success(facade.open(
        &frame("open", Json::Null, args(v1.clone(), Json::Array(vec![]))),
        vec![],
    ));
    assert_eq!(
        success(facade.call(&frame(
            "fetch",
            owner.clone(),
            object([("query", query("notes"))])
        ))),
        Json::Array(vec![object([
            ("id", text("n1")),
            (
                "fields",
                object([("title", tagged("string", text("kept")))])
            )
        ])])
    );
    success(facade.call(&frame("close", owner, object([]))));
    let owner = success(facade.open(
        &frame("open", Json::Null, args(v2.clone(), migration(steps))),
        vec![],
    ));
    assert_eq!(
        success(facade.call(&frame("schema", owner.clone(), object([])))),
        v2
    );
    assert_eq!(
        success(facade.call(&frame(
            "fetch",
            owner.clone(),
            object([("query", query("notes"))])
        ))),
        Json::Array(vec![object([
            ("id", text("n1")),
            (
                "fields",
                object([
                    ("name", tagged("string", text("kept"))),
                    ("score", tagged("integer", text("9")))
                ])
            )
        ])])
    );
    success(facade.call(&frame("close", owner, object([]))));
    success(facade.call(&frame(
        "memory_drop",
        Json::Null,
        object([("store", store.clone())]),
    )));
    failure(
        facade.open(
            &frame("open", Json::Null, args(v1, Json::Array(vec![]))),
            vec![],
        ),
        "InvalidHandle",
    );
}

fn definition() -> Json {
    object([
        ("name", text("create-note-v1")),
        (
            "arguments",
            object([(
                "title",
                object([("kind", text("String")), ("optional", Json::Bool(false))]),
            )]),
        ),
        ("reference_paths", Json::Array(vec![])),
    ])
}

#[test]
fn g06_g18_command_scope_abort_commit_and_durable_retry_status() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = success(facade.open(
        &frame(
            "open",
            Json::Null,
            memory_args(schema("notes", "title"), "08080808080808080808080808080808"),
        ),
        vec![],
    ));
    let context = object([
        ("identity", tagged("string", text("actor"))),
        ("now_ms", text("100")),
    ]);
    let value = tagged("object", object([("title", tagged("string", text("one")))]));
    let first = success(facade.call(&frame("request", owner.clone(), object([]))));
    let second = success(facade.call(&frame("request", owner.clone(), object([]))));
    let begin = |request: Json| {
        object([
            ("request", request),
            ("definition", definition()),
            ("value", value.clone()),
            ("context", context.clone()),
        ])
    };
    assert_eq!(
        success(facade.call(&frame("begin", owner.clone(), begin(first.clone())))),
        Json::Bool(true)
    );
    assert_eq!(
        success(facade.call(&frame("begin", owner.clone(), begin(second.clone())))),
        Json::Bool(false)
    );
    assert_eq!(
        success(facade.call(&frame(
            "scope_args",
            owner.clone(),
            object([("request", first.clone())])
        ))),
        value
    );
    assert_eq!(
        success(facade.call(&frame(
            "scope_context",
            owner.clone(),
            object([("request", first.clone())])
        ))),
        context
    );
    let insert = |request: Json| {
        object([
            ("request", request),
            ("table", text("notes")),
            ("fields", object([("title", tagged("string", text("one")))])),
            ("proposed_id", Json::Null),
        ])
    };
    let staged = success(facade.call(&frame("scope_insert", owner.clone(), insert(first.clone()))));
    assert_eq!(
        success(facade.call(&frame(
            "fetch",
            owner.clone(),
            object([("query", query("notes"))])
        ))),
        Json::Array(vec![])
    );
    success(facade.call(&frame(
        "abort",
        owner.clone(),
        object([("request", first.clone())]),
    )));
    failure(
        facade.call(&frame(
            "scope_args",
            owner.clone(),
            object([("request", first)]),
        )),
        "InvalidHandle",
    );
    assert_eq!(
        success(facade.call(&frame("begin", owner.clone(), begin(second.clone())))),
        Json::Bool(true)
    );
    let committed = success(facade.call(&frame(
        "scope_insert",
        owner.clone(),
        insert(second.clone()),
    )));
    assert_eq!(committed, staged, "aborted allocation was never persisted");
    let result = success(facade.call(&frame(
        "finish",
        owner.clone(),
        object([
            ("request", second),
            ("result", tagged("integer", text("9223372036854775807"))),
        ]),
    )));
    let Json::Object(result) = result else {
        unreachable!()
    };
    assert_eq!(
        result["result"],
        tagged("integer", text("9223372036854775807"))
    );
    assert_eq!(
        success(facade.call(&frame(
            "fetch",
            owner.clone(),
            object([("query", query("notes"))])
        ))),
        Json::Array(vec![committed])
    );
    let status = success(facade.call(&frame("queue_status", owner.clone(), object([]))));
    let Json::Object(status) = status else {
        unreachable!()
    };
    assert_eq!(status["pending"], text("1"));
    let Json::Array(entries) = &status["commands"] else {
        unreachable!()
    };
    let Json::Object(entry) = &entries[0] else {
        unreachable!()
    };
    assert_eq!(entry["kind"], text("command"));
    assert_eq!(entry["table"], Json::Null);
    assert_eq!(entry["record_id"], Json::Null);
    assert_eq!(entry["attempts"], text("0"));
    assert_eq!(entry["next_due_ms"], Json::Null);
    let batch = success(facade.call(&frame(
        "prepare_push",
        owner.clone(),
        object([("now_ms", text("100")), ("jitter_basis_points", text("0"))]),
    )));
    let Json::Object(batch) = batch else {
        unreachable!()
    };
    let Json::Array(envelopes) = &batch["envelopes"] else {
        unreachable!()
    };
    assert_eq!(envelopes.len(), 1);
    let Json::String(envelope) = &envelopes[0] else {
        unreachable!()
    };
    let Json::Object(envelope) = Json::parse(envelope).unwrap() else {
        unreachable!()
    };
    assert_eq!(envelope["idempotencyKey"], result["key"]);
    let status = success(facade.call(&frame("queue_status", owner.clone(), object([]))));
    let Json::Object(status) = status else {
        unreachable!()
    };
    let Json::Array(entries) = &status["commands"] else {
        unreachable!()
    };
    let Json::Object(entry) = &entries[0] else {
        unreachable!()
    };
    assert_eq!(entry["attempts"], text("1"));
    assert_eq!(entry["next_due_ms"], text("1100"));
    success(facade.call(&frame(
        "receive",
        owner.clone(),
        object([("key", result["key"].clone()), ("outcome", Json::Null)]),
    )));
    let status = success(facade.call(&frame("queue_status", owner.clone(), object([]))));
    let Json::Object(status) = status else {
        unreachable!()
    };
    assert_eq!(status["awaiting_outcome"], text("1"));
    success(facade.call(&frame("close", owner, object([]))));
}

#[test]
fn g05_g10_observation_modes_buffered_cancellation_and_unsigned_drain_clock() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = success(facade.open(
        &frame(
            "open",
            Json::Null,
            memory_args(schema("notes", "title"), "09090909090909090909090909090909"),
        ),
        vec![],
    ));
    let membership = success(facade.call(&frame(
        "observe",
        owner.clone(),
        object([
            ("query", query("notes")),
            ("mode", text("membership")),
            ("columns", Json::Array(vec![])),
        ]),
    )));
    let columns = success(facade.call(&frame(
        "observe",
        owner.clone(),
        object([
            ("query", query("notes")),
            ("mode", text("columns")),
            ("columns", Json::Array(vec![text("title")])),
        ]),
    )));
    let count = success(facade.call(&frame(
        "observe",
        owner.clone(),
        object([
            ("query", query("notes")),
            ("mode", text("count")),
            ("columns", Json::Array(vec![])),
        ]),
    )));
    let initial = success(facade.call(&frame(
        "drain",
        owner.clone(),
        object([("now_ms", text("0"))]),
    )));
    let Json::Object(initial) = initial else {
        unreachable!()
    };
    let Json::Array(initial) = &initial["events"] else {
        unreachable!()
    };
    assert_eq!(initial.len(), 3);
    success(facade.call(&frame(
        "insert",
        owner.clone(),
        object([
            ("table", text("notes")),
            (
                "fields",
                object([("title", tagged("string", text("first")))]),
            ),
            ("proposed_id", text("n")),
        ]),
    )));
    assert_eq!(
        success(facade.call(&frame(
            "unsubscribe",
            owner.clone(),
            object([("subscription", membership.clone())])
        ))),
        Json::Bool(true)
    );
    let drain = success(facade.call(&frame(
        "drain",
        owner.clone(),
        object([("now_ms", text("18446744073709551615"))]),
    )));
    let Json::Object(drain) = drain else {
        unreachable!()
    };
    assert_eq!(drain["has_more"], Json::Bool(false));
    let Json::Array(events) = &drain["events"] else {
        unreachable!()
    };
    assert!(!events.is_empty());
    let mut saw_columns = false;
    let mut counts = Vec::new();
    for event in events {
        let Json::Object(event) = event else {
            unreachable!()
        };
        assert_ne!(event["subscription"], membership);
        assert_eq!(event["kind"], text("data"));
        let Json::Object(value) = &event["value"] else {
            unreachable!()
        };
        if event["subscription"] == columns {
            assert_eq!(value["kind"], text("rows"));
            assert!(value.contains_key("rows"));
            saw_columns = true;
        }
        if event["subscription"] == count {
            assert_eq!(value["kind"], text("count"));
            counts.push(value["count"].clone());
        }
    }
    assert!(saw_columns);
    assert_eq!(counts, vec![text("1")]);
    assert_eq!(
        success(facade.call(&frame(
            "unsubscribe_sync",
            owner.clone(),
            object([("subscription", count.clone())])
        ))),
        Json::Bool(false),
        "wrong subscription kind cannot cancel"
    );
    assert_eq!(
        success(facade.call(&frame(
            "unsubscribe",
            owner.clone(),
            object([("subscription", count)])
        ))),
        Json::Bool(true)
    );
    let diagnostics = success(facade.call(&frame("diagnostics", owner.clone(), object([]))));
    let Json::Object(diagnostics) = diagnostics else {
        unreachable!()
    };
    assert_eq!(diagnostics.len(), 10);
    assert!(
        diagnostics
            .values()
            .all(|v| matches!(v, Json::String(_) | Json::Bool(_)))
    );
    success(facade.call(&frame("close", owner, object([]))));
}

#[test]
fn g01_g04_g07_general_relations_reject_contradictory_target_and_manage_join_rows() {
    let _owners = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    let mut facade = Facade::new();
    let schema=Json::parse(r#"{"version":"1","tables":{
      "authors":{"columns":{"name":{"kind":"String","optional":false}},"relations":{"books":{"kind":"HasMany","table":"books","foreign_key":"author_id"},"tags":{"kind":"ManyToMany","table":"tags","through":"author_tags","source_relation":"author","target_relation":"tag"}}},
      "books":{"columns":{"title":{"kind":"String","optional":false}},"relations":{"author":{"kind":"ToOne","table":"authors","foreign_key":"author_id","optional":false}}},
      "tags":{"columns":{"label":{"kind":"String","optional":false}},"relations":{}},
      "author_tags":{"columns":{"rank":{"kind":"Integer","optional":false}},"relations":{"author":{"kind":"ToOne","table":"authors","foreign_key":"author_id","optional":false},"tag":{"kind":"ToOne","table":"tags","foreign_key":"tag_id","optional":false}}}
    }}"#).unwrap();
    let owner = success(facade.open(
        &frame(
            "open",
            Json::Null,
            memory_args(schema, "11111111111111111111111111111111"),
        ),
        vec![],
    ));
    let mut insert = |table: &str, id: &str, fields: Json| {
        success(facade.call(&frame(
            "insert",
            owner.clone(),
            object([
                ("table", text(table)),
                ("fields", fields),
                ("proposed_id", text(id)),
            ]),
        )))
    };
    let author = insert(
        "authors",
        "a",
        object([("name", tagged("string", text("Ada")))]),
    );
    let book = insert(
        "books",
        "b",
        object([
            ("title", tagged("string", text("Notes"))),
            ("author_id", tagged("reference", text("a"))),
        ]),
    );
    let tag = insert(
        "tags",
        "t",
        object([("label", tagged("string", text("math")))]),
    );
    let related = |table: &str, source: &str, id: &str, relation: &str| {
        let mut q = query(table);
        let Json::Object(ref mut f) = q else {
            unreachable!()
        };
        f.insert(
            "source".into(),
            object([
                ("table", text(source)),
                ("id", text(id)),
                ("relation", text(relation)),
            ]),
        );
        q
    };
    for (q, row) in [
        (related("authors", "books", "b", "author"), author),
        (related("books", "authors", "a", "books"), book),
    ] {
        assert_eq!(
            success(facade.call(&frame("fetch", owner.clone(), object([("query", q)])))),
            Json::Array(vec![row])
        );
    }
    let linked = success(facade.call(&frame(
        "link",
        owner.clone(),
        object([
            ("table", text("authors")),
            ("id", text("a")),
            ("relation", text("tags")),
            ("target", text("t")),
            ("fields", object([("rank", tagged("integer", text("7")))])),
            ("proposed_id", text("j")),
        ]),
    )));
    assert_eq!(
        linked,
        object([
            ("id", text("j")),
            (
                "fields",
                object([
                    ("rank", tagged("integer", text("7"))),
                    ("author_id", tagged("reference", text("a"))),
                    ("tag_id", tagged("reference", text("t")))
                ])
            )
        ])
    );
    assert_eq!(
        success(facade.call(&frame(
            "fetch",
            owner.clone(),
            object([("query", related("tags", "authors", "a", "tags"))])
        ))),
        Json::Array(vec![tag])
    );
    failure(
        facade.call(&frame(
            "fetch",
            owner.clone(),
            object([("query", related("books", "authors", "a", "tags"))]),
        )),
        "InvalidQuery",
    );
    assert_eq!(
        success(facade.call(&frame(
            "unlink",
            owner.clone(),
            object([
                ("table", text("authors")),
                ("id", text("a")),
                ("relation", text("tags")),
                ("target", text("t"))
            ])
        ))),
        text("1")
    );
    assert_eq!(
        success(facade.call(&frame(
            "fetch",
            owner.clone(),
            object([("query", query("author_tags"))])
        ))),
        Json::Array(vec![])
    );
    success(facade.call(&frame("close", owner, object([]))));
}

#[test]
fn g17_g21_wire_outcomes_survive_reopen_and_corrections_map_canonical_ids() {
    let _owners = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    let mut facade = Facade::new();
    let store = success(facade.call(&frame("memory_store", Json::Null, object([]))));
    let mut args = memory_args(schema("notes", "title"), "12121212121212121212121212121212");
    let Json::Object(ref mut f) = args else {
        unreachable!()
    };
    f.insert(
        "engine".into(),
        object([("kind", text("memory")), ("store", store.clone())]),
    );
    let owner = success(facade.open(&frame("open", Json::Null, args.clone()), vec![]));
    let request = success(facade.call(&frame("request", owner.clone(), object([]))));
    success(facade.call(&frame(
        "begin",
        owner.clone(),
        object([
            ("request", request.clone()),
            ("definition", definition()),
            (
                "value",
                tagged(
                    "object",
                    object([("title", tagged("string", text("draft")))]),
                ),
            ),
            (
                "context",
                object([
                    ("identity", object([("kind", text("null"))])),
                    ("now_ms", text("100")),
                ]),
            ),
        ]),
    )));
    let row = success(facade.call(&frame(
        "scope_insert",
        owner.clone(),
        object([
            ("request", request.clone()),
            ("table", text("notes")),
            (
                "fields",
                object([("title", tagged("string", text("draft")))]),
            ),
            ("proposed_id", Json::Null),
        ]),
    )));
    let Json::Object(row) = row else {
        unreachable!()
    };
    let result = success(facade.call(&frame(
        "finish",
        owner.clone(),
        object([
            ("request", request),
            ("result", object([("kind", text("null"))])),
        ]),
    )));
    let Json::Object(result) = result else {
        unreachable!()
    };
    success(facade.call(&frame(
        "prepare_push",
        owner.clone(),
        object([("now_ms", text("100")), ("jitter_basis_points", text("0"))]),
    )));
    let body = Json::Array(vec![
        object([
            ("idempotencyKey", result["key"].clone()),
            ("status", text("corrected")),
            (
                "result",
                object([
                    ("remote", text("server:1")),
                    ("integer", Json::Integer(i64::MAX)),
                ]),
            ),
        ]),
        object([
            ("idempotencyKey", text("unknown")),
            ("status", text("applied")),
            ("result", Json::Null),
        ]),
    ])
    .canonical()
    .unwrap();
    let reports = success(facade.call(&frame(
        "receive_wire",
        owner.clone(),
        object([("body", text(&body))]),
    )));
    let Json::Array(reports) = reports else {
        unreachable!()
    };
    assert_eq!(
        reports[0],
        object([("key", result["key"].clone()), ("ok", Json::Bool(true))])
    );
    let Json::Object(unknown) = &reports[1] else {
        unreachable!()
    };
    assert_eq!(unknown["ok"], Json::Bool(false));
    assert_eq!(unknown.len(), 3);
    success(facade.call(&frame("close", owner, object([]))));
    let owner = success(facade.open(&frame("open", Json::Null, args), vec![]));
    let pending =
        success(facade.call(&frame("pending_reconciliations", owner.clone(), object([]))));
    assert_eq!(
        pending,
        Json::Array(vec![object([
            ("key", result["key"].clone()),
            (
                "outcome",
                object([
                    ("status", text("corrected")),
                    (
                        "result",
                        tagged(
                            "object",
                            object([
                                ("remote", tagged("string", text("server:1"))),
                                ("integer", tagged("integer", text("9223372036854775807")))
                            ])
                        )
                    )
                ])
            )
        ])])
    );
    let Json::String(local_id) = &row["id"] else {
        unreachable!()
    };
    let plan = object([
        (
            "writes",
            Json::Array(vec![object([
                ("op", text("put")),
                ("table", text("notes")),
                ("id", text("server:1")),
                (
                    "fields",
                    object([("title", tagged("string", text("authoritative")))]),
                ),
            ])]),
        ),
        (
            "mappings",
            Json::Object(BTreeMap::from([(local_id.clone(), text("server:1"))])),
        ),
    ]);
    success(facade.call(&frame(
        "reconcile",
        owner.clone(),
        object([("key", result["key"].clone()), ("plan", plan)]),
    )));
    assert_eq!(
        success(facade.call(&frame(
            "fetch",
            owner.clone(),
            object([("query", query("notes"))])
        ))),
        Json::Array(vec![object([
            ("id", text("server:1")),
            (
                "fields",
                object([("title", tagged("string", text("authoritative")))])
            )
        ])])
    );
    assert_eq!(
        success(facade.call(&frame("pending_reconciliations", owner.clone(), object([])))),
        Json::Array(vec![])
    );
    success(facade.call(&frame(
        "forget",
        owner.clone(),
        object([("key", result["key"].clone())]),
    )));
    success(facade.call(&frame("close", owner, object([]))));
    success(facade.call(&frame(
        "memory_drop",
        Json::Null,
        object([("store", store)]),
    )));
}

#[test]
fn g16_original_open_preflight_is_pure_exact_and_preserves_full_wire_budget() {
    let facade = Facade::new();
    let valid = frame(
        "open",
        Json::Null,
        memory_args(
            schema("sensors", "label"),
            "13131313131313131313131313131313",
        ),
    );
    assert_eq!(success(facade.validate_open(&valid, 0)), Json::Null);
    failure(facade.validate_open(&valid, 32), "InvalidRequest");
    failure(
        facade.validate_open(&valid.replace("\"abi\":\"1\"", "\"abi\":\"2\""), 0),
        "InvalidRequest",
    );
    failure(
        facade.validate_open(&valid.replacen("{", "{\"abi\":\"1\",", 1), 0),
        "InvalidPayload",
    );
    failure(
        facade.validate_open(&valid.replace("\"String\"", "\"Unknown\""), 0),
        "InvalidRequest",
    );
    let mut native_args = memory_args(
        schema("sensors", "label"),
        "13131313131313131313131313131313",
    );
    let Json::Object(ref mut fields) = native_args else {
        unreachable!()
    };
    fields.insert(
        "engine".into(),
        object([
            ("kind", text("native")),
            (
                "directory",
                text("/a/nonexistent/path/preflight/must/not/stat"),
            ),
            ("name", text("store.sqlite")),
            ("backup_excluded", Json::Bool(true)),
        ]),
    );
    assert_eq!(
        success(facade.validate_open(&frame("open", Json::Null, native_args), 32)),
        Json::Null
    );
    let empty = frame(
        "open",
        Json::Null,
        memory_args(schema("sensors", ""), "13131313131313131313131313131313"),
    );
    let near_limit = frame(
        "open",
        Json::Null,
        memory_args(
            schema("sensors", &"x".repeat(1_048_570 - empty.len())),
            "13131313131313131313131313131313",
        ),
    );
    assert_eq!(near_limit.len(), 1_048_570);
    assert_eq!(success(facade.validate_open(&near_limit, 0)), Json::Null);
}

#[test]
fn g01_g20_opens_two_unrelated_caller_schemas_in_real_memory() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let mut handles = vec![];
    for (table, field, namespace) in [
        ("authors", "name", "01010101010101010101010101010101"),
        ("sensors", "label", "02020202020202020202020202020202"),
    ] {
        let expected = schema(table, field);
        let owner = success(facade.open(
            &frame("open", Json::Null, memory_args(expected.clone(), namespace)),
            vec![],
        ));
        assert!(
            matches!(&owner, Json::String(value) if value.parse::<u64>().is_ok_and(|id| id > 0))
        );
        assert!(!handles.contains(&owner));
        assert_eq!(
            success(facade.call(&frame(
                "schema",
                owner.clone(),
                Json::Object(BTreeMap::new())
            ))),
            expected
        );
        handles.push(owner);
    }
    for owner in handles {
        assert_eq!(
            success(facade.call(&frame("close", owner, Json::Object(BTreeMap::new())))),
            Json::Null
        );
    }
}

#[test]
fn g16_g20_memory_truthfully_reports_no_at_rest_cipher_or_backup_control() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = success(facade.open(
        &frame(
            "open",
            Json::Null,
            memory_args(
                schema("devices", "name"),
                "03030303030303030303030303030303",
            ),
        ),
        vec![],
    ));
    assert_eq!(
        success(facade.call(&frame("protection", owner, object([])))),
        object([
            ("encrypted", Json::Bool(false)),
            ("cipher", text("none")),
            ("version", text("not_applicable")),
            ("backup", text("not_applicable")),
        ])
    );
}

#[test]
fn g16_protected_native_open_rejects_wrong_keys_and_reopens_schema() {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    struct Root(std::path::PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    let root = Root(std::env::temp_dir().join(format!(
        "hoya-sdk-native-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    std::fs::create_dir(&root.0).unwrap();
    let mut args = memory_args(
        schema("instruments", "description"),
        "04040404040404040404040404040404",
    );
    let Json::Object(ref mut fields) = args else {
        unreachable!()
    };
    fields.insert(
        "engine".into(),
        object([
            ("kind", text("native")),
            (
                "directory",
                text(root.0.canonicalize().unwrap().to_str().unwrap()),
            ),
            ("name", text("store.sqlite")),
            ("backup_excluded", Json::Bool(true)),
        ]),
    );
    let request = frame("open", Json::Null, args);
    let mut facade = Facade::new();
    let owner = success(facade.open(&request, vec![0x77; 32]));
    assert_eq!(
        success(facade.call(&frame("protection", owner.clone(), object([])))),
        object([
            ("encrypted", Json::Bool(true)),
            ("cipher", text("SQLCipher")),
            ("version", text("4.19.0")),
            ("backup", text("host_managed")),
        ])
    );
    success(facade.call(&frame("close", owner, object([]))));
    let before = std::fs::read(root.0.join("store.sqlite")).unwrap();
    assert!(!before.starts_with(b"SQLite format 3\0"));
    let Json::Object(error) = Json::parse(&facade.open(&request, vec![0x55; 32])).unwrap() else {
        unreachable!()
    };
    assert_eq!(error["ok"], Json::Bool(false));
    assert_eq!(error["error"], object([("kind", text("BackendFailure"))]));
    assert_eq!(std::fs::read(root.0.join("store.sqlite")).unwrap(), before);
    let owner = success(facade.open(&request, vec![0x77; 32]));
    assert_eq!(
        success(facade.call(&frame("schema", owner.clone(), object([])))),
        schema("instruments", "description")
    );
    success(facade.call(&frame("close", owner, object([]))));
}

#[test]
fn g06_scoped_replace_remove_batch_and_explicit_inverse_paths() {
    let _owners = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    let mut facade = Facade::new();
    let owner = success(facade.open(
        &frame(
            "open",
            Json::Null,
            memory_args(schema("notes", "title"), "14141414141414141414141414141414"),
        ),
        vec![],
    ));
    let request = success(facade.call(&frame("request", owner.clone(), object([]))));
    success(facade.call(&frame(
        "begin",
        owner.clone(),
        object([
            ("request", request.clone()),
            ("definition", definition()),
            (
                "value",
                tagged(
                    "object",
                    object([("title", tagged("string", text("initial")))]),
                ),
            ),
            (
                "context",
                object([
                    ("identity", object([("kind", text("null"))])),
                    ("now_ms", text("10")),
                ]),
            ),
        ]),
    )));
    let row = success(facade.call(&frame(
        "scope_insert",
        owner.clone(),
        object([
            ("request", request.clone()),
            ("table", text("notes")),
            ("fields", object([("title", tagged("string", text("one")))])),
            ("proposed_id", Json::Null),
        ]),
    )));
    let Json::Object(row) = row else {
        unreachable!()
    };
    let id = row["id"].clone();
    let changed = success(facade.call(&frame(
        "scope_replace",
        owner.clone(),
        object([
            ("request", request.clone()),
            ("table", text("notes")),
            ("id", id.clone()),
            ("fields", object([("title", tagged("string", text("two")))])),
        ]),
    )));
    assert_eq!(
        success(facade.call(&frame(
            "scope_fetch",
            owner.clone(),
            object([("request", request.clone()), ("query", query("notes"))])
        ))),
        Json::Array(vec![changed])
    );
    assert_eq!(
        success(facade.call(&frame(
            "scope_remove",
            owner.clone(),
            object([
                ("request", request.clone()),
                ("table", text("notes")),
                ("id", id.clone())
            ])
        ))),
        Json::Bool(true)
    );
    let writes = Json::Array(vec![object([
        ("op", text("insert")),
        ("table", text("notes")),
        (
            "fields",
            object([("title", tagged("string", text("three")))]),
        ),
        ("proposed_id", text("local:inverse")),
    ])]);
    let changes = success(facade.call(&frame(
        "scope_batch",
        owner.clone(),
        object([("request", request.clone()), ("writes", writes)]),
    )));
    let Json::Array(changes) = changes else {
        unreachable!()
    };
    assert_eq!(changes.len(), 1);
    let result = success(facade.call(&frame(
        "finish",
        owner.clone(),
        object([
            ("request", request),
            ("result", object([("kind", text("null"))])),
        ]),
    )));
    let Json::Object(result) = result else {
        unreachable!()
    };
    failure(
        facade.call(&frame(
            "rollback",
            owner.clone(),
            object([("key", result["key"].clone())]),
        )),
        "Protocol",
    );
    success(facade.call(&frame(
        "cancel",
        owner.clone(),
        object([("key", result["key"].clone())]),
    )));
    assert_eq!(
        success(facade.call(&frame(
            "fetch",
            owner.clone(),
            object([("query", query("notes"))])
        ))),
        Json::Array(vec![])
    );
    success(facade.call(&frame(
        "rollback",
        owner.clone(),
        object([("key", result["key"].clone())]),
    )));
    let ticket = success(facade.call(&frame("request", owner.clone(), object([]))));
    success(facade.call(&frame(
        "cancel_request",
        owner.clone(),
        object([("request", ticket.clone())]),
    )));
    failure(
        facade.call(&frame(
            "scope_args",
            owner.clone(),
            object([("request", ticket)]),
        )),
        "InvalidHandle",
    );
    success(facade.call(&frame("close", owner, object([]))));
}

#[test]
fn g17_entity_receipts_require_the_exact_frozen_tuple() {
    let _owners = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    let mut facade = Facade::new();
    let owner = success(facade.open(
        &frame(
            "open",
            Json::Null,
            memory_args(schema("notes", "title"), "15151515151515151515151515151515"),
        ),
        vec![],
    ));
    success(facade.call(&frame(
        "entity_configure",
        owner.clone(),
        object([(
            "config",
            object([
                ("scope", text("entity-tuples")),
                ("tables", Json::Array(vec![text("notes")])),
            ]),
        )]),
    )));
    success(facade.call(&frame(
        "entity_batch",
        owner.clone(),
        object([
            (
                "writes",
                Json::Array(vec![object([
                    ("op", text("insert")),
                    ("table", text("notes")),
                    (
                        "fields",
                        object([("title", tagged("string", text("entity")))]),
                    ),
                    ("proposed_id", Json::Null),
                ])]),
            ),
            (
                "context",
                object([
                    ("identity", object([("kind", text("null"))])),
                    ("now_ms", text("100")),
                ]),
            ),
        ]),
    )));
    let push = success(facade.call(&frame(
        "prepare_push",
        owner.clone(),
        object([("now_ms", text("100")), ("jitter_basis_points", text("0"))]),
    )));
    let Json::Object(push) = push else {
        unreachable!()
    };
    let Json::Array(envelopes) = &push["envelopes"] else {
        unreachable!()
    };
    let Json::String(envelope) = &envelopes[0] else {
        unreachable!()
    };
    let Json::Object(envelope) = Json::parse(envelope).unwrap() else {
        unreachable!()
    };
    let args = |table: &str| {
        object([
            ("table", text(table)),
            ("record_id", envelope["recordId"].clone()),
            ("write_id", envelope["writeId"].clone()),
            ("outcome", Json::Null),
        ])
    };
    failure(
        facade.call(&frame("receive_entity", owner.clone(), args("wrong"))),
        "Protocol",
    );
    success(facade.call(&frame("receive_entity", owner.clone(), args("notes"))));
    let status = success(facade.call(&frame("queue_status", owner.clone(), object([]))));
    let Json::Object(status) = status else {
        unreachable!()
    };
    assert_eq!(status["awaiting_outcome"], text("1"));
    success(facade.call(&frame("close", owner, object([]))));
}
