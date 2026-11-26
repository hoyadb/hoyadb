//! TC-0040 OD01/OD02/OD05/OD06 (SPEC-0036 item 1): an observation event that
//! cannot be a sole drain item terminates only its subscription.
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

fn tagged(kind: &str, value: Json) -> Json {
    object([("kind", text(kind)), ("value", value)])
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
    assert_eq!(
        Json::parse(&output).unwrap(),
        object([
            ("abi", text("1")),
            ("ok", Json::Bool(false)),
            ("error", object([("kind", text(kind))])),
        ])
    );
}

fn string_column() -> Json {
    object([("kind", text("String")), ("optional", Json::Bool(false))])
}

fn open(facade: &mut Facade, namespace: &str) -> Json {
    let table = |field: &str| {
        object([
            ("columns", object([(field, string_column())])),
            ("relations", Json::Object(BTreeMap::new())),
        ])
    };
    let schema = object([
        ("version", text("1")),
        (
            "tables",
            object([("big", table("body")), ("small", table("label"))]),
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

fn query(table: &str, order_by_id: bool, after: Option<&str>, limit: Option<&str>) -> Json {
    let predicates = match after {
        Some(id) => vec![object([
            ("field", text("id")),
            ("operator", text("Gt")),
            ("values", Json::Array(vec![tagged("string", text(id))])),
        ])],
        None => vec![],
    };
    object([
        ("table", text(table)),
        ("predicates", Json::Array(predicates)),
        (
            "order",
            if order_by_id {
                object([("field", text("id")), ("direction", text("Asc"))])
            } else {
                Json::Null
            },
        ),
        ("limit", limit.map(text).unwrap_or(Json::Null)),
        ("source", Json::Null),
    ])
}

fn observe(facade: &mut Facade, owner: &Json, query: Json) -> Json {
    success(facade.call(&frame(
        "observe",
        owner.clone(),
        object([
            ("query", query),
            ("mode", text("membership")),
            ("columns", Json::Array(vec![])),
        ]),
    )))
}

fn insert(facade: &mut Facade, owner: &Json, table: &str, field: &str, value: &str, id: &str) {
    success(facade.call(&frame(
        "insert",
        owner.clone(),
        object([
            ("table", text(table)),
            ("fields", object([(field, tagged("string", text(value)))])),
            ("proposed_id", text(id)),
        ]),
    )));
}

fn drain(facade: &mut Facade, owner: &Json) -> (Vec<Json>, bool) {
    let Json::Object(page) = success(facade.call(&frame(
        "drain",
        owner.clone(),
        object([("now_ms", text("0"))]),
    ))) else {
        panic!("drain page must be an object");
    };
    let Json::Array(events) = page["events"].clone() else {
        panic!("events must be an array");
    };
    (events, page["has_more"] == Json::Bool(true))
}

fn drain_all(facade: &mut Facade, owner: &Json) -> Vec<Json> {
    let mut all = Vec::new();
    for _ in 0..200 {
        let (events, more) = drain(facade, owner);
        all.extend(events);
        if !more {
            return all;
        }
    }
    panic!("drain did not finish");
}

fn subscription_of(event: &Json) -> Json {
    let Json::Object(event) = event else {
        panic!("event must be an object")
    };
    event["subscription"].clone()
}

fn rows_len(event: &Json) -> Option<usize> {
    let Json::Object(event) = event else {
        return None;
    };
    let Json::Object(value) = &event["value"] else {
        return None;
    };
    match (&event["kind"], value.get("rows")) {
        (Json::String(kind), Some(Json::Array(rows))) if kind == "data" => Some(rows.len()),
        _ => None,
    }
}

fn terminal(subscription: &Json) -> Json {
    object([
        ("kind", text("error")),
        ("subscription", subscription.clone()),
        ("value", object([("kind", text("LimitExceeded"))])),
    ])
}

const ROW: usize = 40_000;

/// OD01 fixture: S1 membership on big, S2 membership on small, Q queue status.
/// Thirty 40 kB rows make S1 snapshots cross one frame at the 27th row.
fn od01(facade: &mut Facade) -> (Json, Json, Json, Json, Vec<Json>) {
    let owner = open(facade, "40404040404040404040404040400001");
    let s1 = observe(facade, &owner, query("big", false, None, None));
    let s2 = observe(facade, &owner, query("small", false, None, None));
    let q = success(facade.call(&frame("observe_status", owner.clone(), object([]))));
    for n in 0..30 {
        insert(
            facade,
            &owner,
            "big",
            "body",
            &"b".repeat(ROW),
            &format!("b{n:02}"),
        );
    }
    insert(facade, &owner, "small", "label", "x", "x");
    let events = drain_all(facade, &owner);
    (owner, s1, s2, q, events)
}

#[test]
fn od01_healthy_subscriptions_continue_past_an_oversized_snapshot() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let (owner, s1, s2, q, events) = od01(&mut facade);
    let s1_events: Vec<_> = events.iter().filter(|e| subscription_of(e) == s1).collect();
    let terminals: Vec<_> = s1_events.iter().filter(|e| ***e == terminal(&s1)).collect();
    assert_eq!(terminals.len(), 1, "exactly one S1 terminal event");
    let last = s1_events.last().unwrap();
    assert_eq!(**last, terminal(&s1), "terminal is S1's final event");
    let delivered: Vec<_> = s1_events.iter().filter_map(|e| rows_len(e)).collect();
    assert_eq!(
        delivered,
        (0..=26).collect::<Vec<_>>(),
        "fitting S1 snapshots delivered once in order"
    );
    let s2_rows: Vec<_> = events
        .iter()
        .filter(|e| subscription_of(e) == s2)
        .filter_map(rows_len)
        .collect();
    assert_eq!(s2_rows, vec![0, 1], "S2 initial and updated snapshots");
    assert_eq!(
        events.iter().filter(|e| subscription_of(e) == q).count(),
        1,
        "queue status initial event"
    );
    insert(&mut facade, &owner, "small", "label", "y", "y");
    insert(&mut facade, &owner, "big", "body", "later", "c");
    let later = drain_all(&mut facade, &owner);
    assert!(
        later.iter().all(|e| subscription_of(e) != s1),
        "terminated S1 stays silent"
    );
    assert_eq!(
        later
            .iter()
            .filter(|e| subscription_of(e) == s2)
            .filter_map(rows_len)
            .collect::<Vec<_>>(),
        vec![2]
    );
    success(facade.call(&frame("close", owner, object([]))));
}

#[test]
fn od02_single_terminal_event_and_deterministic_recovery() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let (owner, s1, _s2, _q, _events) = od01(&mut facade);
    for _ in 0..3 {
        let (events, more) = drain(&mut facade, &owner);
        assert!(events.is_empty() && !more);
    }
    insert(&mut facade, &owner, "big", "body", "again", "z");
    assert!(
        drain_all(&mut facade, &owner)
            .iter()
            .all(|e| subscription_of(e) != s1)
    );
    assert_eq!(
        success(facade.call(&frame(
            "unsubscribe",
            owner.clone(),
            object([("subscription", s1)])
        ))),
        Json::Bool(false)
    );
    let bounded = observe(&mut facade, &owner, query("big", true, None, Some("10")));
    let events = drain_all(&mut facade, &owner);
    let snapshots: Vec<_> = events
        .iter()
        .filter(|e| subscription_of(e) == bounded)
        .filter_map(rows_len)
        .collect();
    assert_eq!(snapshots, vec![10], "one bounded initial snapshot");
    success(facade.call(&frame("close", owner, object([]))));
}

/// One `documents`-like event for subscription `s` whose sole-item frame with
/// has_more true is exactly one MiB, so the has_more false frame exceeds by one byte.
fn exact_body(s: &Json) -> String {
    let event = |body: &str| {
        object([
            ("kind", text("data")),
            ("subscription", s.clone()),
            (
                "value",
                object([
                    ("kind", text("rows")),
                    (
                        "rows",
                        Json::Array(vec![object([
                            ("id", text("e")),
                            ("fields", object([("body", tagged("string", text(body)))])),
                        ])]),
                    ),
                ]),
            ),
        ])
    };
    let complete = |body: &str, more: bool| {
        object([
            ("abi", text("1")),
            ("ok", Json::Bool(true)),
            (
                "value",
                object([
                    ("events", Json::Array(vec![event(body)])),
                    ("has_more", Json::Bool(more)),
                ]),
            ),
        ])
    };
    let body = "e".repeat(1_048_576 - complete("", true).canonical().unwrap().len());
    assert_eq!(complete(&body, true).canonical().unwrap().len(), 1_048_576);
    assert_eq!(
        complete(&body, false).canonical(),
        Err(hoyadb_core::command::JsonError::LimitExceeded)
    );
    body
}

#[test]
fn od05a_boundary_event_is_delivered_while_another_event_remains() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = open(&mut facade, "40404040404040404040404040400005");
    let s = observe(&mut facade, &owner, query("big", false, None, None));
    let t = observe(&mut facade, &owner, query("small", false, None, None));
    drain_all(&mut facade, &owner);
    let body = exact_body(&s);
    insert(&mut facade, &owner, "big", "body", &body, "e");
    insert(&mut facade, &owner, "small", "label", "t", "t");
    let (first, more) = drain(&mut facade, &owner);
    assert_eq!(first.len(), 1);
    assert_eq!(subscription_of(&first[0]), s);
    assert_eq!(rows_len(&first[0]), Some(1));
    assert!(more, "the other subscription's event remains");
    let (second, more) = drain(&mut facade, &owner);
    assert_eq!(second.len(), 1);
    assert_eq!(subscription_of(&second[0]), t);
    assert!(!more);
    success(facade.call(&frame("close", owner, object([]))));
}

#[test]
fn od05b_boundary_event_terminates_when_it_is_the_last_event() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = open(&mut facade, "40404040404040404040404040400006");
    let s = observe(&mut facade, &owner, query("big", false, None, None));
    drain_all(&mut facade, &owner);
    let body = exact_body(&s);
    insert(&mut facade, &owner, "big", "body", &body, "e");
    let (page, more) = drain(&mut facade, &owner);
    assert_eq!(page, vec![terminal(&s)]);
    assert!(!more);
    assert_eq!(
        success(facade.call(&frame(
            "unsubscribe",
            owner.clone(),
            object([("subscription", s)])
        ))),
        Json::Bool(false)
    );
    success(facade.call(&frame("close", owner, object([]))));
}

