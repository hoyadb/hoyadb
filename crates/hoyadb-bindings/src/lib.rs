//! API-0011 / TC-0036: one general-schema boundary over the existing core.
mod budget;
mod codec;
mod commands;
mod data;
mod engine;
mod entities;
mod observations;
mod reconciliation;

use codec::*;
use data::*;
use engine::Engine;
use hoyadb_core::{
    command::{CommandDatabase, CommandError, Json, JsonError},
    storage::StorageError,
};
use hoyadb_storage_memory::MemoryStorage;
use hoyadb_storage_sqlite::{EncryptionKey, SqliteOpenOptions, SqliteStorage};
use std::{
    collections::BTreeMap,
    future::Future,
    sync::atomic::{AtomicU64, Ordering},
    task::{Context, Poll, Waker},
};

type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug)]
struct Error(&'static str);

impl From<StorageError> for Error {
    fn from(value: StorageError) -> Self {
        Self(match value {
            StorageError::MigrationFailure {
                ..
            } => "MigrationFailure",
            StorageError::SchemaConflict {
                ..
            } => "SchemaConflict",
            StorageError::InvalidSchema {
                ..
            } => "InvalidSchema",
            StorageError::InvalidQuery {
                ..
            } => "InvalidQuery",
            StorageError::InvalidRow {
                ..
            } => "InvalidRow",
            StorageError::BackendFailure {
                ..
            } => "BackendFailure",
            StorageError::TableNotFound {
                ..
            } => "TableNotFound",
            StorageError::RowAlreadyExists {
                ..
            } => "RowAlreadyExists",
            StorageError::RowNotFound {
                ..
            } => "RowNotFound",
            StorageError::StoreInUse => "Busy",
        })
    }
}

impl From<CommandError> for Error {
    fn from(value: CommandError) -> Self {
        Self(match value {
            CommandError::Storage(error) => return error.into(),
            CommandError::InvalidCommand => "InvalidCommand",
            CommandError::InvalidTicket => "InvalidTicket",
            CommandError::Busy => "Busy",
            CommandError::Capacity => "Capacity",
            CommandError::InvalidPayload(_) => "InvalidPayload",
            CommandError::Protocol => "Protocol",
            CommandError::UnknownLocalReference => "UnknownLocalReference",
            CommandError::RollbackConflict => "RollbackConflict",
            CommandError::Closed => "Closed",
            CommandError::LimitExceeded => "LimitExceeded",
            CommandError::AuthorityRequired => "AuthorityRequired",
            CommandError::StalePull => "StalePull",
            CommandError::PullConflict => "PullConflict",
        })
    }
}

impl From<JsonError> for Error {
    fn from(value: JsonError) -> Self {
        Self(if value == JsonError::LimitExceeded {
            "LimitExceeded"
        } else {
            "InvalidPayload"
        })
    }
}

fn run<T, E: Into<Error>>(future: impl Future<Output = std::result::Result<T, E>>) -> Result<T> {
    match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(result) => result.map_err(Into::into),
        Poll::Pending => Err(Error("UnexpectedPending")),
    }
}

fn reply(result: Result<Json>) -> String {
    let value = match result {
        Ok(value) => object([
            ("abi", text("1")),
            ("ok", Json::Bool(true)),
            ("value", value),
        ]),
        Err(error) => object([
            ("abi", text("1")),
            ("ok", Json::Bool(false)),
            ("error", object([("kind", text(error.0))])),
        ]),
    };
    value
        .canonical()
        .unwrap_or_else(|_| r#"{"abi":"1","ok":false,"error":{"kind":"LimitExceeded"}}"#.into())
}

static NEXT: AtomicU64 = AtomicU64::new(1);

fn allocate() -> Result<u64> {
    NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
        next.checked_add(1)
    })
    .map_err(|_| Error("LimitExceeded"))
}

fn handle(value: &Json) -> Result<u64> {
    let value = unsigned(value)?;
    if value == 0 {
        return Err(Error("InvalidHandle"));
    }
    Ok(value)
}

struct Owner {
    db: CommandDatabase<Engine>,
    protection: Json,
    path: Option<std::path::PathBuf>,
    store: Option<u64>,
    requests: BTreeMap<u64, hoyadb_core::command::CommandRequest>,
    push: Option<commands::PendingPush>,
    subscriptions: BTreeMap<u64, observations::Token>,
    events: Option<std::collections::VecDeque<Json>>,
    entities: entities::State,
}

#[derive(Default)]
pub struct Facade {
    owners: BTreeMap<u64, Owner>,
    stores: BTreeMap<u64, Option<MemoryStorage>>,
}

