//! TC-0048 QUALITY07: exact public parsing results, including bounded termination.
use hoyadb_core::{
    command::{Json, JsonError},
    storage::FiniteFloat,
};
const CORPUS_COMPLETED: i32 = 73;

use std::{
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn supervised(mode: &str, deadline: Duration) -> Result<std::process::ExitStatus, String> {
    let filter = if mode == "empty" {
        "__no_matching_parser_test__"
    } else {
        "quality07_json_parser_is_bounded"
    };
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", filter, "--nocapture"])
        .env("HOYADB_JSON_TEST_CHILD", mode)
        .stdin(Stdio::null())
        .spawn()
        .expect("start isolated parser test");
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if start.elapsed() < deadline => thread::sleep(Duration::from_millis(10)),
            result => {
                child.kill().expect("terminate stuck parser child");
                child.wait().expect("reap parser child after termination");
                return Err(format!(
                    "parser child exceeded deadline or could not be polled: {result:?}"
                ));
            }
        }
    }
}

#[test]
fn quality07_json_parser_is_bounded() {
    match std::env::var("HOYADB_JSON_TEST_CHILD").as_deref() {
        Ok("stall") => loop {
            thread::sleep(Duration::from_secs(1));
        },
        Ok("fail") => panic!("synthetic child assertion failure"),
        Ok("corpus") => {
            corpus();
            // A successful libtest exit alone also means zero tests matched.
            std::process::exit(CORPUS_COMPLETED);
        }
        _ => {
            let status = supervised("corpus", Duration::from_secs(10))
                .expect("finite JSON corpus must terminate within ten seconds");
            assert!(
                status.code() == Some(CORPUS_COMPLETED),
                "parser value/error assertion failed: {status}"
            );
        }
    }
}

#[test]
fn quality07_supervisor_reaps_nonterminating_child() {
    let result = supervised("stall", Duration::from_millis(100));
    assert!(result.unwrap_err().contains("exceeded deadline"));
}

// Exact malformed-number categories and underflow-to-zero characterize the
// current Rust implementation. API-0006 promises rejection and permits underflow;
// a deliberate change to these details requires reviewing these expectations.
fn corpus() {
    for codepoint in 0..=0xffff {
        if let Some(character) = char::from_u32(codepoint) {
            for escaped in [
                format!("\"\\u{codepoint:04x}\""),
                format!("\"\\u{codepoint:04X}\""),
            ] {
                assert_eq!(
                    Json::parse(&escaped),
                    Ok(Json::String(character.to_string())),
                    "{escaped}"
                );
            }
        }
    }
    for (escaped, character) in [
        ("\"\\uD800\\uDC00\"", '\u{10000}'),
        ("\"\\uDBFF\\uDFFF\"", '\u{10ffff}'),
        ("\"\\ud83d\\ude00\"", '😀'),
    ] {
        assert_eq!(
            Json::parse(escaped),
            Ok(Json::String(character.to_string()))
        );
    }
    for invalid in [
        "\"\\uZZZZ\"",
        "\"\\u000g\"",
        "\"\\u000/\"",
        "\"\\uD800\"",
        "\"\\uDC00\"",
        "\"\\uD800\\uDBFF\"",
        "\"\\uD800\\uE000\"",
    ] {
        assert_eq!(
            Json::parse(invalid),
            Err(JsonError::InvalidSyntax),
            "{invalid}"
        );
    }
    for value in [i64::MIN, -42, -1, 0, 1, 42, i64::MAX] {
        assert_eq!(Json::parse(&value.to_string()), Ok(Json::Integer(value)));
    }
    for (input, expected) in [("-0", 0), (" 123\n", 123)] {
        assert_eq!(Json::parse(input), Ok(Json::Integer(expected)));
    }
    for (input, expected) in [
        ("0.0", 0.0),
        ("-0.0", 0.0),
        ("1.25", 1.25),
        ("-12.5", -12.5),
        ("1e2", 100.0),
        ("1E+2", 100.0),
        ("1e-2", 0.01),
        ("1e308", 1e308),
        ("1e-9999", 0.0),
    ] {
        assert_eq!(
            Json::parse(input),
            Ok(Json::Float(FiniteFloat::new(expected).unwrap())),
            "{input}"
        );
    }
    for input in [
        "-",
        "1.",
        "1e",
        "1e+",
        "1e-",
        "--1",
        "9223372036854775808",
        "-9223372036854775809",
        "1e309",
        "-1e309",
    ] {
        assert_eq!(Json::parse(input), Err(JsonError::InvalidNumber), "{input}");
    }
    for input in ["01", "-01", "1a", "0x1", "+1", ".1"] {
        assert_eq!(Json::parse(input), Err(JsonError::InvalidSyntax), "{input}");
    }
    assert_eq!(
        Json::parse(&"9".repeat(1024)),
        Err(JsonError::InvalidNumber)
    );
}

#[test]
fn quality07_supervisor_rejects_empty_and_failed_children() {
    for mode in ["empty", "fail"] {
        let status = supervised(mode, Duration::from_secs(10)).unwrap();
        assert!(
            status.code() != Some(CORPUS_COMPLETED),
            "child without completed corpus must not pass: {mode}"
        );
    }
}
