---
id: API-0007
title: Atomic command recovery storage and exact persisted records
type: api
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-23
updated: 2026-10-27
parents: [ARC-0001, ARC-0002]
satisfies: [REQ-0013, REQ-0018, REQ-0019, REQ-0033, REQ-0043]
depends_on: [SPEC-0029, ADR-0032, API-0003, API-0004, API-0006, DM-0004, SPEC-0040]
tags: [command-sync, commands, storage, recovery]
---

# API-0007: Atomic recovery storage

This additive capability targets existing transactional memory/native/browser
engines. It introduces no runtime dependency or private SQLite physical DDL.
The common core decoder validates known recovery records before either adapter
recognizes them in an inventory. This is not permission to accept arbitrary
reserved tables or best-effort atomicity on unimplemented engines.

## Capability and atomic boundary

Expose `storage::SyncStorage: ObservationStorage` with synchronous methods
`load_sync(&self) -> Result<Option<Vec<Row>>, StorageError>` and
`apply_sync_plan(&mut self, plan: SyncPlan) -> Result<Vec<CommittedChange>, StorageError>`.
SPEC-0037 R3 adds a provided `ownership(&self) -> StoreOwnership` method,
with default `StoreOwnership::Unproven` and enum variants `Unproven` and
`Exclusive`. Native SQLite may return Exclusive only while it holds its actual
connection lease; owned memory values may return Exclusive. Unknown/custom
adapters keep the conservative default. Every WASM owner retains the global
coordinator irrespective of this value. This is an adapter contract with tests,
not a caller request field or a claim of cross-tab/multi-writer support.

SPEC-0037 R6 additionally provides `reads_ready(&self) -> bool`, defaulting to
`false`. This is independent of ownership: `Exclusive` does not promise read
readiness. Returning `true` promises that every future returned by `get`, `scan`,
`query`, `query_validated`, `project` and `catalog` completes on its first poll.
Native SQLite and `SnapshotStorage` return true; the facade `Engine` forwards
its adapter's value. WASM may return true only for genuinely immediate reads;
its Unproven ownership still prevents a live-store overlay. No caller request
or persisted option can enable this capability.

Only a strictly validated Exclusive owner with `reads_ready() == true` stages
against its live store. All other owners await a full snapshot at `begin`, then
stage against that private, immediate-read copy. An Exclusive adapter whose
reads may suspend remains supported with per-command copying and validation;
no Phase-2 staging performance claim applies to that fallback. Dropping pending
snapshot acquisition releases admission and publishes no active scope or
model, row or ledger change. A false readiness promise that returns Pending
fails with `StorageError::BackendFailure`, without re-polling or busy-waiting.
The failed operation restores its entire delta and first-image state, including
earlier primitives of a batch, and the scope remains usable. No dependency,
async runtime or `Sync` bound on adapters is added.

The synchronous commit deliberately has no Pending/cancellation interval between
durable commit and caller state update. The later async command owner still
supports awaited host reads/writes; no transaction spans a handler await.

`SyncPlan` owns `expected_model: ModelState`, `next_model: ModelState`,
`expected_sync: Option<Vec<Row>>`, `next_sync: Vec<Row>`,
`guards: Vec<RowGuard>` and `mutations: Vec<Mutation>`. `RowGuard` owns
`table: String`, `id: String`, `expected: Option<Row>` containing raw versioned
rows. Derive the application schema from the checked model definition, not a
caller-supplied projection. Next model must have identical version/definition
and nondecreasing next_id. Application mutations cannot address reserved tables
or fields other than the existing managed row-version field.
Every inserted/replaced row's ROW_VERSION must equal next_model.version exactly.
Live command schema versions must equal that model version; fully resolved
history may retain its original version. For an existing ledger, namespace stays
identical and header sequence/last_ms never decrease across a successful plan.

