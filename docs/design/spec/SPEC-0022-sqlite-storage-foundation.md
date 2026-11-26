---
id: SPEC-0022
title: Add native SQLite storage with shared CRUD and batch conformance
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-21
updated: 2026-10-21
parents: [ARC-0001, ADR-0003]
satisfies: [REQ-0034, REQ-0038, NFR-0003]
depends_on: [SPEC-0001, SPEC-0002, SPEC-0019, SPEC-0020, SPEC-0021]
tags: [storage, sqlite, conformance, foundation]
---

# SPEC-0022: Native SQLite storage foundation

Origin: operator instruction to finish Stage 0 and Phase 1, followed by explicit
approval of `rusqlite = "=0.40.2"`, `default-features = false`,
`features = ["bundled"]`, and reviewed locked transitives. Add an unpublished
`crates/hoyadb-storage-sqlite/` adapter to the production workspace, depending
on core and this approved dependency. Keep core dependency-free. Do not promote
or depend on spike implementation code.

## Scope and contract

Implement the exact typed CRUD and optional atomic-write subset in API-0002,
with the same outcomes as memory storage. Add `StorageError::BackendFailure
{ message: String }` for operational SQLite/open/decode failures; preserve the
three existing table/row errors and their original coordinates. Backend error
text is diagnostic, not a stable machine-readable format. No adapter logging
or telemetry is added. A failed read never returns a partial row.

`SqliteStorage::open(path: impl AsRef<std::path::Path>) -> Result<Self,
StorageError>` opens or creates a local database and initializes its private
format without resetting an existing database. The caller supplies a literal
local path, not SQL or a SQLite URI. Reject the `file:` prefix explicitly,
as well as empty and `:memory:` paths: the bundled SQLite build enables URI
parsing globally even without OPEN_URI, which could otherwise silently select
an in-memory database. Failure to open or initialize returns
BackendFailure without a usable adapter. Do not create parent directories or
silently fall back to memory. Tests use only newly owned temporary directories;
do not open an unrelated existing database. Dropping the adapter closes its
connection; each successful mutation already committed, with no deferred save.

File ownership is explicit: a new or empty SQLite database may receive this
adapter's private schema and a checked format/version marker in one transaction.
An existing nonempty database is usable only when it has the supported marker
and compatible expected physical tables, columns and keys. Validate ownership
and compatibility before any schema/data mutation. Reject unrelated databases,
missing/unknown versions, incompatible schema and malformed non-SQLite bytes
with BackendFailure; never stamp them as owned, add tables, reset or migrate
them automatically. A permissive sequence of `CREATE TABLE IF NOT EXISTS`
alone is insufficient validation. The internal initial format is version 1;
this does not introduce application schema-version migrations. Tests compare
unrelated file/schema contents before and after rejected open.

SQLite work runs synchronously at the trait method call, returning an already
resolved `Send` future. Awaitability does not imply nonblocking execution or
lazy scheduling. No executor, worker thread, lock or runtime is introduced.
The adapter owns one connection, can move to another thread with ownership,
and makes no shared concurrent-access guarantee. Document this behavior in
API-0002 and test `Send` futures without holding a connection reference across
an await. Preserve SQLite's journal/synchronization defaults; configure no
automatic busy retry so an operational lock failure is returned to the caller.

## Stored representation and atomicity

Use fixed namespaced physical tables for logical tables, row identities and
typed fields. Logical table/row/field names are bound SQL values, never dynamic
SQL identifiers. The physical format is internal and experimental, not a
released migration format or implementation of application schema declarations.
Empty strings, Unicode, embedded NULs and SQL-looking strings retain the exact
API-0002 string semantics. A row with no fields must still exist independently
of the field table. Text, signed i64 and boolean fields use native SQLite
storage with an explicit checked value discriminator. Decode text/integer
storage types, discriminator, boolean range and required/forbidden NULL payloads
strictly; reject incompatible
stored values as BackendFailure rather than coercing or dropping them.
No JSON/binary serialization or new dependency is required.