impl Facade {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self, input: &str, key: Vec<u8>) -> String {
        let key = zeroize::Zeroizing::new(key);
        reply(self.open_inner(input, &key))
    }

    pub fn call(&mut self, input: &str) -> String {
        reply(self.call_inner(input))
    }

    pub fn validate_open(&self, input: &str, key_length: usize) -> String {
        reply(Self::checked_open(input, key_length).map(|_| Json::Null))
    }

    fn parse(input: &str) -> Result<Json> {
        let value = Json::parse(input)?;
        wire_only(&value)?;
        let f = exact(&value, &["abi", "op", "owner", "args"])?;
        if string(&f["abi"])? != "1" {
            return Err(Error("InvalidRequest"));
        }
        string(&f["op"])?;
        map(&f["args"])?;
        Ok(value)
    }

    fn checked_open(input: &str, key_length: usize) -> Result<Json> {
        let value = Self::parse(input)?;
        let frame = map(&value)?;
        if string(&frame["op"])? != "open" || frame["owner"] != Json::Null {
            return Err(Error("InvalidRequest"));
        }
        let f = exact(
            &frame["args"],
            &["engine", "schema", "migrations", "namespace", "options"],
        )?;
        schema(&f["schema"])?;
        migrations(&f["migrations"])?;
        options(&f["options"])?;
        let namespace = string(&f["namespace"])?;
        if namespace.len() != 32
            || !namespace
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error("InvalidRequest"));
        }
        let engine = map(&f["engine"])?;
        match string(engine.get("kind").ok_or(Error("InvalidRequest"))?)? {
            "memory" => {
                let e = exact(&f["engine"], &["kind", "store"])?;
                if key_length != 0 {
                    return Err(Error("InvalidRequest"));
                }
                if e["store"] != Json::Null {
                    handle(&e["store"])?;
                }
            }
            "native" => {
                let e = exact(
                    &f["engine"],
                    &["kind", "directory", "name", "backup_excluded"],
                )?;
                if key_length != 32 {
                    return Err(Error("InvalidRequest"));
                }
                string(&e["directory"])?;
                string(&e["name"])?;
                boolean(&e["backup_excluded"])?;
            }
            "opfs" => {
                let e = exact(&f["engine"], &["kind", "name"])?;
                if key_length != 32 {
                    return Err(Error("InvalidRequest"));
                }
                let name = string(&e["name"])?;
                if name.is_empty()
                    || name == ":memory:"
                    || name.starts_with("file:")
                    || name.contains(['/', '\0'])
                {
                    return Err(Error("InvalidRequest"));
                }
            }
            _ => return Err(Error("InvalidRequest")),
        }
        Ok(value)
    }

    fn open_inner(&mut self, input: &str, key: &[u8]) -> Result<Json> {
        let frame = Self::checked_open(input, key.len())?;
        let frame = map(&frame)?;
        if string(&frame["op"])? != "open" || frame["owner"] != Json::Null {
            return Err(Error("InvalidRequest"));
        }
        let f = exact(
            &frame["args"],
            &["engine", "schema", "migrations", "namespace", "options"],
        )?;
        let definition = schema(&f["schema"])?;
        let migrations = migrations(&f["migrations"])?;
        let namespace = string(&f["namespace"])?;
        if namespace.len() != 32
            || !namespace
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error("InvalidRequest"));
        }
        let mut seed = [0; 16];
        for (index, byte) in seed.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&namespace[index * 2..index * 2 + 2], 16)
                .map_err(|_| Error("InvalidRequest"))?;
        }
        let options = options(&f["options"])?;
        let engine = map(&f["engine"])?;
        let kind = string(engine.get("kind").ok_or(Error("InvalidRequest"))?)?;
        let token = allocate()?;
        let mut retained = None;
        let (engine, protection, path) = match kind {
            "memory" => {
                let engine = exact(&f["engine"], &["kind", "store"])?;
                if !key.is_empty() {
                    return Err(Error("InvalidRequest"));
                }
                let storage = if engine["store"] == Json::Null {
                    MemoryStorage::default()
                } else {
                    let store = handle(&engine["store"])?;
                    let storage = self
                        .stores
                        .get(&store)
                        .ok_or(Error("InvalidHandle"))?
                        .as_ref()
                        .ok_or(Error("Busy"))?
                        .clone();
                    retained = Some(store);
                    storage
                };
                (
                    Engine::Memory(storage),
                    object([
                        ("encrypted", Json::Bool(false)),
                        ("cipher", text("none")),
                        ("version", text("not_applicable")),
                        ("backup", text("not_applicable")),
                    ]),
                    None,
                )
            }
            "native" => {
                let engine = exact(
                    &f["engine"],
                    &["kind", "directory", "name", "backup_excluded"],
                )?;
                // Validate the binary key before evaluating any filesystem path.
                if key.len() != 32 {
                    return Err(Error("InvalidRequest"));
                }
                boolean(&engine["backup_excluded"])?;
                let directory = std::path::Path::new(string(&engine["directory"])?);
                let name = string(&engine["name"])?;
                if !directory.is_absolute()
                    || name.is_empty()
                    || name == "."
                    || name == ".."
                    || name.contains(['/', '\\', '\0'])
                {
                    return Err(Error("InvalidRequest"));
                }
                let metadata =
                    std::fs::symlink_metadata(directory).map_err(|_| Error("InvalidRequest"))?;
                let canonical = directory
                    .canonicalize()
                    .map_err(|_| Error("InvalidRequest"))?;
                if !metadata.is_dir() || metadata.is_symlink() || canonical != directory {
                    return Err(Error("InvalidRequest"));
                }
                let path = canonical.join(name);
                if self
                    .owners
                    .values()
                    .any(|owner| owner.path.as_ref() == Some(&path))
                {
                    return Err(Error("Busy"));
                }
                match std::fs::symlink_metadata(&path) {
                    Ok(metadata) if !metadata.is_file() || metadata.is_symlink() => {
                        return Err(Error("InvalidRequest"));
                    }
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => return Err(Error("InvalidRequest")),
                }
                let bytes: [u8; 32] = key.try_into().map_err(|_| Error("InvalidRequest"))?;
                let store = SqliteStorage::open_with_options(
                    &path,
                    SqliteOpenOptions {
                        encryption_key: Some(EncryptionKey::from_bytes(bytes)),
                    },
                )?;
                let status = store.storage_protection();
                let protection = object([
                    ("encrypted", Json::Bool(status.encrypted)),
                    ("cipher", text(status.cipher)),
                    ("version", text(status.version)),
                    ("backup", text("host_managed")),
                ]);
                (Engine::Sqlite(store), protection, Some(path))
            }
            #[cfg(target_arch = "wasm32")]
            "opfs" => {
                let name = string(&engine["name"])?;
                let path = std::path::PathBuf::from(name);
                if self
                    .owners
                    .values()
                    .any(|owner| owner.path.as_ref() == Some(&path))
                {
                    return Err(Error("Busy"));
                }
                let bytes: [u8; 32] = key.try_into().map_err(|_| Error("InvalidRequest"))?;
                let storage = SqliteStorage::open_with_options(
                    &path,
                    SqliteOpenOptions {
                        encryption_key: Some(EncryptionKey::from_bytes(bytes)),
                    },
                )?;
                let status = storage.storage_protection();
                let protection = object([
                    ("encrypted", Json::Bool(status.encrypted)),
                    ("cipher", text(status.cipher)),
                    ("version", text(status.version)),
                    ("backup", text("unsupported")),
                ]);
                (Engine::Sqlite(storage), protection, Some(path))
            }
            _ => return Err(Error("InvalidRequest")),
        };
        let db = run(CommandDatabase::open(
            engine, definition, migrations, seed, options,
        ))?;
        if let Some(store) = retained {
            *self.stores.get_mut(&store).ok_or(Error("InvalidHandle"))? = None;
        }
        self.owners.insert(
            token,
            Owner {
                db,
                protection,
                path,
                store: retained,
                requests: BTreeMap::new(),
                push: None,
                subscriptions: BTreeMap::new(),
                events: None,
                entities: entities::State::default(),
            },
        );
        Ok(text(token))
    }

    fn call_inner(&mut self, input: &str) -> Result<Json> {
        let frame = Self::parse(input)?;
        let frame = map(&frame)?;
        let op = string(&frame["op"])?;
        if matches!(op, "memory_store" | "memory_drop") {
            if frame["owner"] != Json::Null {
                return Err(Error("InvalidRequest"));
            }
            return if op == "memory_store" {
                exact(&frame["args"], &[])?;
                let store = allocate()?;
                self.stores.insert(store, Some(MemoryStorage::default()));
                Ok(text(store))
            } else {
                let f = exact(&frame["args"], &["store"])?;
                let store = handle(&f["store"])?;
                if self
                    .stores
                    .get(&store)
                    .ok_or(Error("InvalidHandle"))?
                    .is_none()
                {
                    return Err(Error("Busy"));
                }
                self.stores.remove(&store);
                Ok(Json::Null)
            };
        }
        if matches!(op, "json_decode" | "json_encode") {
            if frame["owner"] != Json::Null {
                return Err(Error("InvalidRequest"));
            }
            return match op {
                "json_decode" => {
                    let f = exact(&frame["args"], &["body"])?;
                    Ok(boxed(&Json::parse(string(&f["body"])?)?))
                }
                _ => {
                    let f = exact(&frame["args"], &["value"])?;
                    Ok(text(unbox(&f["value"])?.canonical()?))
                }
            };
        }
        let token = handle(&frame["owner"])?;
        if op == "close" {
            exact(&frame["args"], &[])?;
            let owner = self.owners.remove(&token).ok_or(Error("InvalidHandle"))?;
            if let Some(store) = owner.store {
                let Engine::Memory(storage) = owner.db.into_storage() else {
                    return Err(Error("Protocol"));
                };
                self.stores.insert(store, Some(storage));
            } else {
                owner.db.close();
            }
            return Ok(Json::Null);
        }
        let owner = self.owners.get_mut(&token).ok_or(Error("InvalidHandle"))?;
        match op {
            "fetch" => {
                let f = exact(&frame["args"], &["query"])?;
                let q = query(
                    owner.db.schema(),
                    &f["query"],
                    |table, source| match source {
                        Some((table, id, relation)) => owner.db.related(table, id, relation),
                        None => owner.db.query(table),
                    },
                )?;
                Ok(rows_wire(&run(q.fetch(&owner.db))?))
            }
            "insert" => {
                let f = exact(&frame["args"], &["table", "fields", "proposed_id"])?;
                let fields = fields(&f["fields"])?;
                let proposed = proposed(&f["proposed_id"])?;
                budget::row(proposed.as_deref(), &fields)?;
                Ok(row_wire(&run(owner.db.insert_proposed(
                    string(&f["table"])?,
                    fields,
                    proposed,
                ))?))
            }
            "replace" => {
                let f = exact(&frame["args"], &["table", "id", "fields"])?;
                let fields = fields(&f["fields"])?;
                budget::row(Some(string(&f["id"])?), &fields)?;
                Ok(row_wire(&run(owner.db.replace(
                    string(&f["table"])?,
                    string(&f["id"])?,
                    fields,
                ))?))
            }
            "remove" => {
                let f = exact(&frame["args"], &["table", "id"])?;
                Ok(Json::Bool(run(owner
                    .db
                    .remove(string(&f["table"])?, string(&f["id"])?))?))
            }
            "batch" => {
                let f = exact(&frame["args"], &["writes"])?;
                let writes = writes(&f["writes"])?;
                budget::batch(&writes, |table, id| {
                    let q = owner.db.query(table).where_(
                        "id",
                        hoyadb_core::storage::Operator::Eq,
                        hoyadb_core::storage::Value::String(id.into()),
                    );
                    run(q.fetch(&owner.db))
                })?;
                Ok(changes_wire(&run(owner.db.apply_batch_proposed(writes))?))
            }
            "schema" => {
                exact(&frame["args"], &[])?;
                Ok(schema_wire(owner.db.schema()))
            }
            "protection" => {
                exact(&frame["args"], &[])?;
                Ok(owner.protection.clone())
            }
            _ => owner.reconciliation_call(op, &frame["args"]),
        }
    }
}

