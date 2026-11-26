//! SPEC-0037 R11: explicit offline ledger conversion tooling.
//!
//! Compiled only with the non-default `ledger-convert-tool` feature on native
//! targets. No ordinary owner, the facade or an SDK calls this module; the
//! `hoyadb-convert-ledger` binary is its only consumer. It exposes no raw
//! connection and never relaxes the ordinary `apply_sync_plan` validator: the
//! only ledger change it applies is a genuine [`LedgerConversion`] proof whose
//! private fields the pure core helper alone can construct.
use super::{EncryptionKey, backend, lifecycle, sync, validate_private_format};
use hoyadb_core::{
    command::{
        convert::{ConversionReport, LedgerConversion, reencode},
        records::SyncLedger,
    },
    storage::{ModelState, Row, StorageError, check_sync_rows, validate_sync_model},
};
use rusqlite::{Connection, ErrorCode, OpenFlags, TransactionBehavior, ffi};
use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::Duration,
};
use zeroize::Zeroizing;

pub enum SourceProtection {
    Unprotected,
    KeyFile(PathBuf),
}

pub enum ConversionMode {
    DryRun,
    Backup(PathBuf),
}

pub struct ConvertOptions {
    pub store: PathBuf,
    pub to: u8,
    pub protection: SourceProtection,
    pub mode: ConversionMode,
}

/// Static, redacted outcomes. Variants never carry paths, keys, rows or packets.
#[derive(Debug, PartialEq, Eq)]
pub enum ConvertError {
    /// Exit 2: malformed arguments, missing/empty/nonregular/invalid/unsupported source,
    /// recovery-required source, missing entity ledger, target already present or a
    /// backup path that is one of the store's own `-journal`/`-wal`/`-shm` sidecars.
    InvalidSource,
    /// Exit 3: a live owner, writer or reader holds a conflicting lease.
    StoreInUse,
    /// Exit 4: the key file could not be read as exactly 32 bytes.
    KeyFile,
    /// Exit 5: protected open or authentication failed (wrong key or corrupt pages).
    ProtectedOpen,
    /// Exit 6: the requested backup path already exists; it is never overwritten.
    BackupExists,
    /// Exit 7: backup preparation, revalidation, write or commit failed.
    Failed {
        incomplete_backup: bool,
    },
}

impl ConvertError {
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::InvalidSource => 2,
            Self::StoreInUse => 3,
            Self::KeyFile => 4,
            Self::ProtectedOpen => 5,
            Self::BackupExists => 6,
            Self::Failed {
                ..
            } => 7,
        }
    }
}

impl fmt::Display for ConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidSource => "unsupported or invalid source or arguments",
            Self::StoreInUse => "store is in use",
            Self::KeyFile => "key file must contain exactly 32 bytes",
            Self::ProtectedOpen => "protected storage open failed",
            Self::BackupExists => "backup already exists",
            Self::Failed {
                incomplete_backup: false,
            } => "backup or conversion failed",
            Self::Failed {
                incomplete_backup: true,
            } => "backup or conversion failed; incomplete backup may remain at the requested path",
        })
    }
}

impl std::error::Error for ConvertError {}

mod stage {
    /// Deterministic lifecycle points for process fixtures. Carries no data.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Stage {
        ReadOnlyValidated,
        ReadWriteReserved,
        ReadOnlyClosed,
        BeforeBackupCreate,
        BackupCreated,
        BeforeBackupFileSync,
        BackupFileSynced,
        BackupVerified,
        BeforeBackupDirectorySync,
        BackupDirectorySynced,
        BeforeSourceWrite,
        BeforeCommit,
        BeforeCleanup,
    }
}
use stage::Stage;

type Observer<'a> = dyn FnMut(Stage) -> io::Result<()> + 'a;

/// Runs the offline converter exactly as the binary does.
pub fn run(options: ConvertOptions) -> Result<ConversionReport, ConvertError> {
    run_inner(options, &mut |_| Ok(()))
}

