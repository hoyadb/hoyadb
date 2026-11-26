//! SPEC-0018 / REQ-0035: experimental native translation over the shared core.

use hoyadb_cross_platform_spike::Order;
mod command;
mod persistence;
mod reactive;
use persistence::NativeStore;
use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, OnceLock};

static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();

fn registry() -> &'static Mutex<Registry> {
    REGISTRY.get_or_init(|| {
        Mutex::new(Registry {
            next: 1,
            stores: HashMap::new(),
        })
    })
}

struct Registry {
    next: u64,
    stores: HashMap<u64, NativeStore>,
}

impl Registry {
    fn open(&mut self) -> u64 {
        self.register(NativeStore::default())
    }

    fn register(&mut self, store: NativeStore) -> u64 {
        let Some(next) = self.next.checked_add(1) else {
            return 0;
        };
        let handle = self.next;
        self.stores.insert(handle, store);
        self.next = next;
        handle
    }
}

fn boundary<T>(fallback: T, operation: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(operation)).unwrap_or(fallback)
}

fn with_registry<T>(
    registry: &Mutex<Registry>,
    operation: impl FnOnce(&mut Registry) -> Result<T, i32>,
) -> Result<T, i32> {
    let mut registry = registry.lock().map_err(|_| 3)?;
    operation(&mut registry)
}

fn status(operation: impl FnOnce() -> Result<(), i32>) -> i32 {
    boundary(3, || match operation() {
        Ok(()) => 0,
        Err(error) => error,
    })
}

// SAFETY: callers uphold readable NUL-terminated input memory; null and UTF-8
// validation cannot establish pointer validity. No input borrow is retained.
unsafe fn input(pointer: *const c_char) -> Result<String, i32> {
    if pointer.is_null() {
        return Err(2);
    }
    unsafe { CStr::from_ptr(pointer) }
        .to_str()
        .map(str::to_owned)
        .map_err(|_| 2)
}

pub type RowVisitor =
    unsafe extern "C" fn(*mut c_void, *const u8, usize, *const u8, usize, i64, u8);

#[unsafe(no_mangle)]
pub extern "C" fn hoya_open() -> u64 {
    boundary(0, || {
        with_registry(registry(), |registry| Ok(registry.open())).unwrap_or(0)
    })
}

/// # Safety
/// Path must be readable NUL-terminated memory for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoya_open_persistent(path: *const c_char) -> u64 {
    boundary(0, || {
        // SAFETY: caller provides a readable NUL-terminated path for this call.
        let Ok(path) = (unsafe { input(path) }) else {
            return 0;
        };
        let Ok(store) = NativeStore::open(&path) else {
            return 0;
        };
        with_registry(registry(), |registry| Ok(registry.register(store))).unwrap_or(0)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn hoya_close(handle: u64) -> i32 {
    status(|| {
        with_registry(registry(), |registry| {
            registry.stores.remove(&handle).map(|_| ()).ok_or(1)
        })
    })
}

/// # Safety
/// Non-null input strings must be valid readable NUL-terminated memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoya_insert(
    handle: u64,
    id: *const c_char,
    row_status: *const c_char,
    total_cents: i64,
    paid: u8,
) -> i32 {
    status(|| {
        if paid > 1 {
            return Err(2);
        }
        // SAFETY: caller guarantees valid readable input strings for the call.
        let id = unsafe { input(id)? };
        // SAFETY: same lifetime and validity contract as the ID input.
        let row_status = unsafe { input(row_status)? };
        with_registry(registry(), |registry| {
            let store = registry.stores.get_mut(&handle).ok_or(1)?;
            store.write_insert(Order {
                id,
                status: row_status,
                total_cents,
                paid: paid == 1,
            })?;
            Ok(())
        })
    })
}

/// # Safety
/// Input strings and callback context must satisfy SPEC-0018's lifetimes;
/// callbacks must not unwind or retain borrowed row pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoya_query(
    handle: u64,
    row_status: *const c_char,
    visitor: Option<RowVisitor>,
    context: *mut c_void,
) -> i32 {
    status(|| {
        let visitor = visitor.ok_or(2)?;
        // SAFETY: caller guarantees a readable NUL-terminated query string.
        let row_status = unsafe { input(row_status)? };
        let rows = with_registry(registry(), |registry| {
            Ok(registry
                .stores
                .get(&handle)
                .ok_or(1)?
                .query_status(&row_status))
        })?;
        // The snapshot owns its strings; no lock or core reference crosses into
        // foreign code, allowing the visitor to perform a bounded nested read.
        for row in rows {
            // SAFETY: row bytes live through this synchronous callback; the
            // caller guarantees callback/context validity and no foreign unwind.
            unsafe {
                visitor(
                    context,
                    row.id.as_ptr(),
                    row.id.len(),
                    row.status.as_ptr(),
                    row.status.len(),
                    row.total_cents,
                    u8::from(row.paid),
                );
            }
        }
        Ok(())
    })
}

pub type EventVisitor = unsafe extern "C" fn(*mut c_void, u64, usize);