#[cfg(test)]
mod od08_tests {
    use super::*;

    /// TC-0040 OD08: data notification errors are core StorageErrors, and no
    /// StorageError kind is LimitExceeded, so a data error event is never the
    /// SPEC-0036 terminal form. The exhaustive match fails to compile when a
    /// core variant is added, forcing this invariant to be revisited.
    #[test]
    fn od08_no_storage_error_maps_to_limit_exceeded() {
        let m = || String::from("m");
        let all = [
            StorageError::MigrationFailure {
                message: m(),
            },
            StorageError::SchemaConflict {
                message: m(),
            },
            StorageError::InvalidSchema {
                message: m(),
            },
            StorageError::InvalidQuery {
                message: m(),
            },
            StorageError::InvalidRow {
                table: m(),
                id: m(),
                message: m(),
            },
            StorageError::BackendFailure {
                message: m(),
            },
            StorageError::TableNotFound {
                table: m(),
            },
            StorageError::RowAlreadyExists {
                table: m(),
                id: m(),
            },
            StorageError::RowNotFound {
                table: m(),
                id: m(),
            },
            StorageError::StoreInUse,
        ];
        for error in all {
            match &error {
                StorageError::MigrationFailure {
                    ..
                }
                | StorageError::SchemaConflict {
                    ..
                }
                | StorageError::InvalidSchema {
                    ..
                }
                | StorageError::InvalidQuery {
                    ..
                }
                | StorageError::InvalidRow {
                    ..
                }
                | StorageError::BackendFailure {
                    ..
                }
                | StorageError::TableNotFound {
                    ..
                }
                | StorageError::RowAlreadyExists {
                    ..
                }
                | StorageError::RowNotFound {
                    ..
                }
                | StorageError::StoreInUse => {}
            }
            assert_ne!(Error::from(error).0, "LimitExceeded");
        }
    }
}