Create an existing table idempotently without changing its rows. For insert,
replace and remove, atomically update the row identity and all its fields using
SQL transactions. Replace removes omitted fields; duplicate inserts preserve
the complete prior row. Preserve exact unknown-table/missing-row precedence
from API-0002. Constraint or I/O errors must not be misreported as ordinary
table/row errors merely because SQLite rejected a statement.

`apply_batch` executes the ordered write log in one real SQL transaction,
reusing the same CRUD validation and encoding logic. Earlier mutations are
visible to later ones inside the transaction. Return the first original typed
error, or BackendFailure for operational failure, with all writes rolled back.
Commit only after the entire batch succeeds; do not maintain a separately
published memory cache or copy all stored rows per write. Empty batches and
known-table absent-row removals succeed. This proves bounded storage atomicity,
not a durable offline queue, concurrent-client isolation or power-loss recovery.

## Shared conformance and acceptance

Review and commit this spec, API-0002, REQ-0038/THR-0012 and TC-0022 before
test code or implementation. Refactor the existing M01–M09 and T01–T07 tests
into a test-only generic module under `tests/conformance/`, included with
`#[path]` by memory and SQLite integration tests. Adapter-local factories create
fresh stores; the same assertions run against both adapters. Do not create a
new crate/dependency, export production test helpers or weaken existing cases.
First prove the extraction preserves all 16 existing memory behaviors, then
observe SQLite cases fail for missing required behavior before implementing it.

TC-0022 adds native persistence, failure/encoding and thread cases beyond the
shared subset. Launch separate writer and reader OS processes for restart
evidence. A second connection may inject deterministic locks, aborting SQL
triggers or malformed field values only into dedicated test fixtures; no
test-only production failure toggle is added. Validate complete rows and error
variants, rather than checking only counts or error presence.

Workspace host tests, formatting and Clippy must pass, including both adapters.
CI must fetch approved locked dependencies before running any offline job
commands: the root workspace, native spike and browser spike now each require
`cargo fetch --locked` with the appropriate manifest before their offline
test/build/lint steps. A warm developer cache is not proof of fresh-runner
bootstrap. Add browser-spike formatting, Clippy and WASM compilation commands
to CI; these compile checks are not evidence of browser runtime execution.
WASM compilation explicitly selects `hoyadb-core` and
`hoyadb-storage-memory`: the new SQLite adapter is native-only. Update documented
commands and the existing CI command accordingly, preserving existing runner
tags and other unrelated user changes. Retain the separate Stage 0 suites and
run traceability/RTM checks. Record actual results and remaining limitations;
a local pass is not evidence that a remote GitLab pipeline ran. Q09 requires
repeatable fresh-cache preparation and checks the native WASM exclusion.

## Security, limits and rollback

THR-0012/REQ-0038 cover SQL-value binding, malformed stored types and atomic
error handling. STRIDE: database bytes may be tampered with; plaintext files
can disclose data (THR-0002); locks, oversized values and hostile files can
deny service. The bounded synthetic fixtures establish neither resource limits
nor a hostile-file sandbox. No identity, authorization or audit guarantee is
introduced. Data remains local; no network or raw SQL is exposed to consumers.

This is a partial native storage foundation. Declared application schemas,
full primitive types, query predicates, subscriptions/change streams, migrations,
encryption/keys, backup exclusion, browser persistence and the offline queue
remain outside this slice. In particular, this adapter does not satisfy
REQ-0030 or all SPEC-0001/SPEC-0002 responsibilities. Existing accepted design
obligations remain in force and must not be marked complete by this subset.

Rollback reverts only this adapter, operational-error extension, shared-test
extraction and associated documented commands through review. Preserve existing
memory/spike work and allocated artifact IDs. There is no published package or
released data-format upgrade to undo. Test cleanup removes only harness-owned
temporary paths; do not reset databases as rollback behavior.

Primary API sources for implementation verification:
[rusqlite 0.40.2](https://docs.rs/rusqlite/0.40.2/rusqlite/),
[SQLite transactions](https://www.sqlite.org/lang_transaction.html), and
[SQLite types](https://www.sqlite.org/datatype3.html).
