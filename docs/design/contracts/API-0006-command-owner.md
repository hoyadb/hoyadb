---
id: API-0006
title: Portable command values and host-driven command owner
type: api
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-23
updated: 2026-11-10
parents: [ARC-0001, ARC-0002]
satisfies: [REQ-0011, REQ-0012, REQ-0015, REQ-0043]
depends_on: [SPEC-0029, ADR-0032, API-0004, API-0005, SPEC-0040]
tags: [phase-four, commands, rust, contract]
---

# API-0006: Portable command boundary

Unpublished Rust API in `hoyadb_core::command`. Section 1 is the first exact
implementation slice. Owner/storage/host operation sections are added and
reviewed before their implementation; they are not implicitly defined by names
in this document. ADR-0032 adopts the operator-approved RFD-0003 policies.

## 1. Structured command values

```rust
pub enum Json {
    Null,
    Bool(bool),
    String(String),
    Integer(i64),
    Float(hoyadb_core::storage::FiniteFloat),
    Array(Vec<Json>),
    Object(std::collections::BTreeMap<String, Json>),
}
pub enum JsonError {
    InvalidUtf8,
    InvalidSyntax,
    InvalidNumber,
    DuplicateKey,
    LimitExceeded,
}
// All three methods return errors without payload text or logging.
impl Json {
    pub fn parse(input: &str) -> Result<Self, JsonError>;
    pub fn from_bytes(input: &[u8]) -> Result<Self, JsonError>;
    pub fn canonical(&self) -> Result<String, JsonError>;
}
```

Values/errors derive Clone, Debug, PartialEq and Eq, but callers must not log
value Debug representations containing Confidential data. Errors contain no
payload or field values. Decoding accepts only JSON whitespace, exact grammar,
one complete value, valid UTF-8 and valid paired UTF-16 escape sequences.
Duplicate decoded object keys reject, including escaped equivalents. Negative
or positive out-of-range integral lexical tokens reject as InvalidNumber, rather
than silently converting to Float. Decimal or exponent forms use finite f64;
overflow rejects, underflow may round to zero, as with the existing FiniteFloat.
An application needing more decimal precision must supply a String explicitly.

Canonical objects use Rust String/BTreeMap key order, not insertion order. Emit
Unicode as UTF-8, escape quotes/backslashes, and emit controls with lowercase
`\u00xx`. Integers use decimal; floats use finite round-trip decimal with `.0`
when required to preserve Float versus Integer on parse. Negative zero uses
FiniteFloat's existing positive-zero canonicalization. Arrays preserve order.
Canonicalization must parse back to the same value for every supported variant.
Canonical output, rather than arbitrary input whitespace, defines frozen bytes.

Input and output are each limited to 1048576 UTF-8 bytes, inclusive. Container
nesting is limited to 64, inclusive, with a scalar root at depth zero. Total
value nodes are limited to 100000, inclusive; object keys are not nodes. Decoder
checks bounds before allocation/recursion where applicable; encoder also checks
caller-constructed trees and escape expansion. Returning a bounded result does
not imply arbitrary caller-built trees or all database snapshots have bounded
total memory. No parser dependency, runtime/network call or debug export exists.

### Section 1 acceptance — TC-0032 C05

Before implementation, write unit regressions at
`crates/hoyadb-core/src/command/json.rs` proving canonical Unicode/control values,
all variants, exact i64 extrema, finite Float roundtrip, valid surrogate pairs,
invalid UTF-8, lone/reversed surrogates, duplicate escaped keys, number grammar,
overflow/underflow, and both sides of all three bounds. Run against a compiling
functional stub and retain the observed behavioral RED; then implement minimally
and verify GREEN, workspace fmt/lint and the core WASM build. A stub itself is
only a RED harness step and is not deliverable implementation.

The existing OpenAPI command envelope remains the HTTP reference contract;
this section defines its bounded in-core JSON representation, not a new wire
envelope. JavaScript integer conversion and concrete host marshaling are tested
in the later exact bridge slice before any platform claim.

## 2. Shared in-process staging implementation

The existing `hoyadb_storage_memory::MemoryStorage` implementation moves into
the dependency-free core as doc-hidden `storage::SnapshotStorage`; the memory
package re-exports it under its existing name. This is an implementation
ownership move, not a new storage behavior, schema format or query operator.
Keep its complete CRUD, ordered atomic cloning, typed evaluator, indexes,
projection and managed schema lifecycle semantics and diagnostic candidate_count.
No core dependency on the memory package or copied second evaluator is permitted.