/// Non-default process-fixture controls. Not compiled into the ordinary tool.
#[cfg(feature = "test-hooks")]
#[doc(hidden)]
pub mod testing {
    pub use super::stage::Stage;
    use super::*;

    /// Runs the converter and reports each lifecycle stage. An observer error at
    /// a `Before*` stage is treated as that step failing, exactly like the
    /// corresponding I/O failure; everywhere else it aborts the conversion.
    pub fn run_with_observer(
        options: ConvertOptions,
        observer: &mut dyn FnMut(Stage) -> io::Result<()>,
    ) -> Result<ConversionReport, ConvertError> {
        run_inner(options, observer)
    }
}

const FAILED: ConvertError = ConvertError::Failed {
    incomplete_backup: false,
};

fn run_inner(
    options: ConvertOptions,
    observe: &mut Observer<'_>,
) -> Result<ConversionReport, ConvertError> {
    let ConvertOptions {
        store,
        to,
        protection,
        mode,
    } = options;
    if !matches!(to, 2 | 3) {
        return Err(ConvertError::InvalidSource);
    }
    let key = match protection {
        SourceProtection::Unprotected => None,
        SourceProtection::KeyFile(path) => Some(read_key(&path)?),
    };
    precheck_store(&store)?;
    // The backup is copied through this descriptor. POSIX releases every lock
    // this process holds on a file when any descriptor of it closes, so it is
    // declared before the connections and outlives both: closing it earlier
    // would silently drop the RESERVED lease before the source write.
    let mut source = File::open(&store).map_err(|_| ConvertError::InvalidSource)?;
    let read_only = open_read_only(&store, key.as_deref())?;
    let (source_rows, model) = validate_source(&read_only)?;
    let conversion = reencode(&source_rows, &model, to).map_err(|_| ConvertError::InvalidSource)?;
    observe(Stage::ReadOnlyValidated).map_err(|_| FAILED)?;
    let backup = match mode {
        ConversionMode::DryRun => {
            drop(read_only);
            return Ok(*conversion.report());
        }
        ConversionMode::Backup(backup) => backup,
    };
    if fs::symlink_metadata(&backup).is_ok() {
        return Err(ConvertError::BackupExists);
    }
    // Keep the validated read lease while admitting the writer: a raw writer
    // cannot commit under our SHARED lock, and RESERVED excludes other writers.
    let mut read_write = open_read_write(&store, key.as_deref())?;
    drop(key);
    let transaction = read_write
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| classify_lease(&error))?;
    observe(Stage::ReadWriteReserved).map_err(|_| FAILED)?;
    drop(read_only);
    observe(Stage::ReadOnlyClosed).map_err(|_| FAILED)?;
    revalidate(&transaction, &conversion)?;
    observe(Stage::BeforeBackupCreate).map_err(|_| FAILED)?;
    write_backup(&mut source, &store, &backup, observe)?;
    observe(Stage::BeforeSourceWrite).map_err(|_| FAILED)?;
    apply_conversion_in(&transaction, &conversion).map_err(|_| FAILED)?;
    observe(Stage::BeforeCommit).map_err(|_| FAILED)?;
    // A failed COMMIT (for example a late reader holding SHARED) consumes the
    // guard, which rolls back; the verified backup is retained either way.
    transaction.commit().map_err(|_| FAILED)?;
    Ok(*conversion.report())
}

/// Reads exactly 32 key bytes with a bounded read that detects an extra byte.
fn read_key(path: &Path) -> Result<Zeroizing<[u8; 32]>, ConvertError> {
    let mut file = File::open(path).map_err(|_| ConvertError::KeyFile)?;
    let mut buffer = Zeroizing::new([0_u8; 33]);
    let mut filled = 0;
    while filled < buffer.len() {
        match file.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return Err(ConvertError::KeyFile),
        }
    }
    if filled != 32 {
        return Err(ConvertError::KeyFile);
    }
    let mut key = Zeroizing::new([0_u8; 32]);
    key.copy_from_slice(&buffer[..32]);
    Ok(key)
}

