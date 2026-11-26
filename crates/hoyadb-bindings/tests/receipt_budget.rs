//! API-0011 §1: a server result that cannot cross this binding's tagged
//! frame is refused before the durable receipt, never stored undeliverable.
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
    assert_eq!(response["ok"], Json::Bool(true), "{output}");
    response["value"].clone()
}

fn open(facade: &mut Facade, namespace: &str) -> Json {
    let column = object([("kind", text("String")), ("optional", Json::Bool(false))]);
    let schema = object([
        ("version", text("1")),
        (
            "tables",
            object([(
                "notes",
                object([
                    ("columns", object([("title", column)])),
                    ("relations", Json::Object(BTreeMap::new())),
                ]),
            )]),
        ),
    ]);
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
    success(facade.open(&frame("open", Json::Null, args), vec![]))
}

/// Commits one command and marks it Sent; returns its idempotency key.
fn sent_command(facade: &mut Facade, owner: &Json) -> Json {
    let request = success(facade.call(&frame("request", owner.clone(), object([]))));
    let definition = object([
        ("name", text("touch-v1")),
        ("arguments", Json::Object(BTreeMap::new())),
        ("reference_paths", Json::Array(vec![])),
    ]);
    success(facade.call(&frame(
        "begin",
        owner.clone(),
        object([
            ("request", request.clone()),
            ("definition", definition),
            (
                "value",
                object([
                    ("kind", text("object")),
                    ("value", Json::Object(BTreeMap::new())),
                ]),
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
    let result = success(facade.call(&frame(
        "finish",
        owner.clone(),
        object([
            ("request", request),
            ("result", object([("kind", text("null"))])),
        ]),
    )));
    success(facade.call(&frame(
        "prepare_push",
        owner.clone(),
        object([("now_ms", text("100")), ("jitter_basis_points", text("0"))]),
    )));
    let Json::Object(result) = result else {
        unreachable!()
    };
    result["key"].clone()
}

fn receive(facade: &mut Facade, owner: &Json, key: &Json, result: Json) -> Json {
    let body = Json::Array(vec![object([
        ("idempotencyKey", key.clone()),
        ("status", text("applied")),
        ("result", result),
    ])])
    .canonical()
    .unwrap();
    let reports = success(facade.call(&frame(
        "receive_wire",
        owner.clone(),
        object([("body", text(&body))]),
    )));
    let Json::Array(mut reports) = reports else {
        unreachable!()
    };
    assert_eq!(reports.len(), 1);
    reports.remove(0)
}

fn refused(key: &Json) -> Json {
    object([
        ("key", key.clone()),
        ("ok", Json::Bool(false)),
        ("error", object([("kind", text("LimitExceeded"))])),
    ])
}

/// The record stays Sent with no outcome and the pending list stays readable.
fn assert_not_received(facade: &mut Facade, owner: &Json) {
    assert_eq!(
        success(facade.call(&frame("pending_reconciliations", owner.clone(), object([])))),
        Json::Array(vec![]),
    );
    let status = success(facade.call(&frame("queue_status", owner.clone(), object([]))));
    let Json::Object(status) = status else {
        unreachable!()
    };
    let Json::Array(commands) = &status["commands"] else {
        unreachable!()
    };
    let Json::Object(entry) = &commands[0] else {
        unreachable!()
    };
    assert_eq!(entry["state"], text("sent"));
}

#[test]
fn receipt_whose_tagged_result_exceeds_wire_depth_is_refused_before_the_durable_receipt() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = open(&mut facade, "31313131313131313131313131313131");
    let key = sent_command(&mut facade, &owner);
    // Valid raw server JSON (depth 42 of 64); its tagged form is twice as deep.
    let deep = (0..40).fold(Json::Null, |inner, _| Json::Array(vec![inner]));
    assert_eq!(receive(&mut facade, &owner, &key, deep), refused(&key));
    assert_not_received(&mut facade, &owner);
    // A deliverable result for the same key is still accepted afterwards.
    assert_eq!(
        receive(&mut facade, &owner, &key, text("fine")),
        object([("key", key.clone()), ("ok", Json::Bool(true))]),
    );
    let pending =
        success(facade.call(&frame("pending_reconciliations", owner.clone(), object([]))));
    let Json::Array(pending) = pending else {
        unreachable!()
    };
    assert_eq!(pending.len(), 1);
    success(facade.call(&frame("close", owner, object([]))));
}

#[test]
fn receipt_whose_tagged_result_exceeds_wire_nodes_is_refused_before_the_durable_receipt() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = open(&mut facade, "32323232323232323232323232323232");
    let key = sent_command(&mut facade, &owner);
    // 40,000 raw integers are about 80 KiB; each becomes a three-node tag.
    let wide = Json::Array(vec![Json::Integer(1); 40_000]);
    assert_eq!(receive(&mut facade, &owner, &key, wide), refused(&key));
    assert_not_received(&mut facade, &owner);
    success(facade.call(&frame("close", owner, object([]))));
}

fn failure_kind(output: String) -> Json {
    let Json::Object(response) = Json::parse(&output).unwrap() else {
        panic!("facade must return an object");
    };
    assert_eq!(response["ok"], Json::Bool(false), "{output}");
    let Json::Object(error) = &response["error"] else {
        unreachable!()
    };
    error["kind"].clone()
}

fn page(facade: &mut Facade, owner: &Json, op: &str, after: Json) -> (Vec<Json>, bool, Json) {
    let reply = facade.call(&frame(op, owner.clone(), object([("after", after)])));
    assert!(reply.len() <= 1_048_576);
    let value = success(reply);
    let Json::Object(fields) = &value else {
        unreachable!()
    };
    let list = if op == "queue_status_page" {
        "commands"
    } else {
        "entries"
    };
    let Json::Array(entries) = &fields[list] else {
        unreachable!()
    };
    let Json::Bool(more) = fields["has_more"] else {
        unreachable!()
    };
    (entries.clone(), more, value.clone())
}

fn key_of(entry: &Json) -> Json {
    let Json::Object(entry) = entry else {
        unreachable!()
    };
    entry["key"].clone()
}

/// Individually deliverable outcomes whose sum exceeds one frame stay readable
/// through owned prefixes; the unpaged operation keeps its exact behaviour.
#[test]
fn pending_reconciliations_page_returns_ordered_prefixes_when_the_sum_exceeds_one_frame() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = open(&mut facade, "33333333333333333333333333333333");
    let mut keys = Vec::new();
    for index in 0..5 {
        let key = sent_command(&mut facade, &owner);
        let result = text(&format!("{index}").repeat(300_000));
        assert_eq!(
            receive(&mut facade, &owner, &key, result),
            object([("key", key.clone()), ("ok", Json::Bool(true))]),
        );
        keys.push(key);
    }
    assert_eq!(
        failure_kind(facade.call(&frame("pending_reconciliations", owner.clone(), object([])))),
        text("LimitExceeded"),
    );
    let op = "pending_reconciliations_page";
    let (first, more, _) = page(&mut facade, &owner, op, Json::Null);
    assert_eq!(
        first.iter().map(key_of).collect::<Vec<_>>(),
        keys[..3].to_vec()
    );
    assert!(more);
    // A caller that ignores has_more sees the same stable prefix again.
    assert_eq!(page(&mut facade, &owner, op, Json::Null).0, first);
    let (second, more, _) = page(&mut facade, &owner, op, keys[2].clone());
    assert_eq!(
        second.iter().map(key_of).collect::<Vec<_>>(),
        keys[3..].to_vec()
    );
    assert!(!more);
    let (last, more, _) = page(&mut facade, &owner, op, keys[4].clone());
    assert!(last.is_empty() && !more);
    assert_eq!(
        failure_kind(facade.call(&frame(
            op,
            owner.clone(),
            object([("after", text("not-a-pending-key"))])
        ))),
        text("Protocol"),
    );
    success(facade.call(&frame("close", owner, object([]))));
}

#[test]
fn queue_status_page_matches_the_unpaged_status_and_checks_its_cursor() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = open(&mut facade, "34343434343434343434343434343434");
    let first = sent_command(&mut facade, &owner);
    let second = sent_command(&mut facade, &owner);
    let whole = success(facade.call(&frame("queue_status", owner.clone(), object([]))));
    let Json::Object(mut expected) = whole else {
        unreachable!()
    };
    expected.insert("has_more".into(), Json::Bool(false));
    let op = "queue_status_page";
    let (commands, more, value) = page(&mut facade, &owner, op, Json::Null);
    assert_eq!(value, Json::Object(expected));
    assert_eq!(
        commands.iter().map(key_of).collect::<Vec<_>>(),
        vec![first.clone(), second.clone()]
    );
    assert!(!more);
    let (rest, more, _) = page(&mut facade, &owner, op, first);
    assert_eq!(rest.iter().map(key_of).collect::<Vec<_>>(), vec![second]);
    assert!(!more);
    assert_eq!(
        failure_kind(facade.call(&frame(
            op,
            owner.clone(),
            object([("after", text("unknown"))])
        ))),
        text("Protocol"),
    );
    success(facade.call(&frame("close", owner, object([]))));
}
