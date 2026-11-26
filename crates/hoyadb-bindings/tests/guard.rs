//! SPEC-0039 / TC-0043 through the facade wire. Operations are called by wire name, so
//! unchanged code compiles and fails by result.
use hoyadb_bindings::Facade;
use hoyadb_core::command::Json;
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
    assert_eq!(response["ok"], Json::Bool(true), "{output}");
    response["value"].clone()
}

#[track_caller]
fn failure(output: String, kind: &str) {
    let Json::Object(response) = Json::parse(&output).unwrap() else {
        panic!("facade must return an object");
    };
    assert_eq!(response["ok"], Json::Bool(false), "{output}");
    assert_eq!(
        response["error"],
        object([("kind", text(kind))]),
        "{output}"
    );
}

fn tagged(kind: &str, value: Json) -> Json {
    object([("kind", text(kind)), ("value", value)])
}

fn fields(title: &str) -> Json {
    object([("title", tagged("string", text(title)))])
}

fn open(facade: &mut Facade, namespace: &str) -> Json {
    let schema = Json::parse(
        r#"{"version":"1","tables":{"notes":{"columns":{"title":{"kind":"String","optional":false}},"relations":{}},"readings":{"columns":{"count":{"kind":"Integer","optional":false},"score":{"kind":"Float","optional":false},"memo":{"kind":"String","optional":true},"target":{"kind":"Reference","optional":false}},"relations":{}}}}"#,
    )
    .unwrap();
    let args = object([
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
    ]);
    let owner = success(facade.open(&frame("open", Json::Null, args), vec![]));
    success(facade.call(&frame(
        "batch",
        owner.clone(),
        object([(
            "writes",
            Json::Array(vec![
                object([
                    ("op", text("insert")),
                    ("table", text("notes")),
                    ("fields", fields("original")),
                    ("proposed_id", text("server-note")),
                ]),
                object([
                    ("op", text("insert")),
                    ("table", text("notes")),
                    ("fields", fields("other")),
                    ("proposed_id", text("other-note")),
                ]),
            ]),
        )]),
    )));
    owner
}

/// One command replacing the seeded row; returns its key.
fn edit(facade: &mut Facade, owner: &Json, title: &str) -> Json {
    let request = success(facade.call(&frame("request", owner.clone(), object([]))));
    let definition = object([
        ("name", text("edit-note-v1")),
        ("arguments", Json::Object(Default::default())),
        ("reference_paths", Json::Array(vec![])),
    ]);
    let begun = success(facade.call(&frame(
        "begin",
        owner.clone(),
        object([
            ("request", request.clone()),
            ("definition", definition),
            ("value", tagged("object", Json::Object(Default::default()))),
            (
                "context",
                object([
                    ("identity", object([("kind", text("null"))])),
                    ("now_ms", text("100")),
                ]),
            ),
        ]),
    )));
    assert_eq!(begun, Json::Bool(true));
    success(facade.call(&frame(
        "scope_replace",
        owner.clone(),
        object([
            ("request", request.clone()),
            ("table", text("notes")),
            ("id", text("server-note")),
            ("fields", fields(title)),
        ]),
    )));
    let Json::Object(result) = success(facade.call(&frame(
        "finish",
        owner.clone(),
        object([
            ("request", request),
            ("result", object([("kind", text("null"))])),
        ]),
    ))) else {
        panic!("finish result")
    };
    result["key"].clone()
}

fn note(facade: &mut Facade, owner: &Json, id: &str) -> Json {
    let Json::Array(rows) = success(facade.call(&frame(
        "fetch",
        owner.clone(),
        object([(
            "query",
            object([
                ("table", text("notes")),
                ("predicates", Json::Array(vec![])),
                ("order", Json::Null),
                ("limit", Json::Null),
                ("source", Json::Null),
            ]),
        )]),
    ))) else {
        panic!("rows")
    };
    rows.into_iter()
        .find(|row| matches!(row, Json::Object(r) if r["id"] == text(id)))
        .expect("row")
}

fn pending(facade: &mut Facade, owner: &Json) -> usize {
    let Json::Array(items) =
        success(facade.call(&frame("pending_reconciliations", owner.clone(), object([]))))
    else {
        panic!("pending")
    };
    items.len()
}

/// A command on server-note, sent and Corrected.
fn corrected(facade: &mut Facade, owner: &Json) -> Json {
    let a = edit(facade, owner, "a");
    success(facade.call(&frame(
        "prepare_push",
        owner.clone(),
        object([("now_ms", text("101")), ("jitter_basis_points", text("0"))]),
    )));
    success(facade.call(&frame(
        "receive",
        owner.clone(),
        object([
            ("key", a.clone()),
            (
                "outcome",
                object([
                    ("status", text("corrected")),
                    ("result", object([("kind", text("null"))])),
                ]),
            ),
        ]),
    )));
    a
}

fn put_other(title: &str) -> Json {
    object([
        (
            "writes",
            Json::Array(vec![object([
                ("op", text("put")),
                ("table", text("notes")),
                ("id", text("other-note")),
                ("fields", fields(title)),
            ])]),
        ),
        ("mappings", Json::Object(Default::default())),
    ])
}

