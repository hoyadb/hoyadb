---
id: API-0010
title: Supplied-key SQLite opens platform backup hints and bounded diagnostics
type: api
status: accepted
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [ARC-0001, ARC-0002]
satisfies: [REQ-0015, REQ-0028, REQ-0030, REQ-0031, REQ-0032, REQ-0045]
depends_on: [SPEC-0032, RFD-0005, API-0005, API-0006, API-0008, API-0009, TC-0035]
tags: [phase-six, security, contract, experimental]
---

# API-0010: Supplied-key SQLite opens platform backup hints and bounded diagnostics

## 1. Adapter factory and key ownership — S01–S07

Add adapter-owned EncryptionKey::from_bytes([u8;32]) and SqliteOpenOptions with
an optional supplied key; Default has no key, and custom Debug never includes
key bytes. Key and key-SQL owned buffers use zeroize; no Clone/serialization/key
persistence or public key accessor. This does not promise erasing all foreign,
allocator/binding/parser/process copies. Caller entropy/storage/lifecycle remain
external. No passphrase/KDF or key generation is performed by hoyadb.

SqliteStorage::open_with_options(path,options) requires a key and uses engine
SQLCipher v4 authenticated AES256-CBC/HMAC-SHA512, 4096-byte page profile,
plaintext header disabled and memory-only temporary storage. The existing
SqliteStorage::open(path) now invokes the secure default and rejects missing key
before filesystem access. Add explicit open_unprotected(path) for historical
unprotected synthetic fixtures; it is never a protected/production-floor claim.
The generic Storage/Typed/Schema/Observation/Sync traits do not acquire key APIs.

The constructor validates path and key before open, then sets a strictly encoded
fixed-size raw key before any schema/catalog/pragma-list read or DDL. Never use
an upstream pragma helper that internally queries pragma_pragma_list first.
Only internal fixed SQL plus 64 validated hex digits form the key statement;
no user SQL or configurable cipher downgrade is accepted. Probe cipher_version
and profile with direct fixed SQL and reject an unavailable/mismatched build.
Only then validate exact existing private schema or initialize an empty store.
Wrong-key/plaintext/corrupt/foreign data must never be stamped/reset/converted.
Map open/cipher errors to static key-free failures; never include engine SQL,
paths, keys, caller values or raw engine errors in protected-open diagnostics.

Add storage_protection() returning owned encrypted Boolean and static cipher/
version capability data, no key/path/identity. Unprotected fixtures report false.
SQLCipher handles salts/IVs/HMAC subkeys internally as part of its engine scheme;
those are not application key generation or a new hoyadb KDF. On-disk main/journal/
WAL contains no test canaries/key text. Temp operands/sorts stay memory-only.
Data schema, queue, counters and metadata remain the same logical format.
No public SQL, ATTACH, extension loading, plaintext export, conversion or rekey.

## 2. Backup and foreign key/configuration boundaries — S08–S10

The Swift reference host sets/verifies Foundation isExcludedFromBackup on its
owned store directory before protected open, default true. False is explicit
opt-in. Failure prevents DB data creation, without changing unrelated paths.
Deployment guidance recommends a dedicated app-owned Application Support
subdirectory. The synthetic harness retains only its pre-established canonical
task-owned temp-root confinement; no new arbitrary/default path resolver is added.
Validate the exact key length, single-component store name and controlled root
before creating a directory or changing any backup attribute. Reject symlink or
non-directory path components rather than following them to unrelated locations.
File replacement is allowed only while the owner is closed; revalidate directory
ownership and reapply/read back backup policy before reopen or further writes.
No arbitrary import, plaintext conversion or unrelated-path policy setter exists.
After its own directory/file replacement it reapplies/verifies the policy before
further writes. Directory coverage includes SQLite sidecars; this is an OS hint,
not guaranteed suppression of arbitrary backups/copies. Keychain/key lifecycle
belongs to the application; tests use fixed synthetic externally supplied keys.

Browser OPFS reports backupExclusion="unsupported". It must not substitute
StorageManager.persist() or origin isolation for a missing backup control.
Protected browser encryption is still required and verified independently.
No filesystem export operation or implicit membership purge is introduced.

