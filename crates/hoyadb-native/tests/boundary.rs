//! TC-0036 G01/G03/G10: real facade, readable spans and copied owned replies.
use hoyadb_core::command::Json;
use hoyadb_native::{hoya_v1_call, hoya_v1_open, hoya_v1_validate_open};
use std::{ffi::c_void, ptr};

const LIMIT: usize = 1_048_576;

#[derive(Default)]
struct Collected(Vec<Vec<u8>>);

unsafe extern "C" fn collect(context: *mut c_void, bytes: *const u8, length: usize) {
    // SAFETY: each test retains its collector and the native reply span is valid
    // for this callback. Copy immediately; never retain the borrowed span.
    let collector = unsafe { &mut *context.cast::<Collected>() };
    let owned = unsafe { std::slice::from_raw_parts(bytes, length) }.to_vec();
    collector.0.push(owned);
}

fn memory_frame(table: &str, column: &str, namespace: &str) -> String {
    format!(
        r#"{{"abi":"1","op":"open","owner":null,"args":{{"engine":{{"kind":"memory","store":null}},"schema":{{"version":"1","tables":{{"{table}":{{"columns":{{"{column}":{{"kind":"String","optional":false}}}},"relations":{{}}}}}}}},"migrations":[],"namespace":"{namespace}","options":{{"capacity":"1000","history":"100","initial_delay_ms":"1000","multiplier":"2","max_delay_ms":"60000"}}}}}}"#
    )
}

fn call(frame: &str) -> Json {
    let mut output = Collected::default();
    // SAFETY: owned UTF8 input and the collector live across this call.
    let status = unsafe {
        hoya_v1_call(
            frame.as_ptr(),
            frame.len(),
            Some(collect),
            (&mut output as *mut Collected).cast(),
        )
    };
    assert_eq!(status, 0);
    assert_eq!(output.0.len(), 1);
    Json::parse(std::str::from_utf8(&output.0[0]).unwrap()).unwrap()
}

fn value(reply: Json) -> Json {
    let Json::Object(mut map) = reply else {
        panic!("reply object")
    };
    assert_eq!(map.remove("abi"), Some(Json::String("1".into())));
    assert_eq!(map.remove("ok"), Some(Json::Bool(true)), "{map:?}");
    map.remove("value").unwrap()
}

#[test]
fn g01_valid_memory_spans_open_two_caller_schemas_and_copy_one_reply() {
    for (table, column, namespace) in [
        ("poets", "display", "11111111111111111111111111111111"),
        ("devices", "label", "22222222222222222222222222222222"),
    ] {
        let frame = memory_frame(table, column, namespace);
        let mut output = Collected::default();
        // SAFETY: owned readable input, empty memory-key span and collector.
        assert_eq!(
            unsafe {
                hoya_v1_open(
                    frame.as_ptr(),
                    frame.len(),
                    ptr::null(),
                    0,
                    Some(collect),
                    (&mut output as *mut Collected).cast(),
                )
            },
            0
        );
        assert_eq!(output.0.len(), 1);
        let reply = Json::parse(std::str::from_utf8(&output.0[0]).unwrap()).unwrap();
        drop(output); // Native spans have ended; the copied JSON remains owned.
        let Json::String(owner) = value(reply) else {
            panic!("owner handle")
        };
        let result = value(call(&format!(
            r#"{{"abi":"1","op":"schema","owner":"{owner}","args":{{}}}}"#
        )));
        let Json::Object(schema) = result else {
            panic!("schema")
        };
        let Json::Object(tables) = &schema["tables"] else {
            panic!("tables")
        };
        assert!(tables.contains_key(table));
        assert_eq!(tables.len(), 1);
        assert_eq!(
            value(call(&format!(
                r#"{{"abi":"1","op":"close","owner":"{owner}","args":{{}}}}"#
            ))),
            Json::Null
        );
    }
}