Existing workspace conformance is the behavioral bracket: run the existing
171-test suite before extraction (recorded in the Phase 4 baseline), move the
implementation mechanically, run those same tests after, then fmt/Clippy and
core/memory WASM. New command staging behavior requires its own meaningful RED
after extraction; existing green tests alone do not establish new scope behavior.
The staging owner will load an owned snapshot at admission and reuse the existing
Database query/write/relation behavior. Snapshot lifecycle costs O(all stored
application rows), distinct from ordinary engine-indexed query execution.

## 3. Internal execution coordinator and staging scope

These crate-private mechanisms support the later public command owner; they are
not a second public unqueued command API. A native command owner whose storage
reports StoreOwnership::Exclusive uses its own coordinator (ADR-0037; SPEC-0037 R3). Unknown custom adapters and all WASM
owners retain ExecutionCoordinator::global(), the process/worker singleton.
Checked process-global owner and request allocators are shared by every
coordinator; tickets cannot alias across coordinators or reopen. Each
coordinator queues requests FIFO. A valid
non-head request is pending; a forged, cross-owner or cancelled request errors.
Admission returns an owned permit only when the head request has no predecessor
active. Dropping that permit releases only its own matching active ticket.
Dropping an owner cancels its queued work and releases its active permit. Mutexes
are held only during synchronous bookkeeping, never over storage/host awaits.
All public command-owner writes/reconciliation reserve an owned idle permit
through completion, rather than checking idle and releasing before an await.

`StagedScope` is admitted before loading all declared application rows and model
metadata into SnapshotStorage. Snapshot initialization holds an owned permit in
a drop guard: failed/dropped initialization releases admission and changes no
committed rows. A scope's existing Database supports fetch/query navigation,
ordered apply_batch, insert/replace/remove/link/unlink with the same schema
validation. External readers continue to use the committed Database. No storage
transaction remains open while the foreign handler runs.

Retain a configured nonclone ID generator in shared private custody instead of
moving its sole ownership out of a surviving Database: represent it once by a
cloneable Arc/Mutex-owned callback, and let committed/staged Databases share that
private generator handle. No public callback type/signature changes. Dropping an
initialized scope cannot replace the committed generator with None; its next
generated value continues the same callback state. The coordinator prevents
simultaneous writes through the supported owner. Default next_id changes stay
staged. User generator external side effects cannot be rewound and are not
promised rollback. Prepare an ordered log of exact staged
IDs/before/after changes and final ModelState; do not replay high-level inserts
against the committed Database, because that allocates different IDs. Restore
managed row-version cells before storage commit. Empty/net-zero handlers still
need envelope/key persistence in the later atomic enqueue slice. No standalone
scope finish may expose data without that slice's durable command record.
Preparation transfers the owned active permit into the prepared commit object;
it must remain held through durable enqueue/data commit or abort/failure cleanup.
Preparing a log alone cannot admit the next waiting handler.

### Section 3 acceptance — TC-0032 C01–C04

Write compiling behavioral RED tests for two-owner FIFO admission, cancelled
queued request, forged/stale/cross-owner tokens, permit/owner-drop release,
staged relation reads after ordered writes, unchanged external reads/counter,
abort and prepared exact IDs/versioned log. Include initialized-scope disposal
followed by the next insert still using its configured generator, and a second
request remaining pending after preparation until its prepared permit releases.
Unit tests may use a private isolated coordinator to avoid cross-test scheduling;
production construction selects the reviewed ownership capability and target
as defined above, rather than assuming all adapters are exclusive.
These establish mechanism behavior only. Public owner admission/finish, actual
suspended foreign handlers, failure/cancel integration and notifications require
their own subsequent integration RED/GREEN and cannot be claimed by these tests.

## 4. Public local command owner and atomic enqueue

`CommandDatabase<S: SyncStorage>` consumes its storage and owns the committed
ReactiveDatabase, one registered ExecutionOwner, the decoded SyncLedger
and at most one private StagedScope. It exposes read-only Deref to Database for
existing QueryBuilder fetch compatibility, never DerefMut, mutable storage or an
unqueued scoped commit. Fields remain private outside the command module.

The exact first owner surface is:

