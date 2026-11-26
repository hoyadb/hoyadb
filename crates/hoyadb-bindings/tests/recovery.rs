//! SPEC-0038 / TC-0042 through the facade wire: RD status (awaiting_key) and CL04 redelivery.
//! Operations are called by wire name, so unchanged code compiles and fails by result.
use hoyadb_bindings::Facade;
use hoyadb_core::command::Json;
use std::sync::Mutex;

static OWNERS: Mutex<()> = Mutex::new(());

/// TC-0044 CP06/CP15: exact additive wire operations and truthful discarded status.
#[test]
fn tc0044_policy_discard_wire() {
    let _owners = OWNERS.lock().unwrap();
    let mut facade = Facade::default();
    let owner = open(&mut facade, "71717171717171717171717171717171");
    assert_eq!(
        success(facade.call(&frame(
            "set_conflict_policy",
            owner.clone(),
            object([(
                "policy",
                object([
                    ("default", text("stop")),
                    ("commands", object([("unknown-v1", text("discard"))]))
                ])
            )])
        ))),
        Json::Null
    );
    let key = edit(&mut facade, &owner, "kept");
    let report = success(facade.call(&frame(
        "discard",
        owner.clone(),
        object([("key", key.clone())]),
    )));
    assert_eq!(
        report,
        object([
            ("kind", text("kept_current")),
            ("discarded_keys", Json::Array(vec![key.clone()])),
            ("unmapped", Json::Array(vec![])),
            ("skipped_writes", text("0")),
        ])
    );
    let status = entries(&mut facade, &owner);
    assert_eq!(field(&status[0], "state"), Some(&text("discarded")));
    assert_eq!(field(&status[0], "rolled_back"), Some(&Json::Bool(false)));
    failure(
        facade.call(&frame(
            "rollback_resolving",
            owner.clone(),
            object([("key", key.clone())]),
        )),
        "Protocol",
    );
    failure(
        facade.call(&frame(
            "reconcile_resolving",
            owner.clone(),
            object([
                ("key", key),
                (
                    "plan",
                    object([("writes", Json::Array(vec![])), ("mappings", object([]))]),
                ),
                ("guards", Json::Array(vec![])),
            ]),
        )),
        "Protocol",
    );
    success(facade.call(&frame("close", owner, object([]))));
}

#[test]
fn tc0044_policy_wire_validation_and_resolving_reports() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = open(&mut facade, "72727272727272727272727272727272");
    success(facade.call(&frame(
        "set_conflict_policy",
        owner.clone(),
        object([(
            "policy",
            object([("default", text("discard")), ("commands", object([]))]),
        )]),
    )));
    for malformed in [
        object([("default", text("stop"))]),
        object([("default", text("force")), ("commands", object([]))]),
        object([
            ("default", text("stop")),
            ("commands", object([("edit-note-v1", text("force"))])),
        ]),
        object([
            ("default", text("stop")),
            ("commands", object([])),
            ("extra", Json::Null),
        ]),
    ] {
        failure(
            facade.call(&frame(
                "set_conflict_policy",
                owner.clone(),
                object([("policy", malformed)]),
            )),
            "InvalidRequest",
        );
    }
    failure(
        facade.call(&frame(
            "set_conflict_policy",
            owner.clone(),
            object([(
                "policy",
                object([
                    ("default", text("stop")),
                    ("commands", object([("bad", text("stop"))])),
                ]),
            )]),
        )),
        "InvalidCommand",
    );
    let key = edit(&mut facade, &owner, "optimistic");
    push(&mut facade, &owner, "100");
    success(facade.call(&frame(
        "receive",
        owner.clone(),
        object([
            ("key", key.clone()),
            (
                "outcome",
                object([("status", text("rejected")), ("reason", text("rule"))]),
            ),
        ]),
    )));
    let empty = object([("writes", Json::Array(vec![])), ("mappings", object([]))]);
    assert_eq!(
        success(facade.call(&frame(
            "reconcile_resolving",
            owner.clone(),
            object([
                ("key", key.clone()),
                ("plan", empty),
                ("guards", Json::Array(vec![]))
            ])
        ))),
        object([("kind", text("completed"))])
    );
    success(facade.call(&frame(
        "batch",
        owner.clone(),
        object([(
            "writes",
            Json::Array(vec![object([
                ("op", text("replace")),
                ("table", text("notes")),
                ("id", text("server-note")),
                ("fields", fields("direct")),
            ])]),
        )]),
    )));
    assert_eq!(
        success(facade.call(&frame(
            "rollback_resolving",
            owner.clone(),
            object([("key", key.clone())])
        ))),
        object([
            ("kind", text("kept_current")),
            ("discarded_keys", Json::Array(vec![key])),
            ("unmapped", Json::Array(vec![])),
            ("skipped_writes", text("0"))
        ])
    );
    success(facade.call(&frame("close", owner, object([]))));
}

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
        r#"{"version":"1","tables":{"notes":{"columns":{"title":{"kind":"String","optional":false}},"relations":{}}}}"#,
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
            Json::Array(vec![object([
                ("op", text("insert")),
                ("table", text("notes")),
                ("fields", fields("original")),
                ("proposed_id", text("server-note")),
            ])]),
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

fn entries(facade: &mut Facade, owner: &Json) -> Vec<Json> {
    let Json::Object(status) =
        success(facade.call(&frame("queue_status", owner.clone(), object([]))))
    else {
        panic!("status object")
    };
    let Json::Array(commands) = &status["commands"] else {
        panic!("commands array")
    };
    commands.clone()
}