Inside one SQLite transaction or memory candidate clone, validate the actual
model and complete prior sync rows equal their expected values (row order is
irrelevant); None means the reserved table is absent. Validate every distinct
mutated application row has exactly one initial guard and no duplicate guards.
Validate all initial guards against actual rows before applying any mutation.
This comparison is necessary because replacing/removing rows does not advance
the model next_id. A same-row ordered sequence uses one initial guard, not one
per operation. SPEC-0039 additionally permits read-only expectation guards: the
mutated row set must be a subset of the guarded row set. A matching no-op or
expected-absence guard is checked inside the same atomic commit even when no
application mutation remains. Every guard addresses a declared application table, never a reserved
`__hoya_*` table, including
guards whose expected row is absent; unknown tables and duplicate guards are
invalid plans. A mismatching read-only guard fails with the existing guard
conflict and changes nothing. Reset retains its stricter complete-inventory
rule: its guards and removals cover exactly the actual application inventory.

Apply ordered typed application mutations, final model counter and the entire
validated next recovery row set atomically. Create the logical recovery table
inside that transaction only when absent. Never create it in a separate earlier
step. Return application before/after changes in operation order, not private
model/recovery changes. An ordinary failure at any point preserves all original
state, including table absence and counters. SQLite uses the existing bound
insert/replace/remove helpers; a shared typed-transaction helper may be extracted
under existing regression tests to avoid nested transactions or duplicated rules.
Memory runs existing immediate-ready operations against its private candidate
and swaps only on complete success.

## Exact reserved format

The one logical table is `__hoya_sync`. It has exactly one row with id `state`
and zero or more `command:<key>` rows. Each row has exactly `format: Integer(1)`
and `data: String(canonical JSON)`; no application ROW_VERSION field. Decode
each JSON value using API-0006 bounds, so 1000 command records do not share a
single 1 MiB aggregate codec limit. Every malformed/future/unknown field or
row shape rejects. Unknown reserved table names still reject through the
unchanged checked_model_catalog rule.

Header payload fields are exactly `namespace` (32 lowercase hex characters),
`sequence` (positive i64 next execution sequence), `last_ms` (nonnegative i64),
and `options`. Options fields are exactly `capacity` (positive integer, default
1000), `history` (nonnegative integer, default 100), `initial_delay_ms` (positive
i64, default 1000), `multiplier` (integer at least 2, default 2), `max_delay_ms`
(at least initial delay, default 60000). Jitter is a supplied integer sample
0..2500 basis points of capped base delay, not persisted as a PRNG seed.

Each command payload has exactly the following typed fields. The common core
types live in `command::records`, with a doc-hidden public facade for adapters.
User-facing operations are specified separately; these records are not an
unvalidated application escape hatch.

| Field | Type and invariant |
| --- | --- |
| key | String equal to the row suffix; generated key, never empty |
| command | Nonempty prefix followed by -v and a positive decimal version, at most 128 UTF-8 bytes |
| args | JSON Object, updated only at declared dependency paths before first send |
| context | JSON Object containing exactly identity (any bounded JSON) and capturedAt (fixed UTC YYYY-MM-DDTHH:mm:ss.SSSZ String, years 1970–9999) |
| schema_version | Positive u32 |
| order | Positive i64, unique, less than header sequence |
| reference_paths | Array of paths; each path is an array of String keys or nonnegative Integer indices |
| dependencies | Sorted unique local-ID strings still unresolved, all owned by retained producers |
| created | Object mapping each created local-ID String to its application table String |
| mappings | Object mapping a producer's created local IDs to final ID strings |
| undo | Array of ordered CommittedChange values with table/id/before/after; rows encode id and typed fields |
| state | One of queued, sent, acknowledged, applied, corrected, rejected, cancelled, discarded |
| frozen | Null before first send, otherwise the exact canonical envelope String |
| attempts | Nonnegative u32 |
| due_ms | Nonnegative i64 |
| outcome | Null or applied/corrected with result JSON, or rejected with reason String |
| rolled_back | Boolean, true only after completed local inverse apply |
| reconciled | Boolean, true after completed final local outcome processing or explicit discard, including a never-sent discard |

Generated keys have exact form `tttttttttttttttt-nnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn-ssssssssssssssss`:
16 lowercase hex digits for the nonnegative clamped millisecond clock, the
header's 32-hex per-store namespace, and 16 hex digits for positive order.
The final segment equals order; key time is at most header last_ms. Timestamp
clamping for key order does not change capturedAt, which records the actual
sample used by ctx.now(). The sequence is checked against i64 exhaustion.

