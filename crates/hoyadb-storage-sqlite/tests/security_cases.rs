//! TC-0035: fail-closed supplied-key storage boundaries.
use hoyadb_storage_sqlite::SqliteStorage;

#[test]
fn s01_default_missing_key_never_creates_a_file() {
    let directory =
        std::env::temp_dir().join(format!("hoya-security-default-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("must-not-exist.sqlite");
    let failed = SqliteStorage::open(&path).is_err();
    let touched = path.exists();
    std::fs::remove_dir_all(directory).unwrap();
    assert!(
        failed,
        "protected default must reject missing application key"
    );
    assert!(
        !touched,
        "missing-key rejection must precede filesystem creation"
    );
}

#[test]
fn s01_missing_key_preserves_existing_bytes_and_never_resolves_the_path() {
    let directory =
        std::env::temp_dir().join(format!("hoya-security-existing-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("existing.sqlite");
    let bytes = b"synthetic existing file must remain exactly unchanged";
    std::fs::write(&path, bytes).unwrap();
    assert!(SqliteStorage::open(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    struct Uninspected;
    impl AsRef<std::path::Path> for Uninspected {
        fn as_ref(&self) -> &std::path::Path {
            panic!("missing key must precede path inspection")
        }
    }
    assert!(SqliteStorage::open(Uninspected).is_err());
    std::fs::remove_dir_all(directory).unwrap();
}