```rust
pub enum CommandError {
    Storage(StorageError), InvalidCommand, InvalidTicket, Busy, Capacity,
    InvalidPayload(JsonError), Protocol, UnknownLocalReference,
    RollbackConflict, Closed, LimitExceeded,
}
pub enum ArgumentType { Any, String, Integer, Float, Boolean, Array, Object }
pub struct Argument { pub kind: ArgumentType, pub optional: bool }
pub struct CommandDefinition {
    pub name: String,
    pub arguments: BTreeMap<String, Argument>,
    pub reference_paths: Vec<Vec<PathPart>>,
}
pub struct CommandRequest(/* private checked ExecutionRequest */);
pub struct CapturedContext { /* private immutable identity + sampled ms */ }
pub struct CommandResult { pub key: String, pub result: Json }
impl CapturedContext {
    pub fn identity(&self) -> &Json;
    pub fn now(&self) -> i64;
}
impl<S: SyncStorage> CommandDatabase<S> {
    pub async fn open(storage: S, schema: SchemaDefinition,
        migrations: Vec<Migration>, new_namespace: [u8; 16], options: SyncOptions)
        -> Result<Self, CommandError>;
    pub fn set_id_generator(&mut self,
        generator: impl FnMut() -> String + Send + 'static) -> Result<(), CommandError>;
    pub fn request(&self) -> Result<CommandRequest, CommandError>;
    pub fn cancel_request(&self, request: CommandRequest) -> Result<(), CommandError>;
    pub async fn begin(&mut self, request: CommandRequest,
        definition: CommandDefinition, args: Json, identity: Option<Json>, now_ms: i64)
        -> Result<bool, CommandError>;
    pub fn scope_args(&self, request: CommandRequest) -> Result<&Json, CommandError>;
    pub fn scope_context(&self, request: CommandRequest)
        -> Result<&CapturedContext, CommandError>;
    pub async fn scope_fetch(&self, request: CommandRequest, query: &QueryBuilder)
        -> Result<Vec<Row>, CommandError>;
    pub async fn scope_apply_batch(&mut self, request: CommandRequest, writes: Vec<Write>)
        -> Result<Vec<CommittedChange>, CommandError>;
    pub async fn scope_insert(&mut self, request: CommandRequest, table: &str,
        fields: BTreeMap<String, Value>) -> Result<Row, CommandError>;
    pub async fn scope_replace(&mut self, request: CommandRequest, table: &str,
        id: &str, fields: BTreeMap<String, Value>) -> Result<Row, CommandError>;
    pub async fn scope_remove(&mut self, request: CommandRequest, table: &str, id: &str)
        -> Result<bool, CommandError>;
    pub async fn scope_link(&mut self, request: CommandRequest, source: &str,
        id: &str, relation: &str, target: &str, fields: BTreeMap<String, Value>)
        -> Result<Row, CommandError>;
    pub async fn scope_unlink(&mut self, request: CommandRequest, source: &str,
        id: &str, relation: &str, target: &str) -> Result<usize, CommandError>;
    pub async fn finish(&mut self, request: CommandRequest, result: Json)
        -> Result<CommandResult, CommandError>;
    pub fn abort(&mut self, request: CommandRequest) -> Result<(), CommandError>;
    pub fn close(self);
}
```

CommandRequest is Copy/Clone/Eq/Debug with private internals; callers must cancel
an unused queued request or close its owner. CapturedContext is immutable through
its API; host mutation of a copied object cannot alter the captured envelope.
CommandDefinition requires a valid versioned name, bounded Object arguments,
exact declared top-level fields and required/optional Argument types. Optional
omission/Null is allowed; required values must match. Any permits any bounded
JSON value. Array/Object here constrain container kind, not business rules or a
full JSON Schema implementation. Handlers own nested business validation; explicit
reference paths still must resolve to nonempty Strings. Invalid definition/args/
identity/time cancel the submitted request without handler admission or data
mutation. Definition paths may specify concrete array indices for that invocation.
set_id_generator configures the existing shared callback through the public
owner while holding an idle permit; replacement during a scope or queued
admission rejects. It changes future ID allocation only and never rewinds prior
callback effects or reassigns committed IDs. This method supports testing the
command-only local-ID rule without private-field injection.

open validates options and known recovery metadata, registers with its selected
coordinator (SPEC-0037 R3) and holds an idle permit across Database's existing
normalized schema lifecycle, since opening may initialize or migrate rows. It
cannot race another active handler of the same coordinator. For a fresh
command store, new_namespace supplies platform entropy and options become its
initial header. Existing persisted namespace/options are authoritative: ignore
the unused fresh namespace but reject conflicting options, never silently reset
identity or timing. The first successful command initializes the reserved table
atomically with its writes; opening a queue-free store alone adds no recovery
table. Missing context returns InvalidCommand and never authenticates itself.
now_ms must be within the supported nonnegative UTC year range; capture once.

