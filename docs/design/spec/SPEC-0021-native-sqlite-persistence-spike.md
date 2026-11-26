---
id: SPEC-0021
title: Validate native SQLite persistence across process restart
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-21
updated: 2026-10-21
parents: [ADR-0008, ARC-0002]
satisfies: [REQ-0035, REQ-0037, NFR-0003]
depends_on: [SPEC-0002, SPEC-0018]
tags: [spike, native, sqlite, persistence]
---

# SPEC-0021: Native persistent-mode experiment

Origin: operator approved `rusqlite = "=0.40.2"`, `default-features = false`,
`features = ["bundled"]`, with reviewed locked transitives, and instructed
completion of the Stage 0 persistence proof. Add it only to the isolated native
spike, preserving the dependency-free shared spike core and production core.
Do not promote spike code into the production workspace or claim full SPEC-0002.

## Behavior and implementation boundary

Add `hoya_open_persistent(const char* path) -> u64`: zero on invalid path,
SQLite failure or incompatible stored data. The borrowed path follows existing
readable NUL-terminated/UTF-8 rules and is copied before return. Reject empty
paths, `:memory:` and the SQLite `file:` URI prefix: this mode must not silently
select an in-memory or URI-configured database. Bundled SQLite enables URI
parsing globally even without an explicit OPEN_URI flag; validate before open. Creation makes
an empty experimental database; reopen restores committed typed rows through
the same Rust Store used by the existing Swift insert/query/handler paths.
Existing `hoya_open()` remains memory-only. Close does not perform a deferred
save: a successful persistent insert/committing scope finish already persisted.

The native registry entry owns Store and optional SQLite connection. Prepare a
private cloned candidate Store for each persistent insert/committing finish;
apply the existing core behavior to it, persist its committed rows in one
SQLite transaction, then publish the candidate only after commit succeeds.
An ordinary storage failure returns status 4, preserving original memory, rows,
notifications and active scope for retry/abort. Existing statuses 0–3 retain
their meaning. Abort publishes no staged rows and needs no database write.
Do not emit subscription events before a successful persistent commit.

The experiment may replace all stored rows per commit; record its O(total rows)
copy/write baseline. SQLite contains one private fixed-format table with ID,
status, signed 64-bit total and boolean. Bind values as SQL parameters, preserve
Unicode and integer extrema, validate decoded column types/boolean range and
duplicate IDs before publishing a restored handle. No dynamic SQL identifiers,
extensions, user schema or serialized foreign pointers enter the file.
Core gains only deliberate Clone/committed-row snapshot/restore support needed
by this experiment; shared matching, staging and notification semantics stay in
the core. Restoration creates no subscriptions, notifications or active scopes.

## Verification and limits

Review and commit spec/cases before tests/code. Observe failing Rust persistence
tests and a real Swift writer/reader proof, then implement minimally. TC-0021
requires two separately launched OS processes, not reopen within one process.
Retain all existing native/core/production tests and relevant format/lint checks.
Record commands, SQLite runtime version, file location, process exits and rows.

Only clean process-restart persistence and ordinary failure atomicity are proved.
Power loss, crash injection, fsync behavior on each filesystem, multi-process
writers, migration, encryption, production schema/query adapter conformance and
durable offline queue are not established. Preserve SQLite journal/sync defaults;
do not weaken them to pass tests. A panic status remains non-rollback as in SPEC-0018.
The SQLite table is experimental; no released upgrade compatibility is promised.

## Boundary/security review and rollback

Paths and database bytes add filesystem input to the native boundary. Reuse
REQ-0035/THR-0009 pointer/ownership preconditions; treat on-disk data as untrusted,
return status/zero on ordinary decoding/SQLite errors, and never interpolate
fixture values into SQL. Harness paths are fresh dedicated temporary directories
containing synthetic data; never open or overwrite a user's existing database.
There is no encryption: THR-0002 remains unresolved; make no REQ-0030 claim.
STRIDE: file tampering/disclosure and resource exhaustion remain possible;
there is no identity/audit/elevation claim or hostile-file sandbox guarantee.
Do not log unrelated paths/data or retain foreign callback/context pointers.
THR-0011/REQ-0037 track file validation and failed-commit publication.
Focused native/SQL review precedes delivery. Revert only this isolated mode,
dependency/lock changes and test files; delete only harness-owned fixtures.

Primary sources: [rusqlite 0.40.2](https://docs.rs/rusqlite/0.40.2/rusqlite/),
[SQLite transaction semantics](https://www.sqlite.org/lang_transaction.html).