fn field<'a>(entry: &'a Json, name: &str) -> Option<&'a Json> {
    let Json::Object(map) = entry else {
        panic!("entry object")
    };
    map.get(name)
}

fn push(facade: &mut Facade, owner: &Json, now: &str) -> Vec<Json> {
    let Json::Object(batch) = success(facade.call(&frame(
        "prepare_push",
        owner.clone(),
        object([("now_ms", text(now)), ("jitter_basis_points", text("0"))]),
    ))) else {
        panic!("batch")
    };
    let Json::Array(envelopes) = &batch["envelopes"] else {
        panic!("envelopes")
    };
    envelopes.clone()
}

/// RD1 through the facade: the later same-row command is withheld and its status names the
/// earliest withholding record; unchanged code sends both and has no awaiting_key field.
#[test]
fn tc0042_rd1_status_wire_names_the_withholding_record() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = open(&mut facade, "61616161616161616161616161616161");
    let a = edit(&mut facade, &owner, "a");
    let b = edit(&mut facade, &owner, "b");
    let status = entries(&mut facade, &owner);
    assert_eq!(
        field(&status[0], "awaiting_key"),
        Some(&Json::Null),
        "A waits for nobody"
    );
    assert_eq!(field(&status[1], "awaiting_key"), Some(&a), "B names A");
    assert_eq!(push(&mut facade, &owner, "101").len(), 1, "only A is sent");
    assert_eq!(field(&entries(&mut facade, &owner)[1], "key"), Some(&b));
    success(facade.call(&frame("close", owner, object([]))));
}

/// CL04 through the facade: explicit redelivery of a lost outcome by wire name.
#[test]
fn tc0042_cl04_request_redelivery_rearms_an_acknowledged_record() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = open(&mut facade, "62626262626262626262626262626262");
    let a = edit(&mut facade, &owner, "a");
    let first = push(&mut facade, &owner, "100");
    assert_eq!(first.len(), 1);
    success(facade.call(&frame(
        "receive",
        owner.clone(),
        object([("key", a.clone()), ("outcome", Json::Null)]),
    )));
    assert!(
        push(&mut facade, &owner, "100000").is_empty(),
        "receipt stops retry"
    );
    let redeliver = |key: Json, now: &str| object([("key", key), ("now_ms", text(now))]);
    assert_eq!(
        success(facade.call(&frame(
            "request_redelivery",
            owner.clone(),
            redeliver(a.clone(), "150")
        ))),
        Json::Null
    );
    let entry = entries(&mut facade, &owner)[0].clone();
    assert_eq!(field(&entry, "state"), Some(&text("sent")));
    assert_eq!(field(&entry, "attempts"), Some(&text("1")));
    assert_eq!(
        field(&entry, "next_due_ms"),
        Some(&text("150")),
        "due at the raw sample"
    );
    assert_eq!(
        success(facade.call(&frame(
            "request_redelivery",
            owner.clone(),
            redeliver(a.clone(), "151")
        ))),
        Json::Null,
        "already Sent is a no-op"
    );
    assert_eq!(
        field(&entries(&mut facade, &owner)[0], "next_due_ms"),
        Some(&text("150"))
    );
    assert_eq!(
        push(&mut facade, &owner, "150"),
        first,
        "identical frozen bytes"
    );
    failure(
        facade.call(&frame(
            "request_redelivery",
            owner.clone(),
            redeliver(text("unknown"), "150"),
        )),
        "Protocol",
    );
    failure(
        facade.call(&frame(
            "request_redelivery",
            owner.clone(),
            redeliver(a, "-1"),
        )),
        "InvalidCommand",
    );
    success(facade.call(&frame("close", owner, object([]))));
}

/// API-0012 / TC-0046 APP05: only persisted pending recovery identities cross this projection.
#[test]
fn tc0046_reconciliation_context_is_exact_and_pending_only() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = open(&mut facade, "64646464646464646464646464646464");
    let key = edit(&mut facade, &owner, "context");
    let args = |key| object([("key", key)]);
    // Not dispatched or terminal yet, and unknown keys reveal nothing.
    for target in [key.clone(), text("unknown")] {
        failure(
            facade.call(&frame(
                "reconciliation_context",
                owner.clone(),
                args(target),
            )),
            "Protocol",
        );
    }
    push(&mut facade, &owner, "100");
    success(facade.call(&frame(
        "receive",
        owner.clone(),
        object([
            ("key", key.clone()),
            (
                "outcome",
                object([
                    ("status", text("applied")),
                    ("result", object([("kind", text("null"))])),
                ]),
            ),
        ]),
    )));
    assert_eq!(
        success(facade.call(&frame(
            "reconciliation_context",
            owner.clone(),
            args(key.clone())
        ))),
        object([("kind", text("command")), ("created", object([]))])
    );
    let Json::Array(pending) =
        success(facade.call(&frame("pending_reconciliations", owner.clone(), object([]))))
    else {
        panic!("pending array")
    };
    let Json::Object(entry) = &pending[0] else {
        panic!("pending entry")
    };
    assert_eq!(
        entry.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["key", "outcome"]
    );
    success(facade.call(&frame(
        "reconcile",
        owner.clone(),
        object([
            ("key", key.clone()),
            (
                "plan",
                object([("writes", Json::Array(vec![])), ("mappings", object([]))]),
            ),
        ]),
    )));
    failure(
        facade.call(&frame("reconciliation_context", owner.clone(), args(key))),
        "Protocol",
    );
    success(facade.call(&frame("close", owner, object([]))));
}