/// GW01 through the facade: an unguarded outside-closure Put on `reconcile` is refused and the
/// later direct edit is kept. Unchanged code returns ok and overwrites other-note.
#[test]
fn tc0043_gw01_facade_unguarded_put_is_refused() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = open(&mut facade, "71717171717171717171717171717171");
    let a = edit(&mut facade, &owner, "a");
    success(facade.call(&frame(
        "replace",
        owner.clone(),
        object([
            ("table", text("notes")),
            ("id", text("other-note")),
            ("fields", fields("direct")),
        ]),
    )));
    success(facade.call(&frame(
        "prepare_push",
        owner.clone(),
        object([("now_ms", text("101")), ("jitter_basis_points", text("0"))]),
    )));
    success(facade.call(&frame(
        "receive",
        owner.clone(),
        object([
            ("key", a.clone()),
            (
                "outcome",
                object([
                    ("status", text("corrected")),
                    ("result", object([("kind", text("null"))])),
                ]),
            ),
        ]),
    )));
    let before = note(&mut facade, &owner, "other-note");
    failure(
        facade.call(&frame(
            "reconcile",
            owner.clone(),
            object([("key", a), ("plan", put_other("backend"))]),
        )),
        "RollbackConflict",
    );
    assert_eq!(
        note(&mut facade, &owner, "other-note"),
        before,
        "direct edit kept"
    );
    assert_eq!(pending(&mut facade, &owner), 1, "outcome still pending");
    success(facade.call(&frame("close", owner, object([]))));
}

/// GW03 through the facade: `reconcile_guarded` by wire name with an exact expected image
/// applies the cross-row Put and the confirmation atomically. Unchanged code has no such
/// operation (InvalidRequest).
#[test]
fn tc0043_gw03_facade_reconcile_guarded_applies_an_expected_cross_row_put() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = open(&mut facade, "72727272727272727272727272727272");
    let a = corrected(&mut facade, &owner);
    let guard = object([
        ("table", text("notes")),
        ("id", text("other-note")),
        ("expected", fields("other")),
    ]);
    assert_eq!(
        success(facade.call(&frame(
            "reconcile_guarded",
            owner.clone(),
            object([
                ("key", a),
                ("plan", put_other("backend")),
                ("guards", Json::Array(vec![guard])),
            ]),
        ))),
        Json::Null
    );
    assert_eq!(
        note(&mut facade, &owner, "other-note"),
        object([("id", text("other-note")), ("fields", fields("backend"))])
    );
    assert_eq!(pending(&mut facade, &owner), 0);
    success(facade.call(&frame("close", owner, object([]))));
}

fn reading(score: &str, memo: Option<Json>) -> Json {
    let mut fields = vec![
        ("count", tagged("integer", text("9223372036854775807"))),
        ("score", tagged("float", text(score))),
        ("target", tagged("reference", text("server-note"))),
    ];
    if let Some(memo) = memo {
        fields.push(("memo", memo));
    }
    Json::Object(fields.into_iter().map(|(k, v)| (k.into(), v)).collect())
}

fn guarded(facade: &mut Facade, owner: &Json, key: &Json, guards: Json) -> String {
    let plan = object([
        (
            "writes",
            Json::Array(vec![object([
                ("op", text("put")),
                ("table", text("readings")),
                ("id", text("s")),
                ("fields", reading("2.5", None)),
            ])]),
        ),
        ("mappings", Json::Object(Default::default())),
    ]);
    facade.call(&frame(
        "reconcile_guarded",
        owner.clone(),
        object([("key", key.clone()), ("plan", plan), ("guards", guards)]),
    ))
}

/// GW06/GW09 through the facade: malformed guard wire is InvalidRequest; the expectation uses
/// the lossless fields codec (integer bounds, finite float, explicit Null distinct from an
/// omitted optional field, references), compared exactly.
#[test]
fn tc0043_gw06_gw09_facade_guard_codec_is_exact() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = open(&mut facade, "73737373737373737373737373737373");
    let null = object([("kind", text("null"))]);
    success(facade.call(&frame(
        "insert",
        owner.clone(),
        object([
            ("table", text("readings")),
            ("fields", reading("1.5", Some(null.clone()))),
            ("proposed_id", text("s")),
        ]),
    )));
    let a = corrected(&mut facade, &owner);
    let guard = |expected: Json| {
        Json::Array(vec![object([
            ("table", text("readings")),
            ("id", text("s")),
            ("expected", expected),
        ])])
    };
    for malformed in [
        Json::Object(Default::default()),
        Json::Array(vec![object([
            ("table", text("readings")),
            ("id", text("s")),
        ])]),
        Json::Array(vec![object([
            ("table", text("readings")),
            ("id", text("s")),
            ("expected", text("not fields")),
            ("extra", Json::Null),
        ])]),
        guard(text("not fields")),
    ] {
        failure(
            guarded(&mut facade, &owner, &a, malformed),
            "InvalidRequest",
        );
    }
    for stale in [
        reading("1.25", Some(null.clone())),
        reading("1.5", None),
        Json::Null,
    ] {
        failure(
            guarded(&mut facade, &owner, &a, guard(stale)),
            "RollbackConflict",
        );
    }
    assert_eq!(pending(&mut facade, &owner), 1);
    assert_eq!(
        success(guarded(
            &mut facade,
            &owner,
            &a,
            guard(reading("1.5", Some(null)))
        )),
        Json::Null
    );
    assert_eq!(pending(&mut facade, &owner), 0);
    success(facade.call(&frame("close", owner, object([]))));
}
