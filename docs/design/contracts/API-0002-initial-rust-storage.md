---
id: API-0002
title: Initial internal Rust storage contract
type: api
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-21
updated: 2026-10-26
parents: [ARC-0001]
satisfies: [REQ-0034, REQ-0038, NFR-0003]
depends_on: [SPEC-0019, SPEC-0020, SPEC-0022, SPEC-0001]
tags: [api, rust, storage, internal]
---

# API-0002: Initial Rust storage contract

This internal foundation is a tested subset of SPEC-0001, not a stable consumer
API or complete engine-conformance claim. Types live in core; adapters depend
on core. No runtime or concrete storage implementation enters core.

```rust
pub enum Value {
    String(String),
    Integer(i64),
    Boolean(bool),
}
pub struct Row {
    pub id: String,
    pub fields: BTreeMap<String, Value>,
}
pub enum StorageError {
    TableNotFound { table: String },
    RowAlreadyExists { table: String, id: String },
    RowNotFound { table: String, id: String },
    BackendFailure { message: String },
    StoreInUse,
}
```

Values, rows and errors support cloning, debugging and equality assertions.
`BTreeMap` is `std::collections::BTreeMap`. Table and row IDs are exact strings;
no schema, reserved names, ID generation or empty-string validation is introduced.

Every `Storage` trait method returns `impl Future<Output = Result<T,
StorageError>> + Send`, with `Future` from `std::future`. The methods borrow
`&mut self` for mutations and `&self` for reads; futures may borrow their inputs
until awaited. No `'static` future, boxed future, trait object or runtime is
required or promised. Method arguments and result `T` are:

| Method | Arguments after receiver | Result T | Behavior |
| --- | --- | --- | --- |
| `create_table` | `name: &str` | `()` | Create empty table; existing table succeeds unchanged. |
| `insert` | `table: &str, row: Row` | `()` | Insert only if ID absent; duplicate returns RowAlreadyExists, preserving old row. |
| `get` | `table: &str, id: &str` | `Option<Row>` | Return owned clone, or None for missing ID. |
| `replace` | `table: &str, row: Row` | `()` | Replace the complete existing row; missing ID returns RowNotFound without insertion. |
| `remove` | `table: &str, id: &str` | `bool` | Return true if removed, false if absent. |

Every method except creation rejects an unknown table with TableNotFound,
including reads/removal of missing IDs. Errors carry the supplied table/ID.
Replace removes omitted fields rather than merging. Rejected operations preserve
existing data. The memory implementation owns maps per instance with no I/O.
Ordering of table storage is incidental; no query ordering contract is added.
Filters and change notifications remain unspecified in this API.
SPEC-0023 compiles and exercises this bound for the SQLite browser target,
whose operations return already ready futures. This does not validate arbitrary
JS-backed asynchronous adapters or browser connection transfer.

## Optional ordered atomic-write capability

SPEC-0020 adds a separate trait; the original `Storage` contract is unchanged.
The owned mutation type supports `Clone`, `Debug`, `PartialEq` and `Eq`:

```rust
pub enum Mutation {
    Insert { table: String, row: Row },
    Replace { table: String, row: Row },
    Remove { table: String, id: String },
}
pub trait TransactionalStorage: Storage {
    fn apply_batch(&mut self, mutations: Vec<Mutation>)
        -> impl Future<Output = Result<(), StorageError>> + Send;
}
```

Mutations run in order, including repeated operations on the same ID. Ordinary
typed errors stop at the first failure and preserve all original tables; return
the original CRUD error without wrapping it. Missing-row removal is a no-op;
unknown tables remain errors. Empty batches succeed. The capability is static
and optional: implementing Storage alone promises no batch support. This is an
atomic-write subset, not a read transaction, durability, queue, observer,
concurrent-client isolation or cancellation contract. The memory adapter's
temporary whole-map copy cost is recorded in SPEC-0020.

## Native SQLite foundation

SPEC-0022 adds `hoyadb_storage_sqlite::SqliteStorage` with synchronous
`open(path: impl AsRef<std::path::Path>) -> Result<Self, StorageError>`.
Empty paths, `:memory:` and paths beginning with SQLite’s `file:` URI prefix
are rejected before opening; the adapter never silently selects memory.
The path identifies a literal local file; open creates the private format and
version-1 marker only in a new or empty database, preserves compatible existing
contents, and returns BackendFailure on
ordinary open/initialization failure without a memory fallback or reset.
Existing nonempty files require the supported ownership/version marker and
compatible private schema; unrelated or incompatible files are rejected before
schema/data mutation. There is no automatic application schema migration.
Parent directories are not created. This native configuration remains unpublished;
the browser target extension is documented below. Core/memory remain independently
compilable for WASM.

SQLite implements the same `Storage` and `TransactionalStorage` behaviors.
Operational SQLite and strict field-decoding failures return BackendFailure;
message text is diagnostic and unstable. Existing domain errors retain their
exact variants and supplied coordinates. A read returns a complete owned row
or an error. Single multi-statement mutations and batches use SQL transactions
and roll back on ordinary errors, including failed commit. No separate memory
cache is published and no notification API is added.

Each method completes its SQLite work synchronously before returning an already
resolved `Send` future; callers can await it but this API makes no nonblocking
or lazy-execution claim. Discarding an unpolled returned future does not cancel
a completed mutation. The adapter owns its connection and can move between threads with ownership.
On native targets it holds an exclusive SQLite file lease for that connection's
lifetime (SPEC-0037 R1), acquired after keying and zero busy timeout, before
initialization or owner publication. Other connections cannot read or write the
owned file; a second hoyadb open returns StoreInUse. A raw SQLite connection may
be allocated but cannot perform those operations. Drop or process termination
releases the lease. No lock file is created. A zeroed -journal sidecar may remain
between transactions, under the same protection/backup policy. WASM claims no
such exclusivity. Protected opens retain StoreInUse while redacting other errors;
the facade maps it to Busy. Adding this variant affects exhaustive Rust matches. There is no
automatic busy retry, executor or async runtime. This is a partial storage
foundation without schema/query, migration, encryption or crash-tested queue
guarantees. REQ-0038 and TC-0022 describe safety/acceptance boundaries.


## Browser SQLite target extension — SPEC-0023

The operator approved the browser persistence experiment on 2026-10-21. On
wasm32-unknown-unknown, SqliteStorage uses the reviewed SQLite WASM backend and
an asynchronously initialized OPFS SAH-pool VFS in one dedicated worker. Its
path argument identifies a literal VFS database name, not a native filesystem
path. Empty, `:memory:` and `file:` URI names remain rejected. Initialize the
VFS before opening; required capability or initialization errors are explicit,
without a memory fallback. The same Rust CRUD, strict format/decoder and SQL
transaction implementation is reused. Ready futures still perform synchronous
SQLite work; no unsafe Send/Sync implementations or browser thread-transfer
support are added. Exactly one active connection/worker owner is supported by
the experiment. Native configuration and existing conformance cases remain
unchanged. TC-0023 documents the applicable browser subset and exclusions;
this is not a stable JavaScript consumer API or full-platform conformance claim.

Executed browser evidence: [SPEC-0023 results](../../implementation/browser-persistence.md).
Browser opens name `opfs-sahpool` explicitly so initialization cannot fall back
to the backend’s default memory VFS.
