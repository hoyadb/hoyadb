//! TC-0036: actual general facade entity/pull/merge boundaries.
use hoyadb_bindings::Facade;
use hoyadb_core::command::Json;
use std::{collections::BTreeMap, sync::Mutex};
static OWNERS: Mutex<()> = Mutex::new(());
fn o<const N: usize>(v: [(&str, Json); N]) -> Json {
    Json::Object(v.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
fn t(v: impl ToString) -> Json {
    Json::String(v.to_string())
}
fn call(f: &mut Facade, owner: &Json, op: &str, args: Json) -> Json {
    Json::parse(
        &f.call(
            &o([
                ("abi", t(1)),
                ("op", t(op)),
                ("owner", owner.clone()),
                ("args", args),
            ])
            .canonical()
            .unwrap(),
        ),
    )
    .unwrap()
}
fn ok(v: Json) -> Json {
    let Json::Object(v) = v else {
        panic!("response")
    };
    assert_eq!(v["ok"], Json::Bool(true), "{v:?}");
    v["value"].clone()
}
fn fail(v: Json, kind: &str) {
    let Json::Object(v) = v else {
        panic!("response")
    };
    assert_eq!(v["ok"], Json::Bool(false));
    assert_eq!(v["error"], o([("kind", t(kind))]));
}
fn m(v: &Json) -> &BTreeMap<String, Json> {
    let Json::Object(v) = v else {
        panic!("object")
    };
    v
}
fn arr(v: &Json) -> &Vec<Json> {
    let Json::Array(v) = v else {
        panic!("array")
    };
    v
}
fn tag(v: impl ToString) -> Json {
    o([("kind", t("string")), ("value", t(v))])
}
fn fields(v: impl ToString) -> Json {
    o([("body", tag(v))])
}
fn query() -> Json {
    o([
        ("table", t("items")),
        ("predicates", Json::Array(vec![])),
        ("order", Json::Null),
        ("limit", Json::Null),
        ("source", Json::Null),
    ])
}
fn context() -> Json {
    o([("identity", o([("kind", t("null"))])), ("now_ms", t(1000))])
}

#[test]
fn a05_aliased_batch_budgets_preserved_patch_fields_before_mutation() {
    let _guard = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    let mut f = Facade::new();
    let owner = open(
        &mut f,
        o([("kind", t("memory")), ("store", Json::Null)]),
        vec![],
    );
    ok(call(
        &mut f,
        &owner,
        "insert",
        o([
            ("table", t("items")),
            ("proposed_id", t("large")),
            ("fields", fields("x".repeat(600_000))),
        ]),
    ));
    ok(call(
        &mut f,
        &owner,
        "entity_configure",
        o([(
            "config",
            o([
                ("scope", t("aliases")),
                ("tables", Json::Array(vec![t("items")])),
            ]),
        )]),
    ));
    fail(
        call(
            &mut f,
            &owner,
            "entity_batch_aliased",
            o([
                (
                    "writes",
                    Json::Array(vec![o([
                        ("op", t("patch")),
                        ("table", t("items")),
                        ("id", t("large")),
                        ("fields", o([("extra", tag("must-not-commit"))])),
                        ("references", o([])),
                    ])]),
                ),
                ("context", context()),
            ]),
        ),
        "LimitExceeded",
    );
    let rows = ok(call(&mut f, &owner, "fetch", o([("query", query())])));
    assert!(!m(&m(&arr(&rows)[0])["fields"]).contains_key("extra"));
    let status = ok(call(&mut f, &owner, "queue_status", o([])));
    assert_eq!(m(&status)["live"], t(0));
    ok(call(&mut f, &owner, "close", o([])));
}

#[test]
fn a05_alias_expansion_and_mapping_are_bounded_before_allocation() {
    let _guard = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    let mut f = Facade::new();
    let mut definition = schema();
    let Json::Object(root) = &mut definition else {
        unreachable!()
    };
    let Json::Object(tables) = root.get_mut("tables").unwrap() else {
        unreachable!()
    };
    let Json::Object(items) = tables.get_mut("items").unwrap() else {
        unreachable!()
    };
    items.insert(
        "relations".into(),
        o([(
            "parent",
            o([
                ("kind", t("ToOne")),
                ("table", t("items")),
                ("foreign_key", t("parent_id")),
                ("optional", Json::Bool(true)),
            ]),
        )]),
    );
    let owner = open_definition(
        &mut f,
        o([("kind", t("memory")), ("store", Json::Null)]),
        vec![],
        definition,
    );
    ok(call(
        &mut f,
        &owner,
        "entity_configure",
        o([(
            "config",
            o([
                ("scope", t("aliases")),
                ("tables", Json::Array(vec![t("items")])),
            ]),
        )]),
    ));
    let writes = (0..2200)
        .map(|index| {
            o([
                ("op", t("insert")),
                ("table", t("items")),
                ("fields", fields("x".repeat(120))),
                ("proposed_id", Json::Null),
                ("alias", t(format!("a{index}"))),
                (
                    "references",
                    if index == 0 {
                        o([])
                    } else {
                        o([("parent_id", t("a0"))])
                    },
                ),
            ])
        })
        .collect();
    let args = o([("writes", Json::Array(writes)), ("context", context())]);
    assert!(args.canonical().unwrap().len() < 1_048_576);
    fail(
        call(&mut f, &owner, "entity_batch_aliased", args),
        "LimitExceeded",
    );
    assert_eq!(
        ok(call(&mut f, &owner, "fetch", o([("query", query())]))),
        Json::Array(vec![])
    );
    let result = ok(call(
        &mut f,
        &owner,
        "entity_batch_aliased",
        o([
            (
                "writes",
                Json::Array(vec![o([
                    ("op", t("insert")),
                    ("table", t("items")),
                    ("fields", fields("retry")),
                    ("proposed_id", Json::Null),
                    ("alias", t("p")),
                    ("references", o([])),
                ])]),
            ),
            ("context", context()),
        ]),
    ));
    let Json::String(id) = &m(&m(&result)["aliases"])["p"] else {
        panic!("id")
    };
    assert!(id.ends_with("0000000000000001"));
    ok(call(&mut f, &owner, "close", o([])));
}

#[test]
fn a07_aliased_batch_patch_preserves_concurrent_fields_and_commits_marker() {
    let _guard = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    let mut f = Facade::new();
    let mut definition = schema();
    let Json::Object(root) = &mut definition else {
        unreachable!()
    };
    let Json::Object(tables) = root.get_mut("tables").unwrap() else {
        unreachable!()
    };
    tables.insert("local".into(), tables["items"].clone());
    let owner = open_definition(
        &mut f,
        o([("kind", t("memory")), ("store", Json::Null)]),
        vec![],
        definition,
    );
    ok(call(
        &mut f,
        &owner,
        "insert",
        o([
            ("table", t("items")),
            ("proposed_id", t("existing")),
            (
                "fields",
                o([
                    ("body", tag("before")),
                    ("extra", tag("concurrent-server-value")),
                ]),
            ),
        ]),
    ));
    ok(call(
        &mut f,
        &owner,
        "entity_configure",
        o([(
            "config",
            o([
                ("scope", t("aliases")),
                ("tables", Json::Array(vec![t("items")])),
            ]),
        )]),
    ));
    let result = ok(call(
        &mut f,
        &owner,
        "entity_batch_aliased",
        o([
            (
                "writes",
                Json::Array(vec![
                    o([
                        ("op", t("patch")),
                        ("table", t("items")),
                        ("id", t("existing")),
                        ("fields", fields("client-value")),
                        ("references", o([])),
                    ]),
                    o([
                        ("op", t("insert")),
                        ("table", t("local")),
                        ("fields", fields("complete")),
                        ("proposed_id", t("migration-marker")),
                        ("alias", Json::Null),
                        ("references", o([])),
                    ]),
                ]),
            ),
            ("context", context()),
        ]),
    ));
    let changes = arr(&m(&result)["changes"]);
    assert_eq!(changes.len(), 2);
    assert_eq!(
        m(&m(&m(&changes[0])["after"])["fields"])["extra"],
        tag("concurrent-server-value")
    );
    let pushed = ok(call(
        &mut f,
        &owner,
        "prepare_push",
        o([("now_ms", t(1000)), ("jitter_basis_points", t(0))]),
    ));
    let envelopes = arr(&m(&pushed)["envelopes"]);
    assert_eq!(envelopes.len(), 1);
    let Json::String(body) = &envelopes[0] else {
        panic!("body")
    };
    assert_eq!(
        m(&Json::parse(body).unwrap())["changes"],
        o([("body", t("client-value"))])
    );
    ok(call(&mut f, &owner, "close", o([])));
}

#[test]
fn a01_aliased_parent_child_creation_is_one_entity_batch() {
    let _guard = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    let mut f = Facade::new();
    let mut definition = schema();
    let Json::Object(root) = &mut definition else {
        unreachable!()
    };
    let Json::Object(tables) = root.get_mut("tables").unwrap() else {
        unreachable!()
    };
    let Json::Object(items) = tables.get_mut("items").unwrap() else {
        unreachable!()
    };
    items.insert(
        "relations".into(),
        o([(
            "parent",
            o([
                ("kind", t("ToOne")),
                ("table", t("items")),
                ("foreign_key", t("parent_id")),
                ("optional", Json::Bool(true)),
            ]),
        )]),
    );
    let owner = open_definition(
        &mut f,
        o([("kind", t("memory")), ("store", Json::Null)]),
        vec![],
        definition,
    );
    ok(call(
        &mut f,
        &owner,
        "entity_configure",
        o([(
            "config",
            o([
                ("scope", t("aliases")),
                ("tables", Json::Array(vec![t("items")])),
            ]),
        )]),
    ));
    let insert = |alias, body, references| {
        o([
            ("op", t("insert")),
            ("table", t("items")),
            ("fields", fields(body)),
            ("proposed_id", Json::Null),
            ("alias", t(alias)),
            ("references", references),
        ])
    };
    let result = ok(call(
        &mut f,
        &owner,
        "entity_batch_aliased",
        o([
            (
                "writes",
                Json::Array(vec![
                    insert("p", "parent", o([])),
                    insert("c", "p", o([("parent_id", t("p"))])),
                ]),
            ),
            ("context", context()),
        ]),
    ));
    let aliases = m(&m(&result)["aliases"]);
    assert_ne!(aliases["p"], aliases["c"]);
    let changes = arr(&m(&result)["changes"]);
    assert_eq!(changes.len(), 2);
    assert_eq!(m(&changes[0])["id"], aliases["p"]);
    assert_eq!(m(&m(&changes[1])["after"])["id"], aliases["c"]);
    let child = m(&m(&m(&changes[1])["after"])["fields"]);
    assert_eq!(
        child["parent_id"],
        o([("kind", t("reference")), ("value", aliases["p"].clone())])
    );
    assert_eq!(child["body"], tag("p"));
    let pushed = ok(call(
        &mut f,
        &owner,
        "prepare_push",
        o([("now_ms", t(1000)), ("jitter_basis_points", t(0))]),
    ));
    assert_eq!(arr(&m(&pushed)["envelopes"]).len(), 1);
    ok(call(&mut f, &owner, "close", o([])));
}
fn schema() -> Json {
    o([
        ("version", t(1)),
        (
            "tables",
            o([(
                "items",
                o([
                    (
                        "columns",
                        o([
                            (
                                "body",
                                o([("kind", t("String")), ("optional", Json::Bool(false))]),
                            ),
                            (
                                "extra",
                                o([("kind", t("String")), ("optional", Json::Bool(true))]),
                            ),
                        ]),
                    ),
                    ("relations", o([])),
                ]),
            )]),
        ),
    ])
}
fn open(f: &mut Facade, engine: Json, key: Vec<u8>) -> Json {
    open_definition(f, engine, key, schema())
}
fn open_definition(f: &mut Facade, engine: Json, key: Vec<u8>, definition: Json) -> Json {
    let args = o([
        ("engine", engine),
        ("schema", definition),
        ("migrations", Json::Array(vec![])),
        ("namespace", t("44444444444444444444444444444444")),
        (
            "options",
            o([
                ("capacity", t(1000)),
                ("history", t(100)),
                ("initial_delay_ms", t(1000)),
                ("multiplier", t(2)),
                ("max_delay_ms", t(60000)),
            ]),
        ),
    ]);
    ok(Json::parse(
        &f.open(
            &o([
                ("abi", t(1)),
                ("op", t("open")),
                ("owner", Json::Null),
                ("args", args),
            ])
            .canonical()
            .unwrap(),
            key,
        ),
    )
    .unwrap())
}
fn change(value: &str, time: i64) -> Json {
    o([
        ("table", t("items")),
        ("record_id", t("remote")),
        ("op", t("update")),
        ("changes", fields(value)),
        ("removed_fields", Json::Array(vec![])),
        ("timestamp_ms", t(time)),
        ("field_timestamps", o([("body", t(time))])),
    ])
}
fn page(cp: &str, value: &str, time: i64) -> Json {
    o([
        ("checkpoint", t(cp)),
        ("complete", Json::Bool(true)),
        ("changes", Json::Array(vec![change(value, time)])),
    ])
}
fn exercise(f: &mut Facade, owner: &Json) {
    ok(call(
        f,
        owner,
        "entity_configure",
        o([(
            "config",
            o([
                ("scope", t("facade-entities")),
                ("tables", Json::Array(vec![t("items")])),
            ]),
        )]),
    ));
    let p = ok(call(f, owner, "begin_pull", o([])));
    let request = m(&p)["request"].clone();
    ok(call(
        f,
        owner,
        "apply_pull",
        o([("request", request), ("page", page("cp1", "base", 10))]),
    ));
    let created = ok(call(
        f,
        owner,
        "entity_batch",
        o([
            (
                "writes",
                Json::Array(vec![o([
                    ("op", t("insert")),
                    ("table", t("items")),
                    ("fields", fields("local")),
                    ("proposed_id", Json::Null),
                ])]),
            ),
            ("context", context()),
        ]),
    ));
    let id = m(&m(&arr(&created)[0])["after"])["id"].clone();
    assert!(
        matches!(&id,Json::String(v) if v.starts_with("entity:44444444444444444444444444444444:"))
    );
    ok(call(
        f,
        owner,
        "patch",
        o([
            ("table", t("items")),
            ("id", id),
            ("fields", o([("extra", tag("patch"))])),
            ("context", context()),
        ]),
    ));
    fail(
        call(
            f,
            owner,
            "insert",
            o([
                ("table", t("items")),
                ("fields", fields("missing-context")),
                ("proposed_id", Json::Null),
            ]),
        ),
        "InvalidCommand",
    );
    let p = ok(call(f, owner, "begin_pull", o([])));
    let request = m(&p)["request"].clone();
    let step = ok(call(
        f,
        owner,
        "merge_prepare",
        o([
            ("request", request),
            ("page", page("cp2", "backend", 20)),
            ("tables", Json::Array(vec![t("items")])),
        ]),
    ));
    let session = m(&step)["session"].clone();
    let index = m(&step)["step"].clone();
    assert_eq!(m(&step)["kind"], t("offer"));
    let mut default = ok(call(
        f,
        owner,
        "merge_input",
        o([
            ("session", session.clone()),
            ("step", index.clone()),
            ("part", t("default")),
        ]),
    ));
    let Json::Object(ref mut default) = default else {
        panic!("default")
    };
    let Json::Object(row) = default.get_mut("row").unwrap() else {
        panic!("row")
    };
    row.insert("fields".into(), fields("custom"));
    let row = Json::Object(row.clone());
    let ready = ok(call(
        f,
        owner,
        "merge_reply",
        o([("session", session.clone()), ("step", index), ("row", row)]),
    ));
    assert_eq!(m(&ready)["kind"], t("ready"));
    ok(call(
        f,
        owner,
        "merge_commit",
        o([("session", session.clone())]),
    ));
    fail(
        call(f, owner, "merge_commit", o([("session", session)])),
        "InvalidHandle",
    );
    let rows = ok(call(f, owner, "fetch", o([("query", query())])));
    assert!(
        arr(&rows)
            .iter()
            .any(|r| m(r)["id"] == t("remote") && m(r)["fields"] == fields("custom"))
    );
    let progress = ok(call(f, owner, "sync_progress", o([])));
    assert_eq!(m(&progress)["pages"], t(2));
    fail(
        call(f, owner, "reset", o([("mode", t("preserve_recovery"))])),
        "Protocol",
    );
    let p = ok(call(f, owner, "begin_pull", o([])));
    let request = m(&p)["request"].clone();
    let step = ok(call(
        f,
        owner,
        "merge_prepare",
        o([
            ("request", request.clone()),
            ("page", page("late", "late", 30)),
            ("tables", Json::Array(vec![t("items")])),
        ]),
    ));
    ok(call(
        f,
        owner,
        "reset",
        o([("mode", t("discard_recovery"))]),
    ));
    fail(
        call(
            f,
            owner,
            "merge_input",
            o([
                ("session", m(&step)["session"].clone()),
                ("step", m(&step)["step"].clone()),
                ("part", t("current")),
            ]),
        ),
        "InvalidHandle",
    );
    fail(
        call(
            f,
            owner,
            "apply_pull",
            o([("request", request), ("page", page("late", "late", 30))]),
        ),
        "InvalidHandle",
    );
    assert!(arr(&ok(call(f, owner, "fetch", o([("query", query())])))).is_empty());
}
#[test]
fn g12_g16_entities_passive_merge_and_reset_memory_and_protected_sqlite() {
    let _owners = OWNERS.lock().unwrap();
    let mut f = Facade::new();
    let owner = open(
        &mut f,
        o([("kind", t("memory")), ("store", Json::Null)]),
        vec![],
    );
    exercise(&mut f, &owner);
    ok(call(&mut f, &owner, "close", o([])));
    let directory =
        std::env::temp_dir().join(format!("hoya-general-entities-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let directory = directory.canonicalize().unwrap();
    let owner = open(
        &mut f,
        o([
            ("kind", t("native")),
            ("directory", t(directory.to_str().unwrap())),
            ("name", t("entities.sqlite")),
            ("backup_excluded", Json::Bool(false)),
        ]),
        vec![0x76; 32],
    );
    exercise(&mut f, &owner);
    ok(call(&mut f, &owner, "close", o([])));
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn g11_patch_preflights_complete_result_before_mutation() {
    let _owners = OWNERS.lock().unwrap();
    let mut f = Facade::new();
    let owner = open(
        &mut f,
        o([("kind", t("memory")), ("store", Json::Null)]),
        vec![],
    );
    let original = ok(call(
        &mut f,
        &owner,
        "insert",
        o([
            ("table", t("items")),
            ("fields", fields("b".repeat(600000))),
            ("proposed_id", t("sized")),
        ]),
    ));
    fail(
        call(
            &mut f,
            &owner,
            "patch",
            o([
                ("table", t("items")),
                ("id", t("sized")),
                ("fields", o([("extra", tag("x".repeat(448500)))])),
                ("context", context()),
            ]),
        ),
        "LimitExceeded",
    );
    assert_eq!(
        ok(call(&mut f, &owner, "fetch", o([("query", query())]))),
        Json::Array(vec![original])
    );
}

#[test]
fn g14_stale_prepare_retires_binding_pull_handle() {
    let _owners = OWNERS.lock().unwrap();
    let mut f = Facade::new();
    let owner = open(
        &mut f,
        o([("kind", t("memory")), ("store", Json::Null)]),
        vec![],
    );
    ok(call(
        &mut f,
        &owner,
        "entity_configure",
        o([(
            "config",
            o([
                ("scope", t("stale-prepare")),
                ("tables", Json::Array(vec![t("items")])),
            ]),
        )]),
    ));
    let p = ok(call(&mut f, &owner, "begin_pull", o([])));
    let request = m(&p)["request"].clone();
    ok(call(
        &mut f,
        &owner,
        "apply_pull",
        o([("request", request), ("page", page("seed", "base", 10))]),
    ));
    let p = ok(call(&mut f, &owner, "begin_pull", o([])));
    let request = m(&p)["request"].clone();
    ok(call(
        &mut f,
        &owner,
        "patch",
        o([
            ("table", t("items")),
            ("id", t("remote")),
            ("fields", fields("local")),
            ("context", context()),
        ]),
    ));
    fail(
        call(
            &mut f,
            &owner,
            "merge_prepare",
            o([
                ("request", request.clone()),
                ("page", page("old", "old", 20)),
                ("tables", Json::Array(vec![t("items")])),
            ]),
        ),
        "StalePull",
    );
    fail(
        call(&mut f, &owner, "abort_pull", o([("request", request)])),
        "InvalidHandle",
    );
}

#[test]
fn g01_g06_g07_managed_links_use_actual_core_for_plain_scoped_and_entity_writes() {
    let _owners = OWNERS.lock().unwrap();
    let mut f = Facade::new();
    let col = || {
        o([(
            "body",
            o([("kind", t("String")), ("optional", Json::Bool(false))]),
        )])
    };
    let fk = |target: &str, key: &str| {
        o([
            ("kind", t("ToOne")),
            ("table", t(target)),
            ("foreign_key", t(key)),
            ("optional", Json::Bool(false)),
        ])
    };
    let definition = o([
        ("version", t(1)),
        (
            "tables",
            o([
                (
                    "items",
                    o([
                        ("columns", col()),
                        (
                            "relations",
                            o([(
                                "groups",
                                o([
                                    ("kind", t("ManyToMany")),
                                    ("table", t("groups")),
                                    ("through", t("memberships")),
                                    ("source_relation", t("item")),
                                    ("target_relation", t("group")),
                                ]),
                            )]),
                        ),
                    ]),
                ),
                ("groups", o([("columns", col()), ("relations", o([]))])),
                (
                    "memberships",
                    o([
                        (
                            "columns",
                            o([(
                                "note",
                                o([("kind", t("String")), ("optional", Json::Bool(true))]),
                            )]),
                        ),
                        (
                            "relations",
                            o([
                                ("item", fk("items", "item_id")),
                                ("group", fk("groups", "group_id")),
                            ]),
                        ),
                    ]),
                ),
            ]),
        ),
    ]);
    let owner = open_definition(
        &mut f,
        o([("kind", t("memory")), ("store", Json::Null)]),
        vec![],
        definition,
    );
    for (table, id) in [("items", "left"), ("groups", "right")] {
        ok(call(
            &mut f,
            &owner,
            "insert",
            o([
                ("table", t(table)),
                ("fields", fields(id)),
                ("proposed_id", t(id)),
            ]),
        ));
    }
    let args = |proposed: Json| {
        o([
            ("table", t("items")),
            ("id", t("left")),
            ("relation", t("groups")),
            ("target", t("right")),
            ("fields", o([("note", tag("membership"))])),
            ("proposed_id", proposed),
        ])
    };
    let linked = ok(call(&mut f, &owner, "link", args(t("edge-fixed"))));
    assert_eq!(m(&linked)["id"], t("edge-fixed"));
    assert_eq!(ok(call(&mut f, &owner, "link", args(t("unused")))), linked);
    let unlink = || {
        o([
            ("table", t("items")),
            ("id", t("left")),
            ("relation", t("groups")),
            ("target", t("right")),
        ])
    };
    assert_eq!(ok(call(&mut f, &owner, "unlink", unlink())), t(1));
    let request = ok(call(&mut f, &owner, "request", o([])));
    let definition = o([
        ("name", t("membership-v1")),
        ("arguments", o([])),
        ("reference_paths", Json::Array(vec![])),
    ]);
    assert_eq!(
        ok(call(
            &mut f,
            &owner,
            "begin",
            o([
                ("request", request.clone()),
                ("definition", definition),
                ("value", o([("kind", t("object")), ("value", o([]))])),
                ("context", context())
            ])
        )),
        Json::Bool(true)
    );
    let mut scoped = m(&args(t("local:scoped-edge"))).clone();
    scoped.insert("request".into(), request.clone());
    let linked = ok(call(&mut f, &owner, "scope_link", Json::Object(scoped)));
    assert_eq!(m(&linked)["id"], t("local:scoped-edge"));
    ok(call(&mut f, &owner, "abort", o([("request", request)])));
    ok(call(
        &mut f,
        &owner,
        "entity_configure",
        o([(
            "config",
            o([
                ("scope", t("memberships")),
                ("tables", Json::Array(vec![t("memberships")])),
            ]),
        )]),
    ));
    fail(
        call(&mut f, &owner, "link", args(Json::Null)),
        "InvalidCommand",
    );
    let mut entity = m(&args(Json::Null)).clone();
    entity.remove("proposed_id");
    entity.insert("context".into(), context());
    let linked = ok(call(&mut f, &owner, "entity_link", Json::Object(entity)));
    assert!(matches!(&m(&linked)["id"],Json::String(value) if value.starts_with("entity:")));
    let mut entity = m(&unlink()).clone();
    entity.insert("context".into(), context());
    assert_eq!(
        ok(call(&mut f, &owner, "entity_unlink", Json::Object(entity))),
        t(1)
    );
}

#[test]
fn g11_mapped_reference_expansion_is_bounded_before_patch() {
    let _owners = OWNERS.lock().unwrap();
    let mut definition = schema();
    let Json::Object(root) = &mut definition else {
        panic!("schema")
    };
    let Json::Object(tables) = root.get_mut("tables").unwrap() else {
        panic!("tables")
    };
    let Json::Object(items) = tables.get_mut("items").unwrap() else {
        panic!("items")
    };
    let Json::Object(columns) = items.get_mut("columns").unwrap() else {
        panic!("columns")
    };
    columns.insert(
        "pointer".into(),
        o([("kind", t("Reference")), ("optional", Json::Bool(true))]),
    );
    let mut f = Facade::new();
    let owner = open_definition(
        &mut f,
        o([("kind", t("memory")), ("store", Json::Null)]),
        vec![],
        definition,
    );
    let original = ok(call(
        &mut f,
        &owner,
        "insert",
        o([
            ("table", t("items")),
            ("fields", fields("b".repeat(848500))),
            ("proposed_id", t("large-row")),
        ]),
    ));
    ok(call(
        &mut f,
        &owner,
        "entity_configure",
        o([(
            "config",
            o([
                ("scope", t("mapped-bounds")),
                ("tables", Json::Array(vec![t("items")])),
            ]),
        )]),
    ));
    let request = ok(call(&mut f, &owner, "request", o([])));
    let definition = o([
        ("name", t("mapped-bound-v1")),
        ("arguments", o([])),
        ("reference_paths", Json::Array(vec![])),
    ]);
    assert_eq!(
        ok(call(
            &mut f,
            &owner,
            "begin",
            o([
                ("request", request.clone()),
                ("definition", definition),
                ("value", o([("kind", t("object")), ("value", o([]))])),
                ("context", context())
            ])
        )),
        Json::Bool(true)
    );
    ok(call(
        &mut f,
        &owner,
        "scope_insert",
        o([
            ("request", request.clone()),
            ("table", t("items")),
            ("fields", fields("producer")),
            ("proposed_id", t("local:producer")),
        ]),
    ));
    let finished = ok(call(
        &mut f,
        &owner,
        "finish",
        o([("request", request), ("result", o([("kind", t("null"))]))]),
    ));
    let key = m(&finished)["key"].clone();
    ok(call(
        &mut f,
        &owner,
        "prepare_push",
        o([("now_ms", t(1000)), ("jitter_basis_points", t(0))]),
    ));
    ok(call(
        &mut f,
        &owner,
        "receive",
        o([
            ("key", key.clone()),
            (
                "outcome",
                o([
                    ("status", t("applied")),
                    ("result", o([("kind", t("null"))])),
                ]),
            ),
        ]),
    ));
    let target = format!("server:{}", "x".repeat(199993));
    ok(call(
        &mut f,
        &owner,
        "reconcile",
        o([
            ("key", key),
            (
                "plan",
                o([
                    ("mappings", o([("local:producer", t(target))])),
                    ("writes", Json::Array(vec![])),
                ]),
            ),
        ]),
    ));
    fail(
        call(
            &mut f,
            &owner,
            "patch",
            o([
                ("table", t("items")),
                ("id", t("large-row")),
                (
                    "fields",
                    o([(
                        "pointer",
                        o([("kind", t("reference")), ("value", t("local:producer"))]),
                    )]),
                ),
                ("context", context()),
            ]),
        ),
        "LimitExceeded",
    );
    let mut q = m(&query()).clone();
    q.insert(
        "predicates".into(),
        Json::Array(vec![o([
            ("field", t("id")),
            ("operator", t("Eq")),
            ("values", Json::Array(vec![tag("large-row")])),
        ])]),
    );
    assert_eq!(
        ok(call(
            &mut f,
            &owner,
            "fetch",
            o([("query", Json::Object(q))])
        )),
        Json::Array(vec![original])
    );
}

#[test]
fn g14_invalid_schema_hook_row_retires_entire_binding_session() {
    let _owners = OWNERS.lock().unwrap();
    let mut f = Facade::new();
    let owner = open(
        &mut f,
        o([("kind", t("memory")), ("store", Json::Null)]),
        vec![],
    );
    ok(call(
        &mut f,
        &owner,
        "entity_configure",
        o([(
            "config",
            o([
                ("scope", t("invalid-hook")),
                ("tables", Json::Array(vec![t("items")])),
            ]),
        )]),
    ));
    let batch = ok(call(&mut f, &owner, "begin_pull", o([])));
    ok(call(
        &mut f,
        &owner,
        "apply_pull",
        o([
            ("request", m(&batch)["request"].clone()),
            ("page", page("seed", "base", 10)),
        ]),
    ));
    let batch = ok(call(&mut f, &owner, "begin_pull", o([])));
    let offered = ok(call(
        &mut f,
        &owner,
        "merge_prepare",
        o([
            ("request", m(&batch)["request"].clone()),
            ("page", page("next", "next", 20)),
            ("tables", Json::Array(vec![t("items")])),
        ]),
    ));
    let session = m(&offered)["session"].clone();
    let step = m(&offered)["step"].clone();
    fail(
        call(
            &mut f,
            &owner,
            "merge_reply",
            o([
                ("session", session.clone()),
                ("step", step.clone()),
                ("row", o([("id", t("remote")), ("fields", o([]))])),
            ]),
        ),
        "InvalidRow",
    );
    fail(
        call(
            &mut f,
            &owner,
            "merge_input",
            o([("session", session), ("step", step), ("part", t("current"))]),
        ),
        "InvalidHandle",
    );
    let batch = ok(call(&mut f, &owner, "begin_pull", o([])));
    assert_eq!(m(&batch)["since"], t("seed"));
}

#[test]
fn g03_merge_indices_keep_the_portable_u32_boundary() {
    let _owners = OWNERS.lock().unwrap();
    let mut f = Facade::new();
    let owner = open(
        &mut f,
        o([("kind", t("memory")), ("store", Json::Null)]),
        vec![],
    );
    ok(call(
        &mut f,
        &owner,
        "entity_configure",
        o([(
            "config",
            o([
                ("scope", t("portable-steps")),
                ("tables", Json::Array(vec![t("items")])),
            ]),
        )]),
    ));
    let pull = ok(call(&mut f, &owner, "begin_pull", o([])));
    let offer = ok(call(
        &mut f,
        &owner,
        "merge_prepare",
        o([
            ("request", m(&pull)["request"].clone()),
            ("page", page("cp1", "incoming", 10)),
            ("tables", Json::Array(vec![t("items")])),
        ]),
    ));
    let session = m(&offer)["session"].clone();
    fail(
        call(
            &mut f,
            &owner,
            "merge_input",
            o([
                ("session", session.clone()),
                ("step", t("4294967296")),
                ("part", t("current")),
            ]),
        ),
        "InvalidRequest",
    );
    fail(
        call(
            &mut f,
            &owner,
            "merge_reply",
            o([
                ("session", session.clone()),
                ("step", t("4294967296")),
                ("row", Json::Null),
            ]),
        ),
        "InvalidRequest",
    );
    assert_eq!(
        ok(call(
            &mut f,
            &owner,
            "merge_input",
            o([
                ("session", session.clone()),
                ("step", m(&offer)["step"].clone()),
                ("part", t("current"))
            ])
        )),
        Json::Null,
        "invalid boundary input leaves valid offer intact"
    );
    ok(call(
        &mut f,
        &owner,
        "merge_abort",
        o([("session", session)]),
    ));
    ok(call(&mut f, &owner, "close", o([])));
}

#[test]
fn d06_durable_entities_configuration_and_resolver_use_shared_facade() {
    let _guard = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    let mut f = Facade::new();
    let owner = open(
        &mut f,
        o([("kind", t("memory")), ("store", Json::Null)]),
        vec![],
    );
    let config = o([(
        "config",
        o([
            ("scope", t("durable")),
            ("tables", Json::Array(vec![t("items")])),
        ]),
    )]);
    ok(call(
        &mut f,
        &owner,
        "entity_configure_durable",
        config.clone(),
    ));
    assert_eq!(
        ok(call(
            &mut f,
            &owner,
            "resolve_entity_id",
            o([("table", t("items")), ("id", t("existing-id"))])
        )),
        t("existing-id")
    );
    fail(
        call(
            &mut f,
            &owner,
            "resolve_entity_id",
            o([
                ("table", t("items")),
                ("id", t("existing-id")),
                ("extra", Json::Null),
            ]),
        ),
        "InvalidRequest",
    );
    let created = ok(call(
        &mut f,
        &owner,
        "entity_batch",
        o([
            (
                "writes",
                Json::Array(vec![o([
                    ("op", t("insert")),
                    ("table", t("items")),
                    ("fields", fields("queued")),
                    ("proposed_id", Json::Null),
                ])]),
            ),
            ("context", context()),
        ]),
    ));
    let id = m(&m(&arr(&created)[0])["after"])["id"].clone();
    assert_eq!(
        ok(call(
            &mut f,
            &owner,
            "resolve_entity_id",
            o([("table", t("items")), ("id", id.clone())])
        )),
        id
    );
    ok(call(&mut f, &owner, "close", o([])));
    let legacy = open(
        &mut f,
        o([("kind", t("memory")), ("store", Json::Null)]),
        vec![],
    );
    ok(call(&mut f, &legacy, "entity_configure", config.clone()));
    fail(
        call(&mut f, &legacy, "entity_configure_durable", config),
        "InvalidCommand",
    );
    ok(call(&mut f, &legacy, "close", o([])));
}

#[test]
fn a05_patch_reply_budget_uses_preceding_replacement_fields() {
    let _guard = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    let mut f = Facade::new();
    let owner = open(
        &mut f,
        o([("kind", t("memory")), ("store", Json::Null)]),
        vec![],
    );
    let original = ok(call(
        &mut f,
        &owner,
        "insert",
        o([
            ("table", t("items")),
            ("proposed_id", t("existing")),
            ("fields", fields("original")),
        ]),
    ));
    ok(call(
        &mut f,
        &owner,
        "entity_configure",
        o([(
            "config",
            o([
                ("scope", t("budget")),
                ("tables", Json::Array(vec![t("items")])),
            ]),
        )]),
    ));
    let args = o([
        (
            "writes",
            Json::Array(vec![
                o([
                    ("op", t("replace")),
                    ("table", t("items")),
                    ("id", t("existing")),
                    ("fields", fields("x".repeat(350_000))),
                    ("references", o([])),
                ]),
                o([
                    ("op", t("patch")),
                    ("table", t("items")),
                    ("id", t("existing")),
                    ("fields", o([("extra", tag("small"))])),
                    ("references", o([])),
                ]),
            ]),
        ),
        ("context", context()),
    ]);
    assert!(args.canonical().unwrap().len() < 1_048_576);
    fail(
        call(&mut f, &owner, "entity_batch_aliased", args),
        "LimitExceeded",
    );
    assert_eq!(
        ok(call(&mut f, &owner, "fetch", o([("query", query())]))),
        Json::Array(vec![original])
    );
    assert_eq!(
        m(&ok(call(&mut f, &owner, "queue_status", o([]))))["live"],
        t(0)
    );
    ok(call(&mut f, &owner, "close", o([])));
}

#[test]
fn a05_repeated_large_patch_expansion_stops_at_reply_lower_bound() {
    let _guard = OWNERS.lock().unwrap_or_else(|e| e.into_inner());
    let mut f = Facade::new();
    let owner = open(
        &mut f,
        o([("kind", t("memory")), ("store", Json::Null)]),
        vec![],
    );
    let original = ok(call(
        &mut f,
        &owner,
        "insert",
        o([
            ("table", t("items")),
            ("proposed_id", t("large")),
            ("fields", fields("x".repeat(300_000))),
        ]),
    ));
    ok(call(
        &mut f,
        &owner,
        "entity_configure",
        o([(
            "config",
            o([
                ("scope", t("bound")),
                ("tables", Json::Array(vec![t("items")])),
            ]),
        )]),
    ));
    let patch = |id| {
        o([
            ("op", t("patch")),
            ("table", t("items")),
            ("id", t(id)),
            ("fields", o([("extra", tag("tiny"))])),
            ("references", o([])),
        ])
    };
    // Five tiny writes cannot cause five retained copies of this large row:
    // after-row bytes alone exceed the complete frame cap before the late read.
    let args = o([
        (
            "writes",
            Json::Array(vec![
                patch("large"),
                patch("large"),
                patch("large"),
                patch("large"),
                patch("not-read-after-limit"),
            ]),
        ),
        ("context", context()),
    ]);
    assert!(args.canonical().unwrap().len() < 2000);
    fail(
        call(&mut f, &owner, "entity_batch_aliased", args),
        "LimitExceeded",
    );
    assert_eq!(
        ok(call(&mut f, &owner, "fetch", o([("query", query())]))),
        Json::Array(vec![original])
    );
    assert_eq!(
        m(&ok(call(&mut f, &owner, "queue_status", o([]))))["live"],
        t(0)
    );
    ok(call(&mut f, &owner, "close", o([])));
}