begin returns false only when a valid queued request is not yet admitted, and
invokes no handler. Once admitted, snapshot and context are installed; the host
then invokes its handler with a tracked scope wrapper. Validate capacity before
handler admission and recheck at commit. Also before admission, apply finish's
per-record preflight to the queued record the normalized args and context imply
without staged images: a nesting or size that can never be stored returns
LimitExceeded and releases the request (TC-0032 C05). Resolve known cached local references
at declared paths before exposing scope_args; unresolved parents remain local
and record dependencies. Unknown/evicted local IDs reject. New invocations also reject references to
cancelled, discarded or rolled-back producers with UnknownLocalReference: retained
tombstones cannot create a future mapping or admit permanently blocked work. Scope operations
reuse existing query/relations and writes, including read-your-own-writes.
The tracked wrapper exposes no request/execute, so nested scope execution cannot
queue itself behind its own handler. Independent outer execute calls queue FIFO.

finish consumes the matching staged scope, validates a bounded returned result
and local created-ID ownership, and builds one SyncPlan: initial row guards,
ordered exact versioned mutations, prepared final ModelState, generated key and
queued record with ordered undo, dependencies, immutable context and next header
sequence/clock. Never replay high-level Insert. Preflight queued encoding and
the future envelope's frozen-copy encoding against API-0007's per-record bound
before optimistic commit, including for blocked records. Passing the args bound
alone does not promise that args plus undo plus frozen copy fit a recovery row.
The codec's size-only check retains all metadata and does not release or fake
dependency state. Reserve the maximum scalar width of attempts, due time and
receipt state as well as the frozen copy, so a queued record accepted at the
bound cannot become unsendable or unacknowledgeable solely from those fields
growing; this is a byte-budget check, not a state transition. Validate the retained permit
immediately before synchronous atomic apply_sync_plan. Update committed model/
ledger in memory without another await after success, then publish application
changes through the existing reactive owner. A postcommit observation error is
an observation event, never a false failed command. Abort/failed finish discard
staging and release the permit, with no committed rows/envelope/counter change.

Public non-command apply_batch/insert/replace/remove/link/unlink forward the
existing API-0004/0005 behavior while holding an owned idle permit through the
operation. They are local-only writes in this phase, not implicit entity diffs.
Committed observe/observe_count/observe_with_columns, unsubscribe, notification
drain, stats and explicit refresh forward to the reactive owner; no callback
runs under a retained core borrow. Queue/status/outcome operations are specified
in the next reviewed section, not inferred here.

### Section 4 acceptance — TC-0032 C01–C06/C08

Before owner implementation add compiling behavioral RED tests for complete
awaited read-your-own-writes/atomic enqueue, two-owner FIFO, unchanged outside
reads, exact single postcommit notification, failure/abort/stale request and
invalid args/context/time, default generated keys/counter persistence, empty
successful handlers, prepared exact IDs, declared local-reference blocking and
mapped-reference normalization, rejected unsafe custom IDs, oversized future
frozen-record rejection before optimistic commit, and direct writes
excluded while a scope holds admission. Use existing transactional memory and
native SQLite integration; generic mechanism tests alone cannot prove this API.
Retain all existing foundation/mechanism/codec regressions. Actual foreign
handler suspension/cancellation and browser integer conversion remain later
binding gates and must not be claimed from Rust-only owner tests.

## 5. Push, reconciliation, rollback and owned events

This section completes the command-only sync surface. Export SyncOptions,
PathPart, RemoteOutcome and SyncState from the record types without changing
their exact persisted representation. Additional public types are:

