//! TC-0035 S06: a scoped supplied-key factory reuses the unchanged shared cases.
use super::*;
use hoyadb_storage_sqlite::{EncryptionKey, SqliteOpenOptions};
use zeroize::Zeroizing;

thread_local! {
    static KEY: RefCell<Option<Zeroizing<[u8; 32]>>> = const { RefCell::new(None) };
}

struct KeyScope;
impl Drop for KeyScope {
    fn drop(&mut self) {
        KEY.with(|key| drop(key.borrow_mut().take()));
    }
}

pub(super) fn fixture_open(
    path: impl AsRef<std::path::Path>,
) -> Result<SqliteStorage, StorageError> {
    KEY.with(|key| match key.borrow().as_ref() {
        None => SqliteStorage::open_unprotected(path),
        Some(key) => {
            let storage = SqliteStorage::open_with_options(
                path,
                SqliteOpenOptions {
                    encryption_key: Some(EncryptionKey::from_bytes(**key)),
                },
            )?;
            let protection = storage.storage_protection();
            assert!(protection.encrypted);
            assert_eq!(protection.cipher, "SQLCipher");
            assert_eq!(protection.version, "4.19.0");
            Ok(storage)
        }
    })
}

#[wasm_bindgen]
pub fn security_conformance(group: &str, index: u32, key_hex: &str) -> Result<JsValue, JsValue> {
    if !["typed", "model", "reactive", "review", "command", "entity"].contains(&group)
        || key_hex.len() != 64
        || !key_hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(backend("invalid security test control"));
    }
    let mut decoded = Zeroizing::new([0u8; 32]);
    for (n, cell) in key_hex.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let digit = |b: u8| {
            if b <= b'9' {
                b - b'0'
            } else {
                b - b'a' + 10
            }
        };
        decoded[n] = digit(cell[0]) * 16 + digit(cell[1]);
    }
    KEY.with(|key| {
        let mut key = key.borrow_mut();
        if key.is_some() {
            return Err(backend("nested security test control"));
        }
        *key = Some(decoded);
        Ok(())
    })?;
    // Every normal/error return clears our owned key. A WASM panic aborts this
    // worker; its owner must terminate it, rather than continuing a partial case.
    let _scope = KeyScope;
    match group {
        "typed" if index == 0 => Ok(typed_conformance().into()),
        "model" if index == 0 => Ok(model_conformance().into()),
        "reactive" if index == 0 => Ok(reactive_conformance().into()),
        "review" => model_evidence::review_conformance(index),
        "command" => command_conformance(index),
        "entity" => entity_conformance(index),
        _ => Err(backend("invalid security index")),
    }
}