Extend the existing test-only common fixture with open_secure: the exact API-0008
open args plus key_hex:String (exactly 64 lowercase ASCII hex digits). Reject
malformed key before opening a file. This uses open_with_options and stores no
key in request registries/queue/progress/diagnostics. Close/reset cannot replay
an old buffered key request. Existing open is retained only as the documented
unprotected legacy test control and explicitly calls open_unprotected.
Add protection (empty args) returning {encrypted:Boolean,cipher:String,
backup_exclusion:"host-managed"|"unsupported"} with no path/key. Actual Swift
verifies its host backup hint; a core declaration is not proof of an OS flag.

JS CommandClient.open uses protected open and requires encryptionKey as an
externally supplied 32-byte Uint8Array. Add openUnprotected for existing explicit
legacy fixtures; do not silently choose it on missing key/error. Native reference
owner has explicit protected supplied-key and legacy-unprotected construction.
The same C/WASM ABI, frame/chunk limits and checked handles remain unchanged.
Key conversion/copy lifetime limits in foreign runtimes are explicit; no key is
included in errors/logs, diagnostics or persisted retry state. Never parse domain
opaque numeric payloads through foreign Number during the upgrade.

Each JS client reserves one in-flight open attempt before sending its RPC.
Another protected or explicit legacy open on that client rejects Busy until
that attempt settles and any required disposal is acknowledged. Closing during
an in-flight open marks that attempt cancelled immediately: its eventual reply
must never publish an owner, invoke a handler or register callbacks on the
closed client. The cancelled open rejects Closed. close() waits for the original
open to return; if it acquired a handle, close sends exactly one disposal for
that handle and waits for its acknowledgement. If the original open fails
without acquiring a handle, close completes without sending a fabricated close.
An already submitted engine open may create its owned database; client-side
cancellation does not claim filesystem rollback.

A disposal error makes close reject and leaves cleanup pending; replacement
opens remain Busy until a later close acknowledges disposal. Repeated closes
share any in-flight cleanup rather than disposing the same handle concurrently.
A fresh open after successful close is a new attempt with freshly supplied
configuration; no old key-bearing open request is replayed. If the transport
never settles, close may remain pending: this contract introduces no implicit
watchdog, transport cancellation or guaranteed cleanup-completion deadline.
These lifetime rules also apply to the explicit legacy factory.

## 3. Safe diagnostics and formatting — S11–S14

Add CommandDatabase::diagnostics() -> DiagnosticSnapshot with only schema_version,
pending, sending, awaiting_outcome, live, blocked, ready, pages, changes and
tombstones. No storage reads/scans, payload cloning, serialization or identifier
cloning are needed; iteration over in-memory records to count state is permitted.
All fields are bounded
numeric/Boolean state, no identifiers or external export destination. The exact
host diagnostics operation has empty args and returns those fields with decimal
Strings for counters/version, Booleans for flags. No automatic output or network.

Custom Debug on CapturedContext, EntityContext, CommandResult, RemoteOutcome,
CommandOutcome, PushBatch, SyncRecord, SyncLedger, PullBatch, PullPage, QueueEntry,
QueueStatus, CommandEvent, CommandError and StorageError prints only type/variant/counts, not payloads,
identity, reasons, record/table names/IDs, key/namespace/scope/checkpoint values.
Explicit application access to Row/Value/Json remains data access; no claim
prevents an app from deliberately logging data returned by those APIs. Data and
retry serialization remains byte-complete; Debug is never its serializer.
Redact raw/worker fallback errors to static error kind at foreign boundaries;
keep typed errors internally and useful caller-owned data only via explicit APIs.

Host handler context is a deep immutable owned snapshot. JS context and every
nested identity node/array reject assignment; scope.context itself is not
replaceable. Snapshot remains unchanged through await/reentry. Rust and Swift
keep their existing immutable captured values. No authentication/membership
inference or telemetry is added. Actual transport is caller-owned and explicit.

## 4. Verification and rollback

TC-0035 maps every boundary to real engine, process, browser/Swift and fault
checks. Encryption tests inspect synthetic raw main/sidecar bytes and unkeyed
reads as well as successful keyed queries. Never infer protection from a PRAGMA
that could be silently ignored or from absence of one plaintext string alone.
Unknown/wrong inputs preserve files/recovery. Revert only task source; retain
compatible readers and caller keys. No production data operation or release.

## Verification status — 2026-10-24

Operator-approved ADR-0034 policies and exact graph are implemented and reviewed.
[Phase 6 execution evidence](../../implementation/phase-six-security.md) records
protected native/Swift/OPFS, fault/restart, privacy and pinned-container checks,
and the final crypto plus lifecycle Claude reviews. Scope and residual limits
remain explicit; this status does not certify new mobile targets or publication.