#[test]
fn g10_invalid_spans_lengths_utf8_and_callback_reject_before_dereference() {
    let mut output = Collected::default();
    let context = (&mut output as *mut Collected).cast();
    let dangling = ptr::NonNull::<u8>::dangling().as_ptr();
    // SAFETY: invalid spans are null/over-limit and must be rejected before any
    // dereference. The malformed UTF8 case is actual owned readable memory.
    unsafe {
        assert_eq!(hoya_v1_call(ptr::null(), 1, Some(collect), context), 1);
        assert_eq!(hoya_v1_call(dangling, LIMIT + 1, Some(collect), context), 1);
        assert_eq!(hoya_v1_call(ptr::null(), 0, None, context), 1);
        assert_eq!(hoya_v1_call([0xff].as_ptr(), 1, Some(collect), context), 1);
        assert_eq!(
            hoya_v1_open(ptr::null(), 0, dangling, 31, Some(collect), context),
            1
        );
        assert_eq!(
            hoya_v1_open(ptr::null(), 0, ptr::null(), 32, Some(collect), context),
            1
        );
    }
    assert!(output.0.is_empty());
}

#[test]
fn g03_empty_valid_span_has_one_static_protocol_reply_and_open_is_separate() {
    let reply = call("");
    let Json::Object(map) = reply else {
        panic!("reply object")
    };
    assert_eq!(map["ok"], Json::Bool(false));
    let Json::Object(error) = &map["error"] else {
        panic!("static error")
    };
    assert_eq!(error.len(), 1);
    assert!(matches!(&error["kind"], Json::String(_)));
    let frame = memory_frame("groups", "name", "33333333333333333333333333333333");
    let Json::Object(map) = call(&frame) else {
        panic!("reply object")
    };
    assert_eq!(map["ok"], Json::Bool(false));
}

#[test]
fn g10_collector_reentry_uses_no_borrowed_registry_lock() {
    unsafe extern "C" fn reenter(context: *mut c_void, bytes: *const u8, length: usize) {
        // SAFETY: this collector has the same valid-span/context contract.
        unsafe { collect(context, bytes, length) };
        // A genuine second facade call proves the first registry lock ended.
        // No application handler/event callback is invoked here.
        let nested = call("");
        let Json::Object(reply) = nested else {
            panic!("nested reply")
        };
        assert_eq!(reply["ok"], Json::Bool(false));
    }
    let mut output = Collected::default();
    // SAFETY: empty valid input and a retained collector/context.
    assert_eq!(
        unsafe {
            hoya_v1_call(
                ptr::null(),
                0,
                Some(reenter),
                (&mut output as *mut Collected).cast(),
            )
        },
        0
    );
    assert_eq!(output.0.len(), 1);
}