```rust
pub enum ConflictReaction { Stop, Discard }
pub struct ConflictPolicy {
    pub default: ConflictReaction,
    pub commands: BTreeMap<String, ConflictReaction>,
}
pub enum Resolution {
    Completed,
    KeptCurrent {
        discarded_keys: Vec<String>, unmapped: Vec<String>, skipped_writes: usize,
    },
}
pub struct PushBatch { pub envelopes: Vec<String>, pub next_due_ms: Option<i64> }
pub enum ReconcileWrite {
    Put { table: String, id: String, fields: BTreeMap<String, Value> },
    Remove { table: String, id: String },
}
pub struct ReconcilePlan {
    pub writes: Vec<ReconcileWrite>,
    pub mappings: BTreeMap<String, String>,
}
pub struct CorrectionGuard {
    pub table: String, pub id: String,
    pub expected: Option<BTreeMap<String, Value>>,
}
pub enum CommandOutcome {
    Queued, Sent, Acknowledged, Applied(Json), Corrected(Json), Rejected(String),
}
pub struct QueueEntry {
    pub key: String, pub state: SyncState, pub blocked_on: Vec<String>,
    pub awaiting_local: bool, pub awaiting_key: Option<String>, pub rolled_back: bool,
}
pub struct QueueStatus {
    pub pending: usize, pub sending: usize, pub awaiting_outcome: usize,
    pub live: usize, pub blocked: bool, pub commands: Vec<QueueEntry>,
}
pub struct SyncSubscription(/* private globally unique u64 */);
pub enum CommandEvent {
    Data(Notification),
    Status { subscription: SyncSubscription, value: QueueStatus },
    Outcome { subscription: SyncSubscription, value: CommandOutcome },
}
```

CommandOutcome carries no correlation field or network code. A keyed operation
or subscription token supplies routing outside that value. QueueEntry intentionally
does carry a key for explicit inspection/cancel. blocked_on contains all sorted
unresolved local IDs; a TypeScript bridge may expose the historical blockedOn
String for the one-parent case plus blockedOnAll for the full vector. QueueStatus
blocked means live capacity is exhausted, not merely that one dependency waits.
pending counts queued entries, including dependency-blocked; sending counts sent;
awaiting_outcome counts receipt-only entries; live includes all recovery records.
commands includes live work and bounded retained history in execution order,
without arguments, identity, frozen bytes, undo or result/reason payloads.

```rust
impl<S: SyncStorage> CommandDatabase<S> {
    pub fn prepare_push(&mut self, now_ms: i64, jitter_basis_points: u16)
        -> Result<PushBatch, CommandError>;
    pub fn receive(&mut self, key: &str, outcome: Option<RemoteOutcome>)
        -> Result<(), CommandError>;
    pub fn request_redelivery(&mut self, key: &str, now_ms: i64)
        -> Result<(), CommandError>;
    pub fn pending_reconciliations(&self) -> Vec<(String, RemoteOutcome)>;
    pub fn reconciliation_context(&self, key: &str) -> Option<(QueuedWriteKind, BTreeMap<String, String>)>;
    pub async fn reconcile(&mut self, key: &str, plan: ReconcilePlan)
        -> Result<(), CommandError>;
    pub async fn reconcile_guarded(&mut self, key: &str, plan: ReconcilePlan,
        guards: Vec<CorrectionGuard>) -> Result<(), CommandError>;
    pub fn set_conflict_policy(&mut self, policy: ConflictPolicy)
        -> Result<(), CommandError>;
    pub async fn reconcile_resolving(&mut self, key: &str, plan: ReconcilePlan,
        guards: Vec<CorrectionGuard>) -> Result<Resolution, CommandError>;
    pub async fn rollback_resolving(&mut self, key: &str)
        -> Result<Resolution, CommandError>;
    pub async fn discard(&mut self, key: &str) -> Result<Resolution, CommandError>;
    pub async fn rollback(&mut self, key: &str) -> Result<(), CommandError>;
    pub async fn cancel(&mut self, key: &str) -> Result<(), CommandError>;
    pub fn forget(&mut self, key: &str) -> Result<(), CommandError>;
    pub fn queue_status(&self) -> QueueStatus;
    pub fn observe_queue_status(&mut self) -> Result<SyncSubscription, CommandError>;
    pub fn observe_outcome(&mut self, key: &str) -> Result<SyncSubscription, CommandError>;
    pub fn unsubscribe_sync(&mut self, subscription: SyncSubscription) -> bool;
    pub fn drain_events(&mut self, now_ms: u64) -> Vec<CommandEvent>;
}
```

### Explicit conflict policy (SPEC-0040)

This is the approved contract before implementation, to be verified under TC-0044.
`ConflictPolicy::default()` is Stop with an empty override map. An exact recorded
command-name override wins over the default; an unknown but valid versioned
name is accepted and inert. Validate values, existing command-name syntax and
codec size limits before installing the policy. Hold an idle permit; Busy or
invalid configuration leaves the previous policy intact. The policy is per open
owner, ephemeral and separate from persisted SyncOptions. A reopened core owner
starts at Stop until configured again.