Typed persisted values are exactly `{ "kind": "null" }` or
`{ "kind": "string|integer|float|boolean|timestamp|reference", "value": ... }`
with one actual tag string, not that literal list. Payload types are respectively
Json::String, Integer, Float, Bool, Integer and String; Float must remain Float
even when integral. Rows are exactly `{ "id": String, "fields": Object }`
of tagged values. CommittedChange is exactly `{ "table": String, "id": String,
"before": Null|Row, "after": Null|Row }`; present row IDs equal change id.
Outcome is exactly `{ "state": "applied|corrected", "result": Json }` with
one actual state, or `{ "state": "rejected", "reason": String }`.
Unknown/extra fields or tags reject. No raw Debug parsing or lossy conversion.
Undo images use public application rows without managed/reserved fields; the
record's schema_version supplies their version when converting to raw RowGuards
or storage mutations. This differs deliberately from raw versioned RowGuard
images. Reject reserved fields in every undo image. Validate live undo tables/
fields/types against the current model with its implicit row-version added only
for validation; fully resolved historical images cannot assume a current schema.

Frozen strings decode to a canonical JSON Object with exactly args, command,
context, idempotencyKey and schemaVersion; all five equal their record fields.
Canonical re-encoding must equal the stored bytes. A syntactically valid unrelated
JSON string is not a valid frozen envelope. A record with frozen bytes has no
unresolved dependencies and cannot change its wire fields on subsequent plans.
Doc-hidden `SyncRecord::check_send_size(&self) -> Result<(), StorageError>`
preflights encoded record size with the current complete metadata and an added
frozen envelope. This is a size-only calculation, not a persisted state change or
dependency admission check; it also works for queued blocked records. A future
authoritative ID/result may still exceed encoding limits and must fail locally
without partial data application; no arbitrary-size backend result is promised.

| State | Required persisted combination |
| --- | --- |
| queued | frozen Null; attempts 0; outcome Null; rolled_back false; reconciled false |
| sent | frozen String; attempts positive; outcome Null; both flags false; no dependencies |
| acknowledged | frozen String; attempts positive; outcome Null; both flags false; no dependencies |
| applied / corrected | frozen String; attempts positive; matching outcome; rolled_back false; reconciled false while local plan is pending, true after it commits |
| rejected | frozen String; attempts positive; matching rejected outcome; reconciled false until the rejection is processed, then true; rolled_back true implies reconciled true |
| cancelled | frozen Null with attempts 0, or frozen String with attempts positive; outcome Null or an already received rejection; unreverted cancellation remains live; a sent tombstone stays live even after successful rollback |
| discarded | reconciled true; rolled_back false; no entity descriptor or metadata_undo; empty mappings (already empty for every eligible root), reference_paths and dependencies; frozen Null and attempts 0 with outcome Null, or unchanged frozen String and positive attempts with retained Rejected outcome; created and undo preserved; non-live |

SPEC-0040 adds `discarded` without changing the ledger format number. Its record
has rolled_back false, reconciled true, no entity descriptor, empty metadata_undo
and empty mappings. An unsent discard has frozen Null, attempts zero and outcome
Null; a sent discard retains frozen bytes, positive attempts and its original
Rejected outcome. Discard atomically clears reference_paths and dependencies;
args, created entries, undo, frozen bytes, attempts, due_ms, outcome and mappings
are unchanged. No other field changes beyond state, reconciled and rolled_back
as specified above. Empty reference_paths and dependencies are required for every
persisted Discarded record. Existing reference and DAG validation remains in
force; there is no exception for dangling local references in other states.
The never-sent Cancelled cleanup that clears created and undo does not apply to
Discarded history. A Discarded producer may be forgotten or trimmed while its
Discarded dependents remain; no additional producer retention is introduced.
The preserved fields describe the discard transition. Existing later mappings
still rewrite historical undo identities and Reference values on every retained
record, including Discarded history; they do not renormalize Discarded args or
change its frozen envelope. Cleared dependencies keep a Discarded record outside
later producer inverse closures. This does not add reference-target existence
enforcement: a later producer rollback can leave a kept dependent row with a
dangling Reference.
It is non-live, never dispatched and excluded from pending reconciliation.
Rollback, cancel, reconcile and request_redelivery return Protocol. Identical
retained Rejected outcomes and receipt-only None are no-ops only for a sent
discard; any receipt/outcome for a never-sent discard is Protocol.

