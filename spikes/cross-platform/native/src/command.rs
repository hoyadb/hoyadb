//! API-0008: call-scoped C ABI over the same actual SQLite fixture as WASM.
use std::{
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Mutex, OnceLock},
};

#[path = "../../../common/command_fixture.rs"]
mod fixture;
static FIXTURE: OnceLock<Mutex<fixture::CommandFixture>> = OnceLock::new();

pub type CommandReply = unsafe extern "C" fn(*mut c_void, *const u8, usize);

/// # Safety
/// Request is a readable byte span for this call; reply/context must be valid.
/// Reply must copy borrowed response bytes and must not unwind across C.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoya_command_rpc(
    request: *const u8,
    length: usize,
    reply: Option<CommandReply>,
    context: *mut c_void,
) -> i32 {
    if request.is_null() || length > 1_048_576 {
        return 1;
    }
    let Some(reply) = reply else {
        return 1;
    };
    // SAFETY: caller provides the readable call-scoped span; size is bounded above.
    let input = unsafe { std::slice::from_raw_parts(request, length) };
    let Ok(input) = std::str::from_utf8(input) else {
        return 1;
    };
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let response = {
            let Ok(mut fixture) = FIXTURE
                .get_or_init(|| Mutex::new(fixture::CommandFixture::new()))
                .lock()
            else {
                return 2;
            };
            fixture.dispatch(input)
        };
        // SAFETY: response remains live through callback. No registry lock is held.
        unsafe { reply(context, response.as_ptr(), response.len()) };
        0
    }));
    outcome.unwrap_or(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "C" fn copy_reply(context: *mut c_void, bytes: *const u8, length: usize) {
        // SAFETY: test passes a live String and the bridge supplies call-scoped bytes.
        let value =
            unsafe { std::str::from_utf8(std::slice::from_raw_parts(bytes, length)).unwrap() };
        unsafe { *(context as *mut String) = value.into() };
    }

    #[test]
    fn h03_c_boundary_validates_spans_and_delivers_owned_typed_reply() {
        let request = br#"{"op":"unknown","owner":null,"args":{}}"#;
        let mut response = String::new();
        let context = &mut response as *mut String as *mut c_void;
        // SAFETY: all spans/context live through this synchronous call.
        let code =
            unsafe { hoya_command_rpc(request.as_ptr(), request.len(), Some(copy_reply), context) };
        assert_eq!(code, 0, "valid request receives typed error JSON");
        assert!(response.contains("InvalidRequest"));
        assert_eq!(
            unsafe { hoya_command_rpc(std::ptr::null(), 1, Some(copy_reply), context) },
            1
        );
        assert_eq!(
            unsafe { hoya_command_rpc(request.as_ptr(), request.len(), None, context) },
            1
        );
        assert_eq!(
            unsafe { hoya_command_rpc([0xff].as_ptr(), 1, Some(copy_reply), context) },
            1
        );
    }

    unsafe extern "C" fn reenter(context: *mut c_void, _bytes: *const u8, _length: usize) {
        let request = br#"{"op":"unknown","owner":null,"args":{}}"#;
        let mut nested = String::new();
        // SAFETY: nested owned buffer lives through callback and registry lock released.
        let status = unsafe {
            hoya_command_rpc(
                request.as_ptr(),
                request.len(),
                Some(copy_reply),
                &mut nested as *mut String as *mut c_void,
            )
        };
        unsafe { *(context as *mut bool) = status == 0 && nested.contains("InvalidRequest") };
    }

    #[test]
    fn h04_reply_callback_reenters_after_registry_unlock() {
        let request = br#"{"op":"unknown","owner":null,"args":{}}"#;
        let mut success = false;
        assert_eq!(
            unsafe {
                hoya_command_rpc(
                    request.as_ptr(),
                    request.len(),
                    Some(reenter),
                    &mut success as *mut bool as *mut c_void,
                )
            },
            0
        );
        assert!(success);
    }
}