The ordinary correctness path runs first, including SPEC-0039 and SPEC-0038 C4.
Only an unresolved closure-proof mismatch reaches policy. Default Stop returns
the existing conflict. Configured Discard on rollback applies only to a processed
Rejected root; an unrolled Cancelled-root mismatch still returns RollbackConflict.
Explicit `discard` separately admits never-sent Queued or never-sent unresolved
Cancelled roots, and processed Rejected roots under SPEC-0040's whole-closure eligibility.
A processed rejection that a conflicted cancel left Cancelled and unresolved keeps
that eligibility through its stored Rejected outcome.
All in-flight, later-work and EntityRecord refusals in SPEC-0040 remain binding.
`cancel` never consults the policy.

`reconcile_resolving` takes the same SPEC-0039 expectations as
`reconcile_guarded`; an empty vector supplies none. On correction-discard, apply
valid mappings, skip all plan writes, and confirm Applied/Corrected without a
convergence claim. Commit every caller expectation as a storage RowGuard even
when its write is skipped. A commit-time guard race remains RollbackConflict;
it never re-enters policy. All earlier validation keeps its error and precedence.

The existing reconcile, reconcile_guarded and rollback methods delegate to the
same planner and discard only the report. They preflight the same bounded result
before persistence, so an oversized report is LimitExceeded with no change even
for a unit-returning caller. Completed means the ordinary path completed.
KeptCurrent reports newly discarded keys and unmapped created IDs on rollback
or manual discard, with skipped_writes zero. Correction-discard has empty lists
and the exact count of skipped plan writes. Discarded is a queue status, not a
new CommandOutcome; it never claims a successful inverse or accepted command.

Every newly discarded record also clears reference_paths and dependencies in
that same atomic plan. These are retired dispatch bookkeeping; args, created,
undo, frozen bytes, attempts, due_ms, outcome and mappings are preserved. Its
state becomes Discarded, reconciled is true and rolled_back is false. This applies
to eligible Queued, processed Rejected and never-sent unresolved Cancelled records.
Existing reference and DAG validation stays in force. Non-live Discarded history
does not extend producer retention. Preservation describes the discard
transition: later ordinary producer mappings still rewrite historical undo
identities and Reference values, including on Discarded records, while their
args and frozen bytes stay unchanged. Discarded records do not rejoin a later
producer's dependent closure. Discarding only a dependent while its producer is
live is allowed. A later producer rollback can remove the producer's row while
the kept dependent row still references it; existing Reference typing does not
guarantee target existence, and discard adds no such guarantee.

### Dispatch and receipt

prepare_push holds an idle permit and retains persisted last_ms as a monotonic
identity floor. It uses the validated raw sample for retry scheduling and selects
queued entries with no dependencies plus due sent retries in local execution
order. SPEC-0038 additionally withholds a Queued record while an earlier record
that wrote any of the same rows is unresolved: Queued, Sent or Acknowledged;
Applied or Corrected and not reconciled; or Rejected and not rolled back.
Entity-entity pairs are exempt; already Sent records are never withheld.
QueueEntry.awaiting_key names the earliest blocker, or None. Existing local-ID
dependencies retain their own meaning. Same-row progression requires explicit
reconciliation; a host without that callback must reconcile manually. This orders
recorded write overlap, not read-only dependencies or unrecorded backend effects.
Freeze each first-send envelope, increment attempts,
and persist Sent/deadline before returning any bytes. Jitter samples are 0..2500
basis points; next base delay is initial_delay * multiplier^(attempts-1), capped
with checked/saturating arithmetic at max_delay. Add bounded positive jitter to
that base; cap applies to the base, so actual delay may be up to 125% of it.
Do not resend acknowledged/final/cancelled work. Empty eligible work returns an
empty batch and the next unblocked retry deadline. Transport is called only for
nonempty arrays; String elements are canonical object bytes, joined as one array
without a parse/re-encode roundtrip that could change i64/Float values.
For clock recovery, cap a Sent deadline only when it exceeds the raw sample plus
the maximum configured backoff including maximum jitter, using wide checked
arithmetic (SPEC-0035). Persist the cap even on an empty send; it never extends
a deadline. Eligibility uses the raw sample, so a backward sample can defer a
send the old identity floor treated as due. Keep key construction clamped and
frozen retry bytes unchanged. TC-0039 covers CL01-CL03; explicit recovery of a
lost final outcome after acknowledgement is defined by SPEC-0038 below.