#[test]
fn g16_real_protected_binary_key_reopen_and_wrong_key_preserve_bytes() {
    let root = std::env::temp_dir().join(format!("hoya-native-abi-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let directory = root.canonicalize().unwrap();
    let frame = memory_frame("instruments", "title", "44444444444444444444444444444444")
        .replace(r#""engine":{"kind":"memory","store":null}"#, &format!(r#""engine":{{"kind":"native","directory":"{}","name":"sealed.sqlite","backup_excluded":false}}"#, directory.to_str().unwrap()));
    let key: Vec<u8> = (0..32).map(|n| (n * 17) as u8).collect();
    assert!(key.contains(&0) && key.contains(&255));
    let open = |key: &[u8]| {
        let mut result = Collected::default();
        // SAFETY: both owned readable spans and collector outlive the call.
        assert_eq!(
            unsafe {
                hoya_v1_open(
                    frame.as_ptr(),
                    frame.len(),
                    key.as_ptr(),
                    key.len(),
                    Some(collect),
                    (&mut result as *mut Collected).cast(),
                )
            },
            0
        );
        assert_eq!(result.0.len(), 1);
        Json::parse(std::str::from_utf8(&result.0[0]).unwrap()).unwrap()
    };
    let Json::String(owner) = value(open(&key)) else {
        panic!("owner")
    };
    value(call(&format!(
        r#"{{"abi":"1","op":"close","owner":"{owner}","args":{{}}}}"#
    )));
    let before = std::fs::read(root.join("sealed.sqlite")).unwrap();
    assert!(!before.starts_with(b"SQLite format 3\0"));
    let Json::Object(error) = open(&[0x55; 32]) else {
        panic!("error")
    };
    assert_eq!(error["ok"], Json::Bool(false));
    assert_eq!(std::fs::read(root.join("sealed.sqlite")).unwrap(), before);
    let Json::Object(error) = open(&[]) else {
        panic!("error")
    };
    assert_eq!(error["ok"], Json::Bool(false));
    assert_eq!(std::fs::read(root.join("sealed.sqlite")).unwrap(), before);
    let Json::String(owner) = value(open(&key)) else {
        panic!("reopened owner")
    };
    let Json::Object(schema) = value(call(&format!(
        r#"{{"abi":"1","op":"schema","owner":"{owner}","args":{{}}}}"#
    ))) else {
        panic!("schema")
    };
    assert_eq!(schema["version"], Json::String("1".into()));
    value(call(&format!(
        r#"{{"abi":"1","op":"close","owner":"{owner}","args":{{}}}}"#
    )));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn g10_readonly_open_preflight_preserves_original_parser_and_borrow_rules() {
    let input = memory_frame("readonly", "name", "55555555555555555555555555555555");
    let preflight = |input: &str, key_length: usize| {
        let mut output = Collected::default();
        // SAFETY: owned readable UTF8 and collector remain live for the call.
        assert_eq!(
            unsafe {
                hoya_v1_validate_open(
                    input.as_ptr(),
                    input.len(),
                    key_length,
                    Some(collect),
                    (&mut output as *mut Collected).cast(),
                )
            },
            0
        );
        assert_eq!(output.0.len(), 1);
        Json::parse(std::str::from_utf8(&output.0[0]).unwrap()).unwrap()
    };
    assert_eq!(value(preflight(&input, 0)), Json::Null);
    let native = input.replace(r#""engine":{"kind":"memory","store":null}"#,
        r#""engine":{"kind":"native","directory":"@hoyadb/default","name":"sealed.sqlite","backup_excluded":true}"#);
    assert_eq!(value(preflight(&native, 32)), Json::Null);
    for bad in [
        input.replace(r#""abi":"1""#, r#""abi":"1","abi":"1""#),
        input.replace(r#""version":"1""#, r#""version":1"#),
        input.replace(r#""kind":"String""#, r#""kind":"Unknown""#),
    ] {
        let Json::Object(result) = preflight(&bad, 0) else {
            panic!("reply")
        };
        assert_eq!(result["ok"], Json::Bool(false));
    }
    let mut collected = Collected::default();
    let ctx = (&mut collected as *mut Collected).cast();
    // SAFETY: deliberately inadmissible spans/lengths reject before dereference;
    // UTF8 uses readable bytes. No forged admitted accessible-size pointer.
    unsafe {
        assert_eq!(
            hoya_v1_validate_open(ptr::null(), 1, 0, Some(collect), ctx),
            1
        );
        assert_eq!(
            hoya_v1_validate_open(
                ptr::NonNull::<u8>::dangling().as_ptr(),
                LIMIT + 1,
                0,
                Some(collect),
                ctx
            ),
            1
        );
        assert_eq!(
            hoya_v1_validate_open(ptr::null(), 0, 31, Some(collect), ctx),
            1
        );
        assert_eq!(
            hoya_v1_validate_open([0xff].as_ptr(), 1, 0, Some(collect), ctx),
            1
        );
        assert_eq!(hoya_v1_validate_open(ptr::null(), 0, 0, None, ctx), 1);
    }
    assert!(collected.0.is_empty());
    unsafe extern "C" fn reenter(context: *mut c_void, bytes: *const u8, length: usize) {
        // SAFETY: this collector retains the same valid spans/context.
        unsafe { collect(context, bytes, length) };
        let _ = call("");
    }
    // SAFETY: owned input and retained collector; a nested ordinary call must work.
    assert_eq!(
        unsafe { hoya_v1_validate_open(input.as_ptr(), input.len(), 0, Some(reenter), ctx) },
        0
    );
    assert_eq!(collected.0.len(), 1);
}