/// No-follow precheck: an existing nonempty regular file at the canonical path.
/// This and SQLite's own path-based open are separate operations (SPEC-0037 R11).
fn precheck_store(store: &Path) -> Result<(), ConvertError> {
    let bytes = store.as_os_str().as_encoded_bytes();
    if bytes.is_empty()
        || bytes.contains(&0)
        || bytes.starts_with(b"file:")
        || store == Path::new(":memory:")
    {
        return Err(ConvertError::InvalidSource);
    }
    let metadata = fs::symlink_metadata(store).map_err(|_| ConvertError::InvalidSource)?;
    if !metadata.file_type().is_file() || metadata.len() == 0 {
        return Err(ConvertError::InvalidSource);
    }
    Ok(())
}

fn configure(connection: &Connection, key: Option<&[u8; 32]>) -> Result<(), ConvertError> {
    if let Some(key) = key {
        // Keying is the first SQL on this connection, exactly as the ordinary opener.
        super::encryption::configure(connection, EncryptionKey::from_bytes(*key))
            .map_err(|_| ConvertError::ProtectedOpen)?;
    }
    connection
        .execute_batch("PRAGMA temp_store=MEMORY")
        .map_err(|_| ConvertError::InvalidSource)?;
    connection
        .busy_timeout(Duration::ZERO)
        .map_err(|_| ConvertError::InvalidSource)?;
    Ok(())
}

/// READ_ONLY, no CREATE or URI, key first, busy_timeout 0, EXCLUSIVE locking mode,
/// then a deferred transaction whose first `sqlite_schema` read acquires and
/// retains SHARED. The read-only pager refuses hot-journal recovery and
/// persisted WAL without opening or creating sidecars (SI-X08).
fn open_read_only(store: &Path, key: Option<&[u8; 32]>) -> Result<Connection, ConvertError> {
    let connection = Connection::open_with_flags(store, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| ConvertError::InvalidSource)?;
    configure(&connection, key)?;
    connection
        .execute_batch("PRAGMA locking_mode=EXCLUSIVE; BEGIN;")
        .map_err(|_| ConvertError::InvalidSource)?;
    match connection.query_row("SELECT count(*) FROM sqlite_schema", [], |row| {
        row.get::<_, i64>(0)
    }) {
        Ok(_) => Ok(connection),
        Err(error) => Err(classify_first_read(&connection, &error, key.is_some())),
    }
}

/// Maps the first read's engine result to the R11 exit classes. A keyed
/// IOERR_LOCK (persisted WAL refused before admission) is classified on this
/// same connection by the operator-approved full-file integrity check.
fn classify_first_read(
    connection: &Connection,
    error: &rusqlite::Error,
    keyed: bool,
) -> ConvertError {
    let Some(failure) = error.sqlite_error() else {
        return ConvertError::InvalidSource;
    };
    match failure.code {
        ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => ConvertError::StoreInUse,
        ErrorCode::SystemIoFailure if failure.extended_code == ffi::SQLITE_IOERR_LOCK && keyed => {
            classify_protected_lock_refusal(connection)
        }
        ErrorCode::NotADatabase if keyed => ConvertError::ProtectedOpen,
        _ => ConvertError::InvalidSource,
    }
}

