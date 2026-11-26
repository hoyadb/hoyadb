//! TC-0041 SI-L04/SI-L07: separate facades share no store, and close releases it.
#![cfg(not(target_arch = "wasm32"))]
use hoyadb_bindings::Facade;
use hoyadb_core::command::Json;

const KEY: [u8; 32] = [0x6b; 32];

fn o<const N: usize>(values: [(&str, Json); N]) -> Json {
    Json::Object(values.into_iter().map(|(k, v)| (k.into(), v)).collect())
}

fn t(value: impl ToString) -> Json {
    Json::String(value.to_string())
}

fn frame(op: &str, owner: Json, args: Json) -> String {
    o([
        ("abi", t(1)),
        ("op", t(op)),
        ("owner", owner),
        ("args", args),
    ])
    .canonical()
    .unwrap()
}

fn open_args(directory: &str) -> Json {
    let notes = o([
        (
            "columns",
            o([(
                "name",
                o([("kind", t("String")), ("optional", Json::Bool(false))]),
            )]),
        ),
        ("relations", o([])),
    ]);
    o([
        (
            "engine",
            o([
                ("kind", t("native")),
                ("directory", t(directory)),
                ("name", t("owned.sqlite")),
                ("backup_excluded", Json::Bool(false)),
            ]),
        ),
        (
            "schema",
            o([("version", t(1)), ("tables", o([("notes", notes)]))]),
        ),
        ("migrations", Json::Array(vec![])),
        ("namespace", t("6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b")),
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
    ])
}

fn opened(output: String) -> Json {
    let Json::Object(response) = Json::parse(&output).unwrap() else {
        panic!("facade must return an object");
    };
    assert_eq!(response["ok"], Json::Bool(true), "{output}");
    response["value"].clone()
}

#[test]
fn si_l04_l07_second_facade_is_busy_until_close() {
    let directory =
        std::env::temp_dir().join(format!("hoya-facade-ownership-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let directory = directory.canonicalize().unwrap();
    let path = directory.to_str().unwrap().to_owned();
    let mut first = Facade::new();
    let mut second = Facade::new();
    let owner = opened(first.open(&frame("open", Json::Null, open_args(&path)), KEY.to_vec()));
    let reply = second.open(&frame("open", Json::Null, open_args(&path)), KEY.to_vec());
    assert_eq!(
        Json::parse(&reply).unwrap(),
        o([
            ("abi", t(1)),
            ("ok", Json::Bool(false)),
            ("error", o([("kind", t("Busy"))])),
        ]),
        "{reply}"
    );
    let hex: String = KEY.iter().map(|byte| format!("{byte:02x}")).collect();
    for secret in [path.as_str(), "owned.sqlite", hex.as_str()] {
        assert!(!reply.contains(secret));
    }
    opened(first.call(&frame("close", owner, o([]))));
    let reopened = opened(second.open(&frame("open", Json::Null, open_args(&path)), KEY.to_vec()));
    opened(second.call(&frame("close", reopened, o([]))));
    std::fs::remove_dir_all(directory).unwrap();
}