Host wrappers wake this method after successful finish, explicit retry, transport
completion and at next_due_ms. Wakes respect backoff. The developer's Transport
owns actual push, request timeout, cancellation and auth/TLS; core owns no network
or timer. Transport failure supplies no receipt. Fake and HTTP examples use this
same seam and return independently keyed outcomes, not positional results.

receive holds an idle permit and durably records receipt/outcome before local
callbacks. None means receipt-only; a final outcome implies receipt and sets its
known state with reconciled false. Identical retained duplicates are no-ops;
conflicting outcomes or unknown keys are Protocol. A sent Discarded record keeps
an identical stored Rejected outcome or receipt-only None as a no-op; conflicting
outcomes are Protocol. Any receipt or outcome for a never-sent Discarded record
is Protocol. A known cancelled tombstone
ignores late receipt/outcome without changing rows or reviving retry. Payload/
storage errors are typed and preserve the prior transaction, never a false local
correction. No valid result is silently truncated to fit record limits.

SPEC-0038 adds request_redelivery(key, now_ms). Hold an idle permit and validate
the same raw timestamp range as prepare_push. An Acknowledged record without
outcome becomes Sent, due at that raw sample, with identical frozen bytes,
attempts, key and last_ms. Sent is a no-op; other states or unknown keys return
Protocol. Persist atomically; never transmit inside this operation. A later
prepare_push performs the retry. Backend deduplication remains required.

### Authoritative local apply

pending_reconciliations is a host-internal keyed work list, not CommandOutcome.
The application translates arbitrary backend result into ReconcilePlan. Rejected
outcomes require an empty plan and become processed without automatic rollback.
Applied/corrected plans must map every surviving locally created ID, and cannot
map IDs owned by a different command. Reject occupied target IDs, duplicate IDs,
local-prefixed final IDs, missing mappings and invalid schema fields unchanged.

Hold an idle permit through snapshot/plan preparation and commit. An Applied or
Corrected plan without writes skips the inverse proof while retaining all mapping,
rename, reference rewrite, dependent undo and queued normalization checks
(SPEC-0038). This changes only the proof for confirmation; it does not drop a
correction automatically. An application choosing an empty plan for Corrected
owns that translation choice, not a convergence claim. A plan with writes keeps
the command closure's optimistic after-image proof against current rows;
independent later edits cause RollbackConflict. Mapping renames owned rows,
rewrites only declared queued argument paths and declared schema Reference cells,
and updates retained dependent undo before/after references to the same final
identities. It never edits arbitrary String fields or frozen wire fields. Remove
resolved dependency edges only with the corresponding map/data transaction.
Put/Remove plans use current guarded typed rows; application fields cannot set
id or reserved metadata. Validate all original/target rows and commit mutations,
maps, dependent changes and reconciled outcome state in one SyncPlan. Publish
data changes after durability, then the owned outcome/status events. Ordinary
subscription/host error delivery remains the existing reactive mechanism.

SPEC-0039 tightens cross-row correction writes. Ordinary reconcile supplies no
CorrectionGuards. Every ordinary Put/Remove outside the command closure requires
an explicit guard through reconcile_guarded, including expected None for an
absent row. The expectation is the complete public before-image without managed
version fields, compared before planned identity/reference rewrites. No guard
means RollbackConflict; a mismatching guard also means RollbackConflict. Duplicate,
unused or invalid guards are InvalidCommand; a guard on a closure-owned row or
its mapping target is unused. Matching guards cannot overwrite another live
record's undo rows. Outside-closure writes on entity-managed tables and typed
Entity corrections outside the closure remain refused, protecting comparison
metadata. These checks do not relax the existing proof for nonempty plans.
Guard checks, mappings, row changes and recovery state commit atomically; a
failure publishes nothing. The app explicitly consents to replacement of the
value it read, not to replacing a later intervening edit. No automatic retry
refreshes an expectation. Existing unguarded server-created-row inserts must
also adopt expected-absence guards; this is a disclosed compatibility tightening.

Reconciliation validates the key and eligible state first (Protocol), then plan
write shape (InvalidCommand), then guard structure (InvalidCommand), before data
conflicts and the existing closure proof (RollbackConflict). A malformed guard
expectation returns InvalidCommand; ordinary Put row validation retains its
existing error mapping. At atomic commit, only the exact API-0007 guard-conflict
error maps to RollbackConflict for reconcile, including ordinary reconcile's
derived-guard race. Other storage SchemaConflict errors remain Storage errors.
Rollback, cancel, pull and merge keep their existing storage-error mappings.
This changes the previous ordinary-reconcile guard-race Storage(SchemaConflict)
result and makes an invalid plan take precedence over a closure conflict.
SPEC-0038's CTRL-K characterization is superseded by TC-0043: an unguarded
outside-closure correction now conflicts and keeps the direct edit.


