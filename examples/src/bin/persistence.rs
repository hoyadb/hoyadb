use hoyadb_doc_examples::*;
use hoyadb_storage_sqlite::{EncryptionKey, SqliteOpenOptions, SqliteStorage};

fn main() {
    let directory = std::env::temp_dir().join(format!(
        "hoyadb-public-docs-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("fixture.sqlite");
    // docs:start key
    // Throwaway test fixture only. Production obtains a secret from its host key store.
    let options = || SqliteOpenOptions {
        encryption_key: Some(EncryptionKey::from_bytes([0x71; 32])),
    };
    // docs:end key
    // docs:start open
    let storage = SqliteStorage::open_with_options(&path, options()).unwrap();
    // docs:end open
    assert!(storage.storage_protection().encrypted);
    // docs:start write
    let mut db = run(Database::open(storage, schema(), vec![])).unwrap();
    let note = run(db.insert("notes", fields("Survives reopen"))).unwrap();
    drop(db);
    // docs:end write
    // docs:start reopen
    let storage = SqliteStorage::open_with_options(&path, options()).unwrap();
    let db = run(Database::open(storage, schema(), vec![])).unwrap();
    // docs:end reopen
    assert_eq!(run(db.query("notes").fetch(&db)).unwrap(), vec![note]);
    drop(db);
    assert!(
        SqliteStorage::open_with_options(
            &path,
            SqliteOpenOptions {
                encryption_key: Some(EncryptionKey::from_bytes([0x72; 32])),
            }
        )
        .is_err()
    );
    std::fs::remove_dir_all(directory).unwrap();
    println!("PASS protected SQLite: supplied key, write, reopen and wrong-key refusal");
}