/// # Safety
/// The status must satisfy the existing readable input string contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoya_observe(handle: u64, row_status: *const c_char) -> u64 {
    boundary(0, || {
        // SAFETY: caller upholds the readable input contract.
        let Ok(row_status) = (unsafe { input(row_status) }) else {
            return 0;
        };
        with_registry(registry(), |registry| {
            Ok(registry
                .stores
                .get_mut(&handle)
                .ok_or(1)?
                .observe(&row_status)
                .unwrap_or(0))
        })
        .unwrap_or(0)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn hoya_unobserve(handle: u64, subscription: u64) -> i32 {
    status(|| {
        with_registry(registry(), |registry| {
            if registry
                .stores
                .get_mut(&handle)
                .ok_or(1)?
                .unobserve(subscription)
            {
                Ok(())
            } else {
                Err(1)
            }
        })
    })
}

/// # Safety
/// Visitor/context must remain valid for this call; visitor must not unwind.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoya_drain(
    handle: u64,
    visitor: Option<EventVisitor>,
    context: *mut c_void,
) -> i32 {
    status(|| {
        let visitor = visitor.ok_or(2)?;
        let events = with_registry(registry(), |registry| {
            Ok(registry
                .stores
                .get_mut(&handle)
                .ok_or(1)?
                .take_notifications())
        })?;
        for event in events {
            // A previous callback may have closed this store or unsubscribed.
            let active = with_registry(registry(), |registry| {
                Ok(registry
                    .stores
                    .get(&handle)
                    .is_some_and(|store| store.is_observing(event.subscription)))
            })?;
            if active {
                // SAFETY: caller owns a valid context/callback for this drain;
                // the registry guard is gone before invoking foreign code.
                unsafe {
                    visitor(context, event.subscription, event.count);
                }
            }
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn hoya_begin_scope(handle: u64) -> u64 {
    boundary(0, || {
        with_registry(registry(), |registry| {
            Ok(registry
                .stores
                .get_mut(&handle)
                .ok_or(1)?
                .begin_scope()
                .unwrap_or(0))
        })
        .unwrap_or(0)
    })
}

/// # Safety
/// Input strings must satisfy the same contract as hoya_insert.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoya_scope_insert(
    handle: u64,
    scope: u64,
    id: *const c_char,
    row_status: *const c_char,
    total: i64,
    paid: u8,
) -> i32 {
    status(|| {
        if paid > 1 {
            return Err(2);
        }
        // SAFETY: caller upholds the input buffer contract for both strings.
        let id = unsafe { input(id)? };
        // SAFETY: same lifetime and validity contract as the ID.
        let row_status = unsafe { input(row_status)? };
        with_registry(registry(), |registry| {
            let row = Order {
                id,
                status: row_status,
                total_cents: total,
                paid: paid == 1,
            };
            if registry
                .stores
                .get_mut(&handle)
                .ok_or(1)?
                .scope_insert(scope, row)
            {
                Ok(())
            } else {
                Err(1)
            }
        })
    })
}

/// # Safety
/// Input string, callback/context must satisfy the hoya_query contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hoya_scope_query(
    handle: u64,
    scope: u64,
    row_status: *const c_char,
    visitor: Option<RowVisitor>,
    context: *mut c_void,
) -> i32 {
    status(|| {
        let visitor = visitor.ok_or(2)?;
        // SAFETY: caller upholds the readable query string contract.
        let row_status = unsafe { input(row_status)? };
        let rows = with_registry(registry(), |registry| {
            registry
                .stores
                .get(&handle)
                .ok_or(1)?
                .scope_query(scope, &row_status)
                .ok_or(1)
        })?;
        for row in rows {
            // SAFETY: snapshot bytes live through the call; the callback and
            // context satisfy the same contract as an ordinary query visitor.
            unsafe {
                visitor(
                    context,
                    row.id.as_ptr(),
                    row.id.len(),
                    row.status.as_ptr(),
                    row.status.len(),
                    row.total_cents,
                    u8::from(row.paid),
                );
            }
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn hoya_finish_scope(handle: u64, scope: u64, commit: u8) -> i32 {
    status(|| {
        if commit > 1 {
            return Err(2);
        }
        with_registry(registry(), |registry| {
            registry
                .stores
                .get_mut(&handle)
                .ok_or(1)?
                .finish(scope, commit == 1)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn n06_catches_unwinding_panic() {
        assert_eq!(boundary(3, || panic!("synthetic boundary failure")), 3);
    }

    #[test]
    fn n06_poison_maps_to_error() {
        let registry = Mutex::new(Registry {
            next: 1,
            stores: HashMap::new(),
        });
        let _ = std::panic::catch_unwind(|| {
            let _lock = registry.lock().unwrap();
            panic!("synthetic poison");
        });
        assert_eq!(with_registry(&registry, |_| Ok(())), Err(3));
    }

    #[test]
    fn n06_exhaustion_does_not_reuse_handle() {
        let mut registry = Registry {
            next: u64::MAX,
            stores: HashMap::new(),
        };
        assert_eq!(registry.open(), 0);
        assert!(registry.stores.is_empty());
        assert_eq!(registry.next, u64::MAX);
    }
}