#[test]
fn od05c_same_subscription_later_events_are_disregarded_for_sizing() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = open(&mut facade, "40404040404040404040404040400007");
    let s = observe(&mut facade, &owner, query("big", false, None, None));
    drain_all(&mut facade, &owner);
    let body = exact_body(&s);
    insert(&mut facade, &owner, "big", "body", &body, "e");
    // A later, small snapshot of the same subscription does not make has_more true.
    assert_eq!(
        success(facade.call(&frame(
            "remove",
            owner.clone(),
            object([("table", text("big")), ("id", text("e"))])
        ))),
        Json::Bool(true)
    );
    let (page, more) = drain(&mut facade, &owner);
    assert_eq!(
        page,
        vec![terminal(&s)],
        "later same-subscription event removed"
    );
    assert!(!more);
    success(facade.call(&frame("close", owner, object([]))));
}

#[test]
fn od05d_cancellation_before_drain_yields_no_terminal_event() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = open(&mut facade, "40404040404040404040404040400008");
    let s = observe(&mut facade, &owner, query("big", false, None, None));
    let t = observe(&mut facade, &owner, query("small", false, None, None));
    drain_all(&mut facade, &owner);
    for n in 0..30 {
        insert(
            &mut facade,
            &owner,
            "big",
            "body",
            &"b".repeat(ROW),
            &format!("b{n:02}"),
        );
    }
    insert(&mut facade, &owner, "small", "label", "t", "t");
    assert_eq!(
        success(facade.call(&frame(
            "unsubscribe",
            owner.clone(),
            object([("subscription", s.clone())])
        ))),
        Json::Bool(true)
    );
    let events = drain_all(&mut facade, &owner);
    assert!(events.iter().all(|e| subscription_of(e) != s));
    assert_eq!(
        events
            .iter()
            .filter(|e| subscription_of(e) == t)
            .filter_map(rows_len)
            .collect::<Vec<_>>(),
        vec![1]
    );
    success(facade.call(&frame("close", owner, object([]))));
}