/// `PRAGMA cipher_integrity_check` on the still-held read-only connection reads
/// the complete main file and returns one row per failing page. Rows mean the
/// key does not authenticate this file (exit 5); zero rows mean a correctly
/// keyed but unsupported WAL source (exit 2). Neither outcome admits WAL or
/// conversion, SQL success alone proves nothing, and any pragma error fails
/// closed as a protected-open failure. No result text is retained or printed.
fn classify_protected_lock_refusal(connection: &Connection) -> ConvertError {
    let rows = (|| -> rusqlite::Result<usize> {
        let mut statement = connection.prepare("PRAGMA cipher_integrity_check")?;
        let mut cursor = statement.query([])?;
        let mut count = 0;
        while cursor.next()?.is_some() {
            count += 1;
        }
        Ok(count)
    })();
    match rows {
        Ok(0) => ConvertError::InvalidSource,
        Ok(_) | Err(_) => ConvertError::ProtectedOpen,
    }
}

fn classify_lease(error: &rusqlite::Error) -> ConvertError {
    match error.sqlite_error_code() {
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => ConvertError::StoreInUse,
        _ => FAILED,
    }
}

/// Strict private format, catalog and recorded model, then the complete ledger.
/// No schema initialization, index installation or application-row validation.
fn validate_source(connection: &Connection) -> Result<(Vec<Row>, ModelState), ConvertError> {
    let invalid = |_| ConvertError::InvalidSource;
    validate_private_format(connection).map_err(invalid)?;
    let model = lifecycle::catalog(connection)
        .map_err(invalid)?
        .state
        .ok_or(ConvertError::InvalidSource)?;
    let rows = sync::load(connection)
        .map_err(invalid)?
        .ok_or(ConvertError::InvalidSource)?;
    Ok((rows, model))
}

/// READ_WRITE without CREATE, key first, busy_timeout 0. The caller acquires
/// RESERVED with `BEGIN IMMEDIATE` while the read-only lease is still held.
fn open_read_write(store: &Path, key: Option<&[u8; 32]>) -> Result<Connection, ConvertError> {
    let connection = Connection::open_with_flags(store, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|_| FAILED)?;
    configure(&connection, key)?;
    Ok(connection)
}

/// Inside the RESERVED transaction, the store must still be exactly the
/// validated source: format, model and every recovery packet.
fn revalidate(connection: &Connection, conversion: &LedgerConversion) -> Result<(), ConvertError> {
    let (rows, model) = validate_source(connection).map_err(|_| FAILED)?;
    if &model != conversion.model() {
        return Err(FAILED);
    }
    check_sync_rows(Some(&rows), Some(conversion.source_rows())).map_err(|_| FAILED)?;
    Ok(())
}

/// Device and inode captured immediately after `create_new`; cleanup removes a
/// file only while the path still resolves to this identity.
struct BackupIdentity {
    device: u64,
    inode: u64,
}

/// The rollback-journal and WAL family names SQLite derives from the store path.
const SIDECAR_SUFFIXES: [&str; 3] = ["-journal", "-wal", "-shm"];

/// Whether the just-created backup is the file one of the store's sidecar paths
/// resolves to. Identity, not spelling: this also covers case-insensitive
/// volumes and aliased directories.
fn is_store_sidecar(store: &Path, identity: &BackupIdentity) -> bool {
    SIDECAR_SUFFIXES.iter().any(|suffix| {
        let mut path = store.as_os_str().to_owned();
        path.push(suffix);
        fs::metadata(&path).is_ok_and(|metadata| {
            metadata.dev() == identity.device && metadata.ino() == identity.inode
        })
    })
}