### Explicit inverse operations and retention

rollback supports a processed rejected command or an unrolled cancelled command;
it is idempotent after a completed inverse. A Discarded target returns Protocol,
including when a previous configured rollback kept current data. SPEC-0040's
configured Discard applies only to the processed Rejected case. Build the transitive dependent closure,
reverse execution order and each ordered undo log. Simulate against one current
snapshot, comparing each inverse's expected after-image before restoring its
before-image. A mismatch returns RollbackConflict with no partial inverse apply.
Use initial guards in the final SyncPlan. Preserve unrelated rows and never rewind
the model ID/key counters. Retain undo on conflicts; clear unnecessary undo after
successful rollback or completed applied/corrected reconciliation.

cancel supports queued, sent and rejected work. Stop retry durably for the full
blocked dependent closure. Attempt the same guarded rollback; on conflict retain
Cancelled recovery and undo while returning RollbackConflict, without overwriting
application rows. Completed unsent cancellation is resolved history. Sent
cancellation always retains a minimal live tombstone until forget, even after
safe rollback. That fully rolled-back tombstone does not block pulled insertion
of other rows into its table (API-0009 section 4). It cannot retract server-side actions. forget permits a fully
rolled-back sent tombstone or fully resolved history with no live dependents;
it cannot erase pending rollback or unacknowledged uncancelled work. Late forgotten
keys are unknown and cannot recreate local state. A Discarded producer may be
forgotten while its Discarded dependents remain in history: their retired
reference_paths and dependencies are already empty, so no recovery edge is
stranded. New references to its unmapped local IDs still reject with
UnknownLocalReference, both before and after producer removal.

Bound fully resolved history to the newest configured count. Fully reverted
unsent cancelled history may clear now-unneeded reference_paths/dependencies and
created ownership after inverse completion, preserving its diagnostic payload
without requiring an evicted producer. This is recovery cleanup, not a change to
any sent envelope. Discarded history keeps created, undo and frozen bytes until the record itself
is trimmed or forgotten; the Cancelled cleanup must not clear those fields on a
Discarded record. Live records/maps/rollback/tombstones are never evicted. If an
active historical dependency still requires a producer, retain that producer as
live rather than deleting a referenced identity. Lifecycle gates remain API-0007.

### Registration and owned host delivery

Registration queues an initial owned status/current outcome without side effects
on failure; allocate checked globally unique subscription IDs only on success.
A final received outcome awaiting local processing observes as Acknowledged until
reconcile completes. Cancelled commands have no invented CommandOutcome variant;
their state is available through QueueStatus, and outcome registration rejects.
Each persisted transition queues changed status/outcome values for its live
watchers, suppressing equal repeats. drain_events combines existing Data events
and queued owned sync events, with data before outcome from a correction. No
callback occurs while owner state is borrowed. Unsubscribe removes pending events;
close drops all watchers. Host wrappers check owner/token liveness before every
callback, use the existing caller-thread/worker-main scheduling contract, and
dispose subscriptions after cancellation/close. Subscription state is ephemeral.

### Section 5 acceptance — TC-0032 C09–C20

Behavioral RED precedes push/receipt, reconciliation, inverse and event slices.
Use the same memory/native fixtures and browser shared assertions. Cover lost/
partial/reordered/duplicate/conflicting responses, empty and length-one arrays,
ACK-only restart, frozen bytes after mapping/restart, transitive/multi-parent
release, actual correction of several rows, exact rollback cascades, overlap
conflicts, cancelled late replies, explicit forget and bounded eviction, capacity
including attention/tombstones, retained deadlines and jitter extremes, status/
outcome cancellation and complete postcommit data snapshots. Run actual native/
browser handlers and a task-owned loopback HTTP reference; fake-only and Rust-
only green cannot establish those host/network-boundary claims.

### Read-only reconciliation context — API-0012

`reconciliation_context` reads the retained recovery ledger with no admission or
mutation. It returns the record kind and its exact created-ID-to-table map only
for a record included in `pending_reconciliations`. Unknown keys, Cancelled or
reconciled records, and records with no retained outcome return `None`. It never
returns args, undo, frozen bytes, identity or outcome payload.