#[test]
fn od06_explicit_keyset_windows_read_a_larger_than_frame_result() {
    let _owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
    let mut facade = Facade::new();
    let owner = open(&mut facade, "40404040404040404040404040400009");
    for n in 0..3000 {
        insert(
            &mut facade,
            &owner,
            "big",
            "body",
            &"k".repeat(400),
            &format!("r{n:04}"),
        );
    }
    failure(
        facade.call(&frame(
            "fetch",
            owner.clone(),
            object([("query", query("big", false, None, None))]),
        )),
        "LimitExceeded",
    );
    let mut seen = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let rows = success(facade.call(&frame(
            "fetch",
            owner.clone(),
            object([("query", query("big", true, after.as_deref(), Some("500")))]),
        )));
        let Json::Array(rows) = rows else {
            panic!("rows")
        };
        if rows.is_empty() {
            break;
        }
        for row in &rows {
            let Json::Object(row) = row else {
                panic!("row")
            };
            let Json::String(id) = &row["id"] else {
                panic!("id")
            };
            seen.push(id.clone());
        }
        after = seen.last().cloned();
    }
    let expected: Vec<_> = (0..3000).map(|n| format!("r{n:04}")).collect();
    assert_eq!(seen, expected, "every row exactly once in id order");
    success(facade.call(&frame("close", owner, object([]))));
}
