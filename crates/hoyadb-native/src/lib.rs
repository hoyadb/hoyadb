//! API-0011 / TC-0036 G03/G09/G10: checked native spans over the general facade.
use hoyadb_bindings::Facade;
use std::{
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Mutex, OnceLock},
};

const MAX_FRAME_BYTES: usize = 1_048_576;
static RUNTIME: OnceLock<Mutex<Facade>> = OnceLock::new();

pub type Reply = Option<unsafe extern "C" fn(*mut c_void, *const u8, usize)>;

#[unsafe(no_mangle)]
pub extern "C" fn hoya_v1_abi_version() -> u32 {
    1
}

/// Open through the separate binary key channel.
///
/// # Safety
/// Nonempty admitted spans must remain readable throughout the synchronous call.
/// The callback/context must remain valid and cannot throw or unwind. Reply spans
/// are borrowed only until the callback returns. Arbitrary inaccessible addresses
/// and foreign faults cannot be made safe by Rust panic containment.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoya_v1_open(
    json: *const u8,
    length: usize,
    key: *const u8,
    key_length: usize,
    reply: Reply,
    context: *mut c_void,
) -> i32 {
    // SAFETY: the caller supplies the documented readable spans and collector.
    unsafe { request(json, length, Some((key, key_length)), reply, context) }
}

/// Execute an ordinary frame; open is accepted only by `hoya_v1_open`.
///
/// # Safety
/// The readable-span and callback obligations of `hoya_v1_open` also apply here.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoya_v1_call(
    json: *const u8,
    length: usize,
    reply: Reply,
    context: *mut c_void,
) -> i32 {
    // SAFETY: the caller supplies the documented readable span and collector.
    unsafe { request(json, length, None, reply, context) }
}

/// Readonly original open-frame validation without key bytes or runtime state.
///
/// # Safety
/// The readable-span and callback obligations of `hoya_v1_open` also apply here.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoya_v1_validate_open(
    json: *const u8,
    length: usize,
    key_length: usize,
    reply: Reply,
    context: *mut c_void,
) -> i32 {
    let Some(reply) = reply else {
        return 1;
    };
    if !matches!(key_length, 0 | 32) || length > MAX_FRAME_BYTES || (length != 0 && json.is_null())
    {
        return 1;
    }
    let bytes = if length == 0 {
        &[]
    } else {
        // SAFETY: null/limit checks precede dereference; caller retains its
        // readable admitted span throughout the synchronous validation call.
        unsafe { std::slice::from_raw_parts(json, length) }
    };
    let Ok(input) = std::str::from_utf8(bytes) else {
        return 1;
    };
    contain(|| {
        // This pure parser never initializes/acquires the process registry,
        // creates a handle, or opens storage. No key bytes are supplied.
        let output = Facade::new().validate_open(input, key_length);
        // SAFETY: caller retains collector/context, and output owns the span.
        unsafe { reply(context, output.as_ptr(), output.len()) };
        0
    })
}

unsafe fn request(
    json: *const u8,
    length: usize,
    key: Option<(*const u8, usize)>,
    reply: Reply,
    context: *mut c_void,
) -> i32 {
    let Some(reply) = reply else {
        return 1;
    };
    if length > MAX_FRAME_BYTES || (length != 0 && json.is_null()) {
        return 1;
    }
    if let Some((pointer, size)) = key
        && (!matches!(size, 0 | 32) || (size != 0 && pointer.is_null()))
    {
        return 1;
    }
    let bytes = if length == 0 {
        &[]
    } else {
        // SAFETY: bounds/null checks precede dereference; readable accessibility
        // and lifetime of an admitted span are the caller's obligations.
        unsafe { std::slice::from_raw_parts(json, length) }
    };
    let Ok(input) = std::str::from_utf8(bytes) else {
        return 1;
    };
    contain(|| {
        let output = {
            let Ok(mut facade) = RUNTIME.get_or_init(|| Mutex::new(Facade::new())).lock() else {
                return 2;
            };
            match key {
                Some((pointer, size)) => {
                    let owned_key = if size == 0 {
                        vec![]
                    } else {
                        // SAFETY: key size/null were checked before any input
                        // dereference; caller retains its readable binary key.
                        unsafe { std::slice::from_raw_parts(pointer, size) }.to_vec()
                    };
                    // Facade immediately owns and zeroizes this key copy.
                    facade.open(input, owned_key)
                }
                None => facade.call(input),
            }
        };
        // The registry lock has ended. Only a borrowed reply collector runs
        // here, never an application handler/event callback.
        // SAFETY: callback/context remain valid for this synchronous call and
        // the owned response lives until the collector returns.
        unsafe { reply(context, output.as_ptr(), output.len()) };
        0
    })
}

fn contain(operation: impl FnOnce() -> i32) -> i32 {
    // The application's panic hook remains application-controlled. Only the
    // boundary response is contained/redacted; no process-global hook changes.
    catch_unwind(AssertUnwindSafe(operation)).unwrap_or(2)
}

#[cfg(test)]
mod tests {
    #[test]
    fn abi_version_is_one() {
        assert_eq!(super::hoya_v1_abi_version(), 1);
    }

    #[test]
    fn rust_panic_returns_static_status_without_escaping() {
        assert_eq!(super::contain(|| panic!("synthetic boundary fault")), 2);
        assert_eq!(super::contain(|| 0), 0);
    }

    #[test]
    fn readonly_preflight_leaves_process_registry_uninitialized() {
        use std::ffi::c_void;
        unsafe extern "C" fn collect(context: *mut c_void, bytes: *const u8, length: usize) {
            // SAFETY: test retains the output and the producer retains its reply.
            let output = unsafe { &mut *context.cast::<Vec<u8>>() };
            output.extend_from_slice(unsafe { std::slice::from_raw_parts(bytes, length) });
        }
        let input = include_str!("../tests/fixtures/memory-open.json");
        let mut output = Vec::<u8>::new();
        assert!(super::RUNTIME.get().is_none());
        // SAFETY: valid owned input/context/collector across this synchronous call.
        assert_eq!(
            unsafe {
                super::hoya_v1_validate_open(
                    input.as_ptr(),
                    input.len(),
                    0,
                    Some(collect),
                    (&mut output as *mut Vec<u8>).cast(),
                )
            },
            0
        );
        assert!(super::RUNTIME.get().is_none());
        assert_eq!(
            String::from_utf8(output).unwrap(),
            r#"{"abi":"1","ok":true,"value":null}"#
        );
    }
}