This is an approved stored semantic compatibility break in the unreleased
library: an older build rejects the unknown state while it remains in history.
Switching policy to Stop does not remove it. Existing non-live forget and trim
rules apply with no promised removal time. No automatic conversion or user-store
operation is authorized. All SDK status decoders change together; implementation
and verification remain pending under TC-0044.

SPEC-0038 permits Acknowledged to return to Sent only through explicit
request_redelivery, using a validated raw retry sample and identical frozen
bytes, attempts and key. No additional persisted field or format change is made;
receipt still prevents automatic retry until explicitly rearmed.

A received final outcome records its known state/result with reconciled false
in the receipt transaction; it is not yet a completed local correction. App
outcome delivery waits for local processing. A rejection may be processed without
automatic rollback. No applied/corrected command can be cancelled. Once cancelled,
late replies cannot change state or rows. Unsent cancellation may retain an
unrolled conflict record; after safe explicit rollback it is resolved history.
Nonempty final mappings require a successfully reconciled applied/corrected
outcome; queued, sent, acknowledged, rejected, cancelled and discarded records cannot assert
authoritative mappings. A cancelled record may retain a backend rejection only
when frozen bytes and positive attempts prove it had been sent. Unsent
cancellation cannot manufacture an authoritative outcome.

The decoded ledger verifies unique key/order/local-ID ownership, required
envelope/context shape, legal state/receipt/frozen/outcome combinations, valid
reference paths and resolvable dependency ownership without cycles. Do not trust
a stored live-count bit. Derive whether recovery is live from actual records:
queued/sent/acknowledged, terminal outcomes needing local reconciliation,
unrolled rejected work and sent-cancel tombstones. Retained producer records
needed by live dependency maps are also live. Already resolved diagnostics and
unneeded final mappings are count-bounded by history; they alone do not block
schema lifecycle. Namespace/sequence/counters must not be regenerated at reopen.
Producer retention is driven by needed local identities/dependency edges, not
merely by equal authoritative IDs in later args/undo references. An already
mapped authoritative ID is usable independently of its bounded historical map;
a new-schema consumer must not reactivate old-schema history merely by using it.

At declared reference paths, String values beginning `local:` are unresolved or
cached local identities and require a retained producer; other nonempty String
values are authoritative IDs. This classification applies only to declared
paths, never arbitrary business strings. Dependencies equal the set of unmapped
declared local references; references whose mapping already exists must have
been rewritten by the atomic mapping transaction. Created command IDs require
the `local:` prefix with a nonempty suffix, and final mapped IDs must be nonempty
and outside that prefix. A custom application ID generator that violates this
command-only rule rejects before optimistic commit; ordinary API-0004 generator
semantics are preserved. Unknown/evicted local references reject explicitly.

## Catalog and schema lifecycle

Only after common exact decoding may adapters exclude `__hoya_sync` from
application inventory, row-version checks and schema candidate validation.
Catalog and same-version open remain usable with live work. Before any schema
plan operation, inside its transaction/candidate, reject if live recovery exists.
This covers complete migrations and missing-chain reset without modifying
accepted ADR bodies. Valid fully resolved history can remain through migration;
each historical envelope keeps its original schema version. Unknown corrupt
recovery metadata always rejects unchanged, including empty application schemas.
Recovery bookkeeping without accompanying managed MODEL_TABLE/state is orphaned
and rejects; it cannot be adopted as unversioned legacy application data.

## Acceptance and rollback — TC-0032 C06/C07/C11/C17

Before implementation, add compiling behavioral RED tests to shared conformance
or each adapter's existing integration tests for absent-table initialization,
data+model+record atomicity, real commit failure, full prior-state CAS, replace
and remove guard mismatch without counter change, ordered repeated-row writes,
reserved/invalid/future packet rejection, reopen identity, live-work migration
rejection and queue-free regression preservation. Codec tests include every
record/state and typed row roundtrip, missing/extra fields, invalid graphs,
duplicate ownership, corrupt version and aggregate records above 1 MiB while
each record remains bounded. Real process/browser kill boundaries are additional
integration gates, not implied by adapter ordinary-error rollback tests.

Source rollback preserves stores and requires compatible readers for recovery
metadata. Do not delete the reserved table to make old code open a live store.
Only synthetic test files and namespaces are modified; no user data operation.
