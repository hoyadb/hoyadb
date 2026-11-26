//! API-0010 / TC-0035: supplied-key ownership below generic storage traits.
use std::fmt;
use zeroize::Zeroizing;

pub struct EncryptionKey(pub(super) Zeroizing<[u8; 32]>);

impl EncryptionKey {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(bytes))
    }
}

impl fmt::Debug for EncryptionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EncryptionKey { .. }")
    }
}

#[derive(Debug, Default)]
pub struct SqliteOpenOptions {
    pub encryption_key: Option<EncryptionKey>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StorageProtection {
    pub encrypted: bool,
    pub cipher: &'static str,
    pub version: &'static str,
}

// No raw key accessor escapes the adapter. SQL text and our owned input buffer
// are wiped on every return path; foreign SQLite/parser copies remain its concern.
pub(super) fn configure(
    connection: &rusqlite::Connection,
    key: EncryptionKey,
) -> Result<StorageProtection, hoyadb_core::storage::StorageError> {
    use std::fmt::Write;
    let mut sql = Zeroizing::new(String::with_capacity(84));
    sql.push_str("PRAGMA key = \"x'");
    for byte in key.0.iter() {
        write!(&mut *sql, "{byte:02x}").expect("writing to an owned String");
    }
    sql.push_str("'\";");
    connection
        .execute_batch(&sql)
        .map_err(|_| super::backend("cipher key rejected"))?;
    drop(sql);
    drop(key);
    connection
        .execute_batch("PRAGMA cipher_compatibility=4; PRAGMA cipher_log_level=NONE;")
        .map_err(|_| super::backend("cipher configuration failed"))?;
    let read = |pragma: &str| {
        connection
            .query_row(pragma, [], |row| row.get::<_, String>(0))
            .map_err(|_| super::backend("cipher capability unavailable"))
    };
    let version = read("PRAGMA cipher_version")?;
    if version.split_whitespace().next() != Some("4.19.0")
        || !read("PRAGMA cipher")?.eq_ignore_ascii_case("aes-256-cbc")
        || read("PRAGMA cipher_page_size")? != "4096"
        || read("PRAGMA cipher_use_hmac")? != "1"
        || read("PRAGMA cipher_hmac_algorithm")? != "HMAC_SHA512"
        || read("PRAGMA cipher_kdf_algorithm")? != "PBKDF2_HMAC_SHA512"
        || read("PRAGMA cipher_plaintext_header_size")? != "0"
        || read("PRAGMA cipher_status")? != "1"
    {
        return Err(super::backend("cipher profile unavailable"));
    }
    Ok(StorageProtection {
        encrypted: true,
        cipher: "SQLCipher",
        version: "4.19.0",
    })
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::SqliteStorage;

    #[test]
    fn s01_s02_s05_options_key_debug_and_actual_temp_profile() {
        struct Uninspected;
        impl AsRef<std::path::Path> for Uninspected {
            fn as_ref(&self) -> &std::path::Path {
                panic!("missing options key must precede path inspection")
            }
        }
        assert!(
            SqliteStorage::open_with_options(Uninspected, SqliteOpenOptions::default()).is_err()
        );
        let options = SqliteOpenOptions {
            encryption_key: Some(EncryptionKey::from_bytes([0x61; 32])),
        };
        assert_eq!(
            format!("{options:?}"),
            "SqliteOpenOptions { encryption_key: Some(EncryptionKey { .. }) }"
        );
        let root = std::env::temp_dir().join(format!("hoya-cipher-profile-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let store = SqliteStorage::open_with_options(root.join("store.sqlite"), options).unwrap();
        assert_eq!(
            store
                .connection
                .query_row("PRAGMA temp_store", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            store
                .connection
                .query_row("PRAGMA cipher_page_size", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "4096"
        );
        assert_eq!(
            store
                .connection
                .query_row("PRAGMA cipher_use_hmac", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "1"
        );
        assert_eq!(
            store
                .connection
                .query_row("PRAGMA cipher_plaintext_header_size", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            "0"
        );
        assert_eq!(
            store
                .connection
                .query_row("PRAGMA cipher_hmac_algorithm", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            "HMAC_SHA512"
        );
        assert_eq!(
            store
                .connection
                .query_row("PRAGMA cipher_kdf_algorithm", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            "PBKDF2_HMAC_SHA512"
        );
        let provider: String = store
            .connection
            .query_row("PRAGMA cipher_provider", [], |row| row.get(0))
            .unwrap();
        let provider_version: String = store
            .connection
            .query_row("PRAGMA cipher_provider_version", [], |row| row.get(0))
            .unwrap();
        #[cfg(target_os = "macos")]
        assert_eq!(provider, "commoncrypto");
        #[cfg(target_os = "linux")]
        assert_eq!(provider, "openssl");
        eprintln!("verified cipher provider: {provider}; version: {provider_version}");
        assert_eq!(
            store.storage_protection(),
            StorageProtection {
                encrypted: true,
                cipher: "SQLCipher",
                version: "4.19.0"
            }
        );
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }
}