/// `create_new` the backup, copy the exact main-file bytes before any source
/// write, sync the file, reread and verify, then fsync the parent directory.
/// The bytes come from the caller's long-lived `source` descriptor, which this
/// function never closes. The backup is created no more permissive than the
/// store, and a path that is one of the store's own sidecars is refused.
/// Any failure removes only this invocation's incomplete backup.
fn write_backup(
    source: &mut File,
    store: &Path,
    backup: &Path,
    observe: &mut Observer<'_>,
) -> Result<(), ConvertError> {
    let mode = source.metadata().map_err(|_| FAILED)?.mode() & 0o777;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(backup)
        .map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                ConvertError::BackupExists
            } else {
                FAILED
            }
        })?;
    let identity = match file.metadata() {
        Ok(metadata) => BackupIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        },
        // Without a captured identity nothing may be deleted; disclose the leftover.
        Err(_) => {
            return Err(ConvertError::Failed {
                incomplete_backup: true,
            });
        }
    };
    if is_store_sidecar(store, &identity) {
        // SQLite would reuse and then delete this path: malformed arguments.
        drop(file);
        return Err(match cleanup_incomplete_backup(backup, &identity) {
            FAILED => ConvertError::InvalidSource,
            leftover => leftover,
        });
    }
    let result = (|| -> io::Result<()> {
        observe(Stage::BackupCreated)?;
        let mut bytes = Vec::new();
        source.seek(SeekFrom::Start(0))?;
        source.read_to_end(&mut bytes)?;
        file.write_all(&bytes)?;
        observe(Stage::BeforeBackupFileSync)?;
        file.sync_all()?;
        observe(Stage::BackupFileSynced)?;
        if fs::read(backup)? != bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "backup verification failed",
            ));
        }
        observe(Stage::BackupVerified)?;
        observe(Stage::BeforeBackupDirectorySync)?;
        let parent = backup
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        File::open(parent)?.sync_all()?;
        observe(Stage::BackupDirectorySynced)?;
        Ok(())
    })();
    drop(file);
    match result {
        Ok(()) => Ok(()),
        Err(_) => {
            // Fixtures may swap the path here; identity is rechecked before removal.
            let _ = observe(Stage::BeforeCleanup);
            Err(cleanup_incomplete_backup(backup, &identity))
        }
    }
}

fn cleanup_incomplete_backup(backup: &Path, identity: &BackupIdentity) -> ConvertError {
    let leftover = ConvertError::Failed {
        incomplete_backup: true,
    };
    match fs::symlink_metadata(backup) {
        Ok(metadata)
            if metadata.file_type().is_file()
                && metadata.dev() == identity.device
                && metadata.ino() == identity.inode =>
        {
            match fs::remove_file(backup) {
                Ok(()) => FAILED,
                Err(_) => leftover,
            }
        }
        // A different file now occupies the path: never delete it.
        Ok(_) => leftover,
        Err(error) if error.kind() == io::ErrorKind::NotFound => FAILED,
        Err(_) => leftover,
    }
}

/// Applies a genuine conversion proof inside the caller's RESERVED transaction.
/// Defense in depth re-derives the marker-only property from the proof's own
/// packets, re-checks the recorded model and every stored packet, then rewrites
/// only the recovery inventory through the shared full-plan helper. Application
/// rows and the model row are never touched.
fn apply_conversion_in(
    connection: &Connection,
    conversion: &LedgerConversion,
) -> Result<(), StorageError> {
    let before = SyncLedger::from_rows(conversion.source_rows())?;
    let after = SyncLedger::from_rows(conversion.rows())?;
    let mut normalized = after.clone();
    match (&before.entity, normalized.entity.as_mut()) {
        (Some(source), Some(target)) if source.ledger_format != target.ledger_format => {
            target.ledger_format = source.ledger_format;
        }
        _ => {
            return Err(backend(
                "conversion proof does not change the ledger format",
            ));
        }
    }
    if normalized != before {
        return Err(backend(
            "conversion proof changes more than the ledger format",
        ));
    }
    validate_sync_model(&after, conversion.model())?;
    if lifecycle::catalog(connection)?.state.as_ref() != Some(conversion.model()) {
        return Err(backend("command model changed before conversion"));
    }
    let actual = sync::load(connection)?;
    check_sync_rows(actual.as_deref(), Some(conversion.source_rows()))?;
    sync::rewrite_sync_rows(connection, actual.is_some(), conversion.rows().to_vec())?;
    lifecycle::catalog(connection)?;
    Ok(())
}
