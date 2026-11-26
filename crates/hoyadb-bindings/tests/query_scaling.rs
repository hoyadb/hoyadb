//! DEF-0058 / TC-0041 SI-Q04: the public facade preserves validated query work.
#![cfg(not(target_arch = "wasm32"))]
use hoyadb_bindings::Facade;
use hoyadb_core::{command::Json, storage::counters};

fn object<const N: usize>(values: [(&str, Json); N]) -> Json {
    Json::Object(values.into_iter().map(|(k, v)| (k.into(), v)).collect())
}

fn text(value: impl ToString) -> Json {
    Json::String(value.to_string())
}

fn frame(op: &str, owner: Json, args: Json) -> String {
    object([
        ("abi", text(1)),
        ("op", text(op)),
        ("owner", owner),
        ("args", args),
    ])
    .canonical()
    .unwrap()
}

#[track_caller]
fn success(output: String) -> Json {
    let Json::Object(reply) = Json::parse(&output).unwrap() else {
        panic!("facade reply must be an object");
    };
    assert_eq!(reply["ok"], Json::Bool(true), "{output}");
    reply["value"].clone()
}

fn fields() -> Json {
    object([(
        "rank",
        object([("kind", text("integer")), ("value", text(7))]),
    )])
}

fn measured(native: bool, n: usize) -> Vec<counters::WorkSnapshot> {
    let directory =
        std::env::temp_dir().join(format!("hoya-facade-query-{}-{n}", std::process::id()));
    let engine = if native {
        std::fs::create_dir(&directory).unwrap();
        object([
            ("kind", text("native")),
            (
                "directory",
                text(directory.canonicalize().unwrap().to_str().unwrap()),
            ),
            ("name", text("store.sqlite")),
            ("backup_excluded", Json::Bool(false)),
        ])
    } else {
        object([("kind", text("memory")), ("store", Json::Null)])
    };
    let schema = Json::parse(r#"{"version":"1","tables":{"items":{"columns":{"rank":{"kind":"Integer","optional":false}},"relations":{}}}}"#).unwrap();
    let options = Json::parse(r#"{"capacity":"1000","history":"100","initial_delay_ms":"1000","multiplier":"2","max_delay_ms":"60000"}"#).unwrap();
    let mut facade = Facade::new();
    let owner = success(facade.open(
        &frame(
            "open",
            Json::Null,
            object([
                ("engine", engine),
                ("schema", schema),
                ("migrations", Json::Array(vec![])),
                ("namespace", text("58585858585858585858585858585858")),
                ("options", options),
            ]),
        ),
        if native {
            vec![0x58; 32]
        } else {
            vec![]
        },
    ));
    let writes = (0..n)
        .map(|i| {
            object([
                ("op", text("insert")),
                ("table", text("items")),
                ("fields", fields()),
                ("proposed_id", text(format!("r{i:04}"))),
            ])
        })
        .collect();
    success(facade.call(&frame(
        "batch",
        owner.clone(),
        object([("writes", Json::Array(writes))]),
    )));
    let expected = Json::Array(
        (0..3)
            .map(|i| object([("id", text(format!("r{i:04}"))), ("fields", fields())]))
            .collect(),
    );
    let mut work = Vec::new();
    for direction in [None, Some("Asc"), Some("Desc")] {
        let order = direction.map_or(Json::Null, |direction| {
            object([("field", text("rank")), ("direction", text(direction))])
        });
        let query = object([
            ("table", text("items")),
            ("predicates", Json::Array(vec![])),
            ("order", order),
            ("limit", text(3)),
            ("source", Json::Null),
        ]);
        counters::reset();
        let rows = success(facade.call(&frame("fetch", owner.clone(), object([("query", query)]))));
        work.push(counters::snapshot());
        assert_eq!(rows, expected, "exact values and ascending IDs within ties");
    }
    success(facade.call(&frame("close", owner, object([]))));
    if native {
        std::fs::remove_dir_all(directory).unwrap();
    }
    work
}

#[test]
fn si_q04_facade_memory_keeps_bounded_validated_queries() {
    let small = measured(false, 10);
    let large = measured(false, 1000);
    for (a, b) in small.iter().zip(&large) {
        assert_eq!(a.memory_rows_visited, 3, "{a:?}");
        assert_eq!(
            a.memory_rows_visited, b.memory_rows_visited,
            "{a:?} vs {b:?}"
        );
        assert_eq!(a.ordered_ops, b.ordered_ops, "{a:?} vs {b:?}");
    }
}

#[test]
fn si_q04_facade_sqlite_keeps_sort_free_equal_work() {
    let small = measured(true, 10);
    let large = measured(true, 1000);
    for (a, b) in small.iter().zip(&large) {
        for work in [a, b] {
            assert_eq!(work.sqlite_sort, 0, "{work:?}");
            assert_eq!(work.sqlite_fullscan_steps, 0, "{work:?}");
            assert!(work.sqlite_rows_read <= 3, "{work:?}");
        }
        assert_eq!(a.sqlite_rows_read, b.sqlite_rows_read, "{a:?} vs {b:?}");
        assert_eq!(a.sqlite_vm_steps, b.sqlite_vm_steps, "{a:?} vs {b:?}");
    }
}
