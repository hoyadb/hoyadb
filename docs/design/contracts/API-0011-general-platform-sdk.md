---
id: API-0011
title: General schema platform facade owned host scopes and consumer packages
type: api
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-11-10
parents: [ARC-0001, ARC-0002]
satisfies: [REQ-0046, REQ-0034, NFR-0001, NFR-0002, NFR-0004, NFR-0005]
depends_on: [RFD-0006, SPEC-0033, API-0004, API-0005, API-0006, API-0007, API-0008, API-0009, API-0010, TC-0036, TC-0037, SPEC-0040]
tags: [phase-seven, contract, bindings, packaging]
---

# API-0011: General schema platform facade owned host scopes and consumer packages

Reviewed preparation precedes TDD. RFD-0006 records the remaining exact graph
approval and ID-example decision; independent work uses existing approved
dependencies. No packaged or mobile implementation is claimed by this contract.

## 1. Shared implementation and boundary limits

Create unpublished workspace crate `hoyadb-bindings`, depending on existing core,
memory and protected SQLite (no new external Rust runtime). A private Engine enum
forwards Storage/Transactional/Typed/Schema/Observation/Sync capabilities to real
adapters; owner is CommandDatabase<Engine>. Codec/handle/lifecycle code lives here
and is reused by browser/native exports, not copied into each language. The general WASM export crate lives in `crates/hoyadb-web`. Thin
native exports live in `crates/hoyadb-native`, JS packages in `packages/client` and `packages/testing`,
Swift in `bindings/swift`, Kotlin in `bindings/android`, and RN native adapter in
`packages/react-native`. These source roots exist; packages remain local and unpublished.
Legacy spikes/common fixtures remain test fixtures and retain their APIs.
New facade-owned Engine/Owner/MemoryStore/merge-plan/context state uses opaque
redacted Debug; do not derive automatic formatting of domain rows or keys.

No schema/query/merge/queue algorithm moves out of core. Known adapter owner
futures must return Ready on their first poll; test every dispatched operation
and forbid custom arbitrary engines in this synchronous facade. OPFS/VFS async
initialization finishes before owner admission. UnexpectedPending is a static
internal capability failure; never silently run a partial future twice or invent
an executor/rollback guarantee. Third-party asynchronous adapters require another
reviewed bridge capability, not implicit blocking/polling.

Preserve API-0008's1,048,576 UTF8-byte wire frame maximum, depth64 and100000 wire
value nodes, exact keys and no raw JSON numeric tokens. Core decoded business
Json also keeps API-0006's own1MiB/depth64/100000-node limits. Tagged representation
and structural envelopes consume wire budget, so **not every theoretical maximal
core Json fits this binding**. Over-limit input fails before mutation; document
and test this tighter boundary. No speculative bigger parser/streaming protocol
or global limit increase. Arbitrary valid *schemas and concepts* are supported
within these explicit bounds; schema names/tables are not fixed.

Preserve existing response budgeting: mutators preflight their complete success
report before core mutation/finish/commit, including IDs/correlation fields.
No successful commit becomes LimitExceeded solely while serializing its reply.
For entity reply preflight, add readonly core helpers
`normalize_entity_fields(table,fields)` and `normalize_entity_target(table,id)`.
They delegate to the existing private reference normalizer only when that table
is configured for entity sync; unconfigured tables retain the supplied values.
Managed links pass their declared through table, while command scopes retain
their existing command reference semantics. These helpers expose only the
caller's normalized application values, never ledger/mapping/undo internals,
and acquire no permit or mutate state. Preflight sizes the normalized full row;
the actual mutation continues through the same core normalizer and transaction.
Read-only large fetch may return LimitExceeded, never truncated rows; results
larger than one frame use explicit limit and keyset predicates on a unique ordered
field such as id, with no implicit paging or large-dataset transfer guarantee.
Drain/push use owned bounded prefixes and `has_more` exactly as API-0008, except
SPEC-0036 drain termination: an event tested as the first item of a page whose
sole-item frame exceeds a wire limit terminates only its subscription. That frame's
has_more is true exactly when another buffered event remains after removing the
subscription's later events. The event and those later events are removed, the
core subscription ends and `{kind:"error",subscription,value:{kind:"LimitExceeded"}}`
takes that position; other events keep order and the drain succeeds. No truncated
or substituted snapshot is delivered; a terminated handle never delivers again and
its unsubscribe returns false. This is a documented binding limit, not a promise
of unlimited row/event transfer. No shadow durable queue.

## 2. ABI entry points, key channel and host lifetimes

ABI major1, locally packaged initial SDK version0.1.0. Proposed C header:

```c
typedef void (*hoya_v1_reply)(void *context, const uint8_t *bytes, size_t length);
uint32_t hoya_v1_abi_version(void); /* returns1 */
int32_t hoya_v1_open(const uint8_t *json, size_t length,
    const uint8_t *key, size_t key_length, hoya_v1_reply reply, void *context);
int32_t hoya_v1_call(const uint8_t *json, size_t length,
    hoya_v1_reply reply, void *context);
int32_t hoya_v1_validate_open(const uint8_t *json, size_t length,
    size_t key_length, hoya_v1_reply reply, void *context);
```

Status0 means one complete JSON reply delivered;1 invalid span/UTF8/callback input;
2 contained Rust panic/poison/internal boundary failure. No raw panic/error text.
Caller safety precondition: every nonempty input span points to valid readable
memory for its full length throughout this synchronous call, and reply callback
/context remain valid. The foreign reply callback must not throw or unwind across
C ABI. Native package panic containment requires panic=unwind; process aborts and
foreign exceptions are outside catch_unwind. The boundary can check null/length/UTF8 before effects;
it cannot validate arbitrary address accessibility or contain SIGSEGV/foreign
memory faults with catch_unwind. Check excessive lengths before any pointer
dereference. `(NULL,0)` is allowed for the memory key only. All input/reply spans are borrowed for this call/callback
only; host copies response before returning. No host application handler/event
callback runs inside this collector. No registry mutex/Rust borrow spans reply;
contain panic, never unwind native/JNI/Swift. IDs are nonzero monotonic u64 decimal
handles, not pointers, with checked exhaustion and owner/type validation. Use one monotonic allocator for all facade handle
kinds in its runtime, never recycled slots that let an owner token alias a
memory/scope token. Transport realm and local epoch prevent stale callbacks
from a replaced worker/runtime being routed into its replacement.

Operator clarification (2026-10-29): after a default browser worker retires,
a later default open starts a fresh worker/runtime. Concurrent replacement opens
share that new attempt. Existing database/runtime handles stay closed; no old
request, database owner or callback is migrated or replayed into the replacement.
Explicit caller-supplied workers do not replace or clear the default runtime.


`hoya_v1_validate_open` is an additive ABI1 readonly capability over the existing
`Facade::validate_open`: same span/UTF8/frame/collector and panic-containment
rules, no key pointer or key copy. Lengths other than0/32 reject with boundary
status1 before dereference, matching open's binary-length boundary. An admitted
engine/key mismatch returns the ordinary static InvalidRequest reply. A valid
frame returns Null without allocating an owner/token, initializing the process
registry, acquiring its mutex, or invoking provider/filesystem/storage policy.
Use a fresh empty safe Facade only for this pure parser. JNI adds package-private
`NativeBridge.validateOpen(byte[] input,int keyLength):byte[]`; negative or
non0/32 lengths reject statically, all arrays are owned and JNIEnv stays local.
Existing native open/call signatures and ABI major1 remain unchanged. The exact
Android export allowlist gains only these two C/JNI symbols (eight total).

Open only uses hoya_v1_open: JSON has no key. Protected key length exactly32;
memory requires0 and touches no path/provider. Validate key size before any
filesystem or backup-policy action. All other calls use hoya_v1_call,
which rejects open. Memory is explicitly key-free and nonpersistent. No production unprotected
persistent open operation; existing explicit
unprotected spikes remain historical controls. Native owned key copies use the
approved zeroize library and never derive Clone/serialize/Debug accessors.

Browser worker open message has exact `{kind:"open",input:String,key:ArrayBuffer}`
with a caller-copy transferred once; memory key is empty ArrayBuffer. Ordinary
message `{kind:"call",input:String}` has no key property. The dedicated worker
initializes real protected OPFS/VFS before calling the facade. JS never puts keys
in normal JSON frames, diagnostics, transport or captured context. JNI exposes
one native open(byte[],byte[]) and one call(byte[]) using byte arrays, not modified
UTF8 strings; local refs and JNIEnv stay thread-local. Swift/Kotlin wrappers copy
responses and schedule app callbacks on a documented selected executor.

Before OPFS initialization, the worker invokes the internal WASM capability
`validateOpen(input:String,keyLength:U32)->String`. It returns the ordinary ABI1
Null/error reply using the same exact original-frame parser, schema/migration
representation decoders, namespace/options widths and engine/key-shape checks as
open. It never allocates an owner, touches storage or evaluates a path/provider.
This is an implementation preflight capability, not an additional wire operation
or a promise that subsequent storage open succeeds. Do not wrap the original
input in a json_decode frame: the extra quoting/tagging would impose a smaller
request budget. Invalid ABI, duplicate keys and unknown enum/key variants reject
before VFS initialization. Open repeats validation at its actual admission.

Wrapper lifecycle is Closed→Opening→Open→Closing→Closed, with a monotonically
advanced local epoch. Opening synchronously reserves the client before await;
second open rejects Busy. Failure releases only its own reservation. close during
pending open invalidates publication, awaits late reply, closes any acquired
owner then settles; no fabricated close before handle exists, no leaked late
handle or stale Open state. A new open cannot race that disposal. Reset/close
invalidate local epoch, stale callbacks/network responses, merge/input caches and
subscriptions; late results cannot publish into a replacement owner. Opening
state contains no diagnostic/Debug key bytes. Keep the actual Phase 6 pending-open
regressions for protected and explicit legacy reference clients unchanged.

No Rust borrow, registry lock or transient native-call/idle permit crosses a host
async handler, transport, custom merge hook or UI event callback. An admitted
command's staged scope retains its owned execution admission permit across
handler awaits until finish or abort, as API-0006 requires. Host runs per-database
FIFO command jobs over core request/begin/finish, using scoped handles across
awaits (ADR-0037, SPEC-0037). JavaScript scopes its queue and pump to each Database
instance; actual handler execution still requires the real core admission. The
conservative global core remains FIFO among allocated tickets; host invocation
order across instances is not promised. A background sync admission Busy waits
through the existing release mechanism without reporting a sync failure. The
tracked scope exposes no request/execute method, exactly API-0006; independently
captured outer execute calls queue FIFO. Awaiting such an outer queued child
inside its admitted parent is unsupported application misuse, not a promise of
reentrant execution or automatic async-context detection. Cancellation before
finish aborts; once finish is submitted it may
commit and its durable result is preserved. Close cancels queued/admitted jobs,
releases scopes and prevents stale delivery; it does not retract backend writes.

## 3. Exact common grammar

Below, `{a:T,b:U}` lists **all required keys**; unknown/missing/duplicate keys
reject. Maps are plain objects keyed by application names; duplicate map keys
reject before construction. Arrays preserve order. UTF8 is strict; strings are
not normalized. U is canonical unsigned decimal String (`0` or nonzero leading
digit followed by digits), checked to destination u32/u64/usize and at most signed
i64 where core storage uses i64. I is canonical signed i64 decimal String; `-0`,
leading plus/zeros, overflow and whitespace reject. Handles are U>0. U16, U32 and U64 mean U checked to the named unsigned width.
Captured command/entity clocks and incoming comparison clocks follow the core
UTC range 0..253402300799999. Row Timestamp cells retain the full signed i64
range. prepare_push uses signed I with its existing core validation; unsigned
monotonic drain time is U64 and has no UTC upper bound. Boolean is real JSON Boolean.
Nullable means explicit JSON null, never omission except union-variant keys.
Portable collection sizes, limits and indices are bounded to u32 on every
platform before conversion to usize; handle identity still uses full u64. This
avoids native-versus-WASM pointer-width differences in accepted requests.

Every ordinary request is `{abi:"1",op:String,owner:Handle|null,args:Object}`.
Open, memory_store/memory_drop and the two pure json_* codec helpers below use
owner:null; every owner-stateful
ordinary operation must own a live owner. Reply is
`{abi:"1",ok:true,value:T}` or `{abi:"1",ok:false,error:{kind:ErrorKind}}`.
Static ErrorKind includes all nine StorageError variants, CommandError variants,
and boundary InvalidRequest/InvalidHandle/InvalidPayload/LimitExceeded/
UnexpectedPending/UnsupportedOperation/Closed. No message/table/id/path/SQL/identity fields in errors.
Unknown ABI major/op/enum keys reject before effects. Data APIs intentionally
return application data; safe diagnostics remain the ten API-0010 scalar fields.

### 3.1 Typed cells and business JSON

`Cell` exactly:

- `{kind:"null"}`;
- `{kind:"boolean",value:Boolean}`;
- `{kind:"string",value:String}` or `{kind:"reference",value:String}`;
- `{kind:"integer",value:I}` or `{kind:"timestamp",value:I}`;
- `{kind:"float",value:String}` parsed by the existing finite-float codec; valid
  JSON number grammar, finite f64 only, retain Float even for42.0/integral text,
  reject NaN/Infinity/overflow. Use the current core FiniteFloat normalization.

`J` for command/context/result business Json uses the same null/boolean/string/
integer/float tags plus `{kind:"array",value:[J...]}` and
`{kind:"object",value:{String:J...}}`. Reference/Timestamp are Cell-only.
No raw JS Number may silently carry an unsafe i64. Public JS accepts bigint or
canonical string for integer/timestamp, safe integral Number convenience only;
Swift/Kotlin use64-bit integers and explicit finite doubles. Decoder reconstructs
existing Json and validates its canonical business limits after wire validation.

`Fields = {String:Cell...}`. `Row = {id:String,fields:Fields}` contains public
columns only; never __hoya_schema_version. Core validates required/optional,
reserved names and references. Null is distinct from absent optional fields.
`Change = {table:String,id:String,before:Row|null,after:Row|null}`; no invented row
on a failed mutation. Any ordering of changed records remains current core order.

### 3.2 Schema and authored migrations

`Column = {kind:"String"|"Integer"|"Float"|"Boolean"|"Timestamp"|"Reference",optional:Boolean}`.
`Table = {columns:{String:Column...},relations:{String:Relation...}}`.
`Schema = {version:U32,tables:{String:Table...}}` (version>0).
Relation union:

```text
{kind:"ToOne",table:String,foreign_key:String,optional:Boolean}
{kind:"HasMany",table:String,foreign_key:String}
{kind:"ManyToMany",table:String,through:String,
 source_relation:String,target_relation:String}
```

Construct existing SchemaDefinition/TableDefinition/RelationDefinition and call
their validation/normalization. Do not infer fixed tables or write a host schema
validator. `__hoya_`,id,dotted-name and through constraints remain API-0004.

`Migration = {from_version:U32,to:Schema,steps:[MigrationStep...]}`. Exact steps:

```text
{op:"AddTable",table:String,definition:Table}
{op:"DropTable",table:String}
{op:"AddColumn",table:String,name:String,column:Column,
 default:{kind:"none"}|{kind:"some",value:Cell}}
{op:"DropColumn",table:String,name:String}
{op:"RenameColumn",table:String,from:String,to:String}
{op:"Backfill",table:String,column:String,value:Cell}
{op:"SetRelations",table:String,relations:{String:Relation...}}
```

Explicit default union distinguishes None from Some(Null). All seven existing
MigrationStep variants reach the same Rust migration implementation, including
ambiguous chains, mismatched endpoints, missing-chain reset/recovery guards and
downgrade/schema-drift rejection. SDK upgrades never invent implicit reset.

### 3.3 Queries and writes

`Query = {table:String,predicates:[Predicate...],order:Order|null,limit:U|null,
source:{table:String,id:String,relation:String}|null}`.
Predicate `{field:String,operator:"Eq"|"Ne"|"Lt"|"Le"|"Gt"|"Ge"|"In",values:[Cell...]}`;
non-In requires exactly one value; In may be empty. Order `{field:String,
direction:"Asc"|"Desc"}`. Construct existing query/related builder; table for a
navigation is its target and must agree with the declared source relation, never
silently override a contradictory caller table. Core resolves field paths,
types/null semantics/order/ties/limit lazily on fetch/observe. No host filtering.
Larger-than-frame reads use explicit limit with ascending id order and id Gt
the last returned id (SPEC-0036). This is an explicit sequence of reads, not a
snapshot cursor; concurrent writes may change membership between those reads.

`Write` exactly `{op:"insert",table:String,fields:Fields,proposed_id:String|null}`,
`{op:"replace",table:String,id:String,fields:Fields}`, or
`{op:"remove",table:String,id:String}`. Proposed-ID semantics in section5.
No upsert or caller-supplied raw internal schema version.

### 3.4 Commands, entity pages and reconciliation

`Definition = {name:String,arguments:{String:{kind:"Any"|"String"|"Integer"|
"Float"|"Boolean"|"Array"|"Object",optional:Boolean}...},reference_paths:[Path...]}`.
Path is array of `{key:String}` or `{index:U}`. Names/version suffix, unique valid
paths, argument kinds and exact context are validated by existing owner.
`Captured = {identity:J,now_ms:I}`; command identity is always an explicitly
supplied J (including boxed null if the app chooses), not missing identity.

`EntityConfig = {scope:String,tables:[String...]}` (set, duplicate table rejects).
EntityChange and PullPage **reuse PullPage::from_json/to_json, API-0009 exact
shape**, not a parallel decoder: `{table,record_id,op:"create"|"update"|"delete",
changes:Fields,removed_fields:[String...],timestamp_ms:I,
field_timestamps:{String:I...}}`; page `{checkpoint:String,complete:Boolean,
changes:[EntityChange...]}`, max1000 changes and canonical1MiB. Duplicate removals,
unknown keys, missing clocks, unsafe values and shape/type errors reject in core.

`RemoteOutcome` is `{status:"applied"|"corrected",result:J}` or
`{status:"rejected",reason:String}`; receipt-only is explicit null.
`Plan = {writes:[PlanWrite...],mappings:{String:String...}}`, writes:
`{op:"put",table:String,id:String,fields:Fields}`, `{op:"remove",table:String,id:String}`,
or `{op:"entity",change:EntityChange}`. Entity corrections require the last typed
variant/clocks; no host-invented timestamps or generic Put substitute.
Mixed transport wire remains API-0009/API-0008, correlated by idempotencyKey or
table/recordId/writeId, with separate entity-only acknowledged literal. Backend
transport bytes are ordinary canonical server JSON, not this binding's J tags.

### 3.2 Conflict configuration and resolution (SPEC-0040)

The SDK host option is optional `conflict:{default:"stop"|"discard",commands?:
{versionedCommandName:"stop"|"discard"}}`. Absence means Stop. Normalize omitted
commands to an empty map before calling the facade setter. An exact recorded
name overrides the default; unknown valid names are inert. Invalid names, values
or oversized configuration reject without replacing the previous policy. The
core idle-admission rule applies, including Busy while a handler is admitted.

`ConflictPolicy = {default:"stop"|"discard",commands:{String:"stop"|"discard"}}`
is the exact setter payload, with both keys required. It is ephemeral per owner,
not part of Options or the open frame. After receiving a new owner, SDKs install
the configuration before publishing a usable database. Failed configuration or
concurrent close disposes that new owner. Reopen starts with core Stop and then
reapplies the host configuration before publication.

`Resolution` is exactly `{kind:"completed"}` or
`{kind:"kept_current",discarded_keys:[String...],unmapped:[String...],skipped_writes:U}`.
Rollback/manual discard report newly discarded keys and created local IDs that
remain unmapped, with skipped_writes zero. Correction-discard reports empty
lists and the exact skipped-write count. The full reply is encoded and bounded
before commit, including when a unit-returning method drops only the report.
Here U is the canonical unsigned decimal String defined in §2. A newly discarded
key may already be removed by normal history trimming in the same commit;
the report describes the transition, not a guaranteed later status lookup.
All policy refusal, C4-first ordering and guard-precedence rules are SPEC-0040.

SDKs use the resolving operations internally for reconcile and rollback and
expose explicit discard. This covers typed, managed and advanced raw calls alike:
a Swift or Kotlin raw `execute` of `reconcile` (exactly `{key,plan}`, sent with
empty guards), `reconcile_guarded` or `rollback` is submitted as
`reconcile_resolving` or `rollback_resolving`, delivers the report as below and
returns Null. A malformed legacy `reconcile` frame is submitted unchanged and
keeps the core's exact-shape rejection. A current-epoch KeptCurrent result invokes optional
per-database `onResolution(key, resolution)` once. Keep that callback installed
when it fails: report each failing invocation once through onBackgroundError as
`{source:"callback",kind:"CallbackFailure",token:null}`. There is no listener
token. A thrown error or returned JavaScript thenable rejection never changes
the committed result. Contain sink failures and suppress old-epoch reports after
close/reset. Do not send these callback failures to startSync onError, which
remains a sync-failure adapter. The same rule applies to JS/web/RN, Swift and
Kotlin; it does not retire an observation listener or add a new event framework.

The Kotlin sink gap is tracked separately by DEF-0040. Source-only decoder,
model, sink and test changes may be prepared while JVM/Android execution is held,
but remain unexecuted and unverified. A callback-only correction does not close
DEF-0040 or establish parity for its other background-error sources.

## 4. Operation table — exact args and result

All listed arg keys required. Scope variants add `request:Handle`; request IDs
are facade-owned checked maps to core CommandRequest. Result Null is JSON null.
Open uses binary-key entry; normal call rejects it. Errors have no side effects
except existing explicit core retry/conflict semantics documented in API-0006/9.

| op | exact args | value / existing core action |
| --- | --- | --- |
| json_decode | `{body:String}` with owner:null | J, using core Json::parse then tagged boxing; no host JSON number parse |
| json_encode | `{value:J}` with owner:null | canonical String, using core tagged decode + Json::canonical |
| memory_store | `{}` with owner:null | retained in-process memory-store Handle, no FS or entropy |
| memory_drop | `{store:Handle}` with owner:null | Null; only closed retained store, Busy if leased |
| open | `{engine:Engine,schema:Schema,migrations:[Migration],namespace:String,options:Options}` | owner Handle after guarded CommandDatabase::open |
| close | `{}` | Null; drop owned scopes/pulls/merge/reply buffers and close actual engine |
| schema | `{}` | normalized Schema owned copy |
| protection | `{}` | `{encrypted:Boolean,cipher:String,version:String,backup:"host_managed"|"unsupported"|"not_applicable"}`; no path/key; native policy is independently host-verified below |
| request | `{}` | Handle; core request |
| cancel_request | `{request}` | Null |
| begin | `{request,definition:Definition,value:J,context:Captured}` | Boolean admitted; false retains FIFO ticket for retry |
| scope_args | `{request}` | J |
| scope_context | `{request}` | Captured; immutable host view |
| fetch / scope_fetch | `{query:Query}` / plus request | `[Row...]`; complete fetch or LimitExceeded |
| insert / scope_insert | `{table,fields,proposed_id}` / plus request | Row; core allocation or validated proposal |
| replace / scope_replace | `{table,id,fields}` / plus request | Row |
| remove / scope_remove | `{table,id}` / plus request | Boolean |
| batch / scope_batch | `{writes:[Write...]}` / plus request | `[Change...]`; one atomic batch |
| link / scope_link | `{table,id,relation,target,fields,proposed_id}` / plus request | managed join Row; reuse core through mapping |
| unlink / scope_unlink | `{table,id,relation,target}` / plus request | U removed count |
| finish | `{request,result:J}` | `{key:String,result:J}`; preflight before durable atomic finish |
| abort | `{request}` | Null; drop staged scope |
| observe | `{query,mode:"membership"|"columns"|"count",columns:[String...]}` | subscription Handle; columns empty unless columns mode |
| unsubscribe | `{subscription:Handle}` | Boolean; liveness enforced in buffered events; false for unknown or terminated |
| refresh | `{}` | Null; core explicit refresh retries |
| observation_stats | `{}` | `{groups:U,observers:U,projected_queries:U,candidate_updates:U,materialized_rows:U}` |
| drain | `{now_ms:U64}` | `{events:[Event...],has_more:Boolean}`; owned bounded prefix; an event that cannot be a sole page item terminates only its subscription (§1, SPEC-0036) |
| queue_status | `{}` | Status below; complete or LimitExceeded |
| queue_status_page | `{after:String\|null}` | Status fields with `commands` an owned prefix in the same core order after the Entry whose key is `after`, plus `has_more:Boolean`; same cursor, Protocol and LimitExceeded rules as pending_reconciliations_page; scalar counters describe the whole queue at that call |
| observe_status / observe_progress | `{}` | sync subscription Handle |
| observe_outcome | `{key:String}` | sync subscription Handle |
| unsubscribe_sync | `{subscription}` | Boolean; false for unknown or terminated |
| prepare_push | `{now_ms:I,jitter_basis_points:U16}` | `{envelopes:[String...],next_due_ms:I|null,has_more:Boolean}`; bounded prefixes of original core-owned batch, not re-dispatch |
| receive | `{key:String,outcome:RemoteOutcome|null}` | Null; durable receipt before local mapper |
| receive_entity | `{table:String,record_id:String,write_id:String,outcome:RemoteOutcome|null}` | Null; core correlation |
| receive_wire | `{body:String}` | API-0008/9 per-target result reports; preflight full report before first receive; a result whose tagged form cannot fit one pending_reconciliations frame reports LimitExceeded for that target before its receipt (§1), as do receive/receive_entity |
| request_redelivery | `{key:String,now_ms:I}` | Null; explicit Acknowledged-to-Sent rearm with identical frozen bytes (SPEC-0038) |
| reconciliation_context | `{key:String}` | `{kind:"command" or "entity",created:{id:table...}}`; readonly, Protocol for unknown/non-pending record; normal success/frame size budget, only kind and created (no args/undo/frozen/payload) |
| pending_reconciliations | `{}` | `[{key:String,outcome:RemoteOutcome}...]`; readonly/application payload; complete list or LimitExceeded |
| pending_reconciliations_page | `{after:String\|null}` | `{entries:[{key:String,outcome:RemoteOutcome}...],has_more:Boolean}`; additive readonly owned prefix of the same list in the same core order, starting after the entry whose key is `after` (null: from the first); never empty while an entry remains; Protocol when `after` is not currently pending (restart from null); LimitExceeded only when the next entry cannot be a sole page item |
| reconcile | `{key,plan:Plan}` | Null; guarded core writes/mapping |
| reconcile_guarded | `{key,plan:Plan,guards:[{table:String,id:String,expected:Fields|null}...]}` | Null; exact public before-images for cross-row effects, including expected absence (SPEC-0039) |
| set_conflict_policy | `{policy:ConflictPolicy}` | Null; validate then replace the ephemeral idle-owner policy |
| reconcile_resolving | `{key,plan:Plan,guards:[{table:String,id:String,expected:Fields|null}...]}` | Resolution; guards empty when none; same SPEC-0039 semantics |
| rollback_resolving | `{key}` | Resolution; ordinary inverse or eligible configured Discard |
| discard | `{key}` | Resolution; explicit eligible whole-closure retirement independent of policy |
| rollback / cancel / forget | `{key}` | Null; existing inverse/retention rules, with rollback using configured policy and identical report preflight; cancel ignores policy |
| entity_configure / entity_reconfigure | `{config:EntityConfig}` | Null; install one-shot native EntityContext provider |
| entity_configure_durable | `{config:EntityConfig}` | Null; SPEC-0049 explicit fresh-store durable alias retention; no conversion of configured legacy stores; an opted-in store rejects `entity_configure` with the same InvalidCommand |
| resolve_entity_id | `{table:String,id:String}` | String; readonly core ID resolution, durable beyond history only with SPEC-0049 opt-in |
| entity_batch | `{writes:[Write...],context:Captured}` | `[Change...]`; no mutation-time foreign callback |
| entity_batch_aliased | `{writes:[AliasedWrite...],context:Captured}` | `{changes:[Change...],aliases:{alias:id...}}`; SPEC-0048 explicit temporary aliases and partial patches, one atomic core batch |
| patch | `{table,id,fields,context:Captured}` | Row; partial patch uses core semantics |
| entity_link | `{table,id,relation,target,fields,context:Captured}` | Row; strict entity-created join identity |
| entity_unlink | `{table,id,relation,target,context:Captured}` | U |
| sync_progress | `{}` | Progress below |
| begin_pull | `{}` | `{request:Handle,since:String|null,scope:String,generation:I}` |
| apply_pull | `{request,page:PullPage}` | Null; ordinary no-host-hook path only |
| abort_pull | `{request}` | Null; disposes any associated passive merge session |
| merge_prepare | `{request,page:PullPage,tables:[String...]}` | MergeStep; core-owned passive path section6 |
| merge_input | `{session:Handle,step:U,part:"current"|"metadata"|"incoming"|"default"}` | corresponding owned typed value; each bounded part independently preflighted |
| merge_reply | `{session,step,row:Row|null}` | next MergeStep after shared core validator |
| merge_commit | `{session}` | Null; complete guarded atomic pull commit |
| merge_abort | `{session}` | Null; retire associated pull request/stage |
| reset | `{mode:"preserve_recovery"|"discard_recovery"}` | Null; exact API-0009 guard plus host generation/cache invalidation |
| diagnostics | `{}` | exact ten API-0010 count/Boolean fields; no new ID/timing fields |

Memory protection is exactly `{encrypted:false,cipher:"none",version:"not_applicable",
backup:"not_applicable"}`. It does not initialize or attest an at-rest cipher.
Protected native/OPFS owners return their actual SQLCipher capability, with
native backup `host_managed` and browser backup `unsupported`; a supplied Boolean
never establishes an OS backup guarantee.

`Options = {capacity:U,history:U,initial_delay_ms:I,multiplier:U32,max_delay_ms:I}`,
with exact existing SyncOptions validation/defaults. SDK wrapper supplies defaults;
reopen must match persisted options. Namespace exactly32lowercase hex supplied by
platform CSPRNG (Swift Security/Kotlin SecureRandom/browser crypto; RN native),
never a Rust getrandom crate added silently. Existing namespace persists on reopen.
The optional testing factory accepts an explicit32hex namespace and then must not
call the production entropy capability; fixed namespace is not a production default.

Engine union: `{kind:"memory",store:Handle|null}`, `{kind:"native",directory:String,name:String,
backup_excluded:Boolean}`, `{kind:"opfs",name:String}`. Memory rejects key/path
fields and bypasses FS/provider; native name is single nonempty component,
directory dedicated canonical app-owned nonsymlink root; browser name follows
existing OPFS validation. Default Android wrapper supplies a noBackupFilesDir
child (opt-in filesDir); Apple prepares/read-backs directory flag before file
creation; browser capability unsupported. Facade native capability says
host_managed, never excluded/included based on an untrusted backup_excluded
JSON Boolean. Only the thin platform wrapper may enrich its public result with
verified actual root-policy data: Apple actual set/readback or Android canonical
noBackup/filesDir child selection, not merely echoing caller input. Direct C ABI
consumers receive no unearned OS backup proof. No arbitrary source import/replacement.

SPEC-0036 fixes protected native backup policy by location. Apple wrappers decide
under the owned no-follow claim `.hoyadb-backup-policy` (O_EXLOCK|O_NONBLOCK with at
most 2000 ms of lock-acquisition retries, then Busy; that bound does not cover
filesystem I/O or the Android lock; regular, single-link, owner, at most 16 bytes, path
identity equal to the locked descriptor). A recorded `excluded`/`included`
reservation is immutable: a different request or inconsistent directory flag
rejects BackupPolicyFailure. An unreserved empty directory gets the flag, readback
and durable reservation before unlock; an unreserved legacy directory is reserved
only when its existing flag matches, else rejects. Android wrappers hold the
process-wide semaphore then a no-follow FileChannel lock on
`noBackupFilesDir/hoyadb/.hoyadb-backup-policy.lock` from the opposite-parent check
through the key-bearing open reply, and reject BackupPolicyFailure when any store
family file exists under the opposite policy parent. A class A policy-rejected
open (conflict, preflight, or claim/lock Busy, before any flag or claim write)
writes no store file, never changes a backup flag and may create only that empty
owned bookkeeping file, never deleted. A class B Apple initialization failure (flag
set/readback, ftruncate, pwrite or fsync) fails closed with BackupPolicyFailure and
no native open; it may leave the flag changed only on a directory that had no other
entry and an empty, partial or unconfirmed-complete claim, with no rollback
guarantee. A later open retries an empty claim in an otherwise empty directory;
an empty claim beside existing entries takes the legacy path, which reserves only
when the existing flag matches and never changes that flag. A partial or malformed
claim fails closed. Separately authorized offline manual recovery may truncate
that claim to empty, without deleting or moving store files or changing the flag;
hoyadb never performs that repair. A complete reservation is honored. Android has
no class B writes.
An open that passes the policy step and then fails natively keeps its reservation
and any flag it set on a previously empty Apple directory. No store is migrated,
moved, deleted or reset.

Retained memory stores are explicit opaque handles, not names or exported data.
store:null is a fresh ephemeral backend dropped on close. A memory_store handle
owns one real SnapshotStorage and may have at most one leased owner; duplicate
active open/drop returns Busy. On reopen clone the closed SnapshotStorage as a
candidate before passing it to consuming CommandDatabase::open; retain the
closed original until full schema/migration/owner open succeeds, preserving it
on failed migration. Success discards the old copy and marks the slot leased.
Add core consuming `CommandDatabase::into_storage(self)->S`, preserving `close`:
abort/drop active scope and execution-owner tickets, then extract underlying
ReactiveDatabase storage. Close parks that actual backend in the retained slot,
not an exported/simplified/shadow snapshot. All subscriptions/providers/merge
sessions are ephemeral and reconfigured on reopen; durable model/ledger data
remain in real memory storage. Explicit memory_drop releases closed state.
No locks/engine cloning or retained-original fallback is introduced for protected
persistent stores. This seam makes actual hermetic authored v1→v2 reopen possible.

Status exact `{pending:U,sending:U,awaiting_outcome:U,live:U,blocked:Boolean,
commands:[Entry...]}`. Entry exact `{kind:"command" or "entity",table:String|null,
record_id:String|null,key:String,state:"queued"|"sent"|"acknowledged"|"applied"|
"corrected"|"rejected"|"cancelled"|"discarded",blocked_on:[String...],awaiting_local:Boolean,
awaiting_key:String|null,
rolled_back:Boolean,attempts:U32,next_due_ms:I|null}`. Command table/record_id null;
entity values from core. attempts mirrors durable record even in history;
next_due_ms is Some(record.due_ms) iff state is Sent and None otherwise. No
result/reason/identity/args/frozen/undo fields. awaiting_key is the earliest
row-causal dispatch blocker computed under SPEC-0038, or null; it is not a
persisted field and does not change blocked_on local-ID dependencies.
Progress exact `{phase:"not_started"|"bootstrapping"|"ready",ready:Boolean,
pages:I,changes:I,tombstones:U}`. Event union mirrors CommandEvent plus error-safe
Data: `{kind:"data",subscription:Handle,value:{kind:"rows",rows:[Row...]}|
{kind:"count",count:U}|{kind:"error",error:{kind:ErrorKind}}}`;
`{kind:"status",subscription,value:Status}`, `{kind:"progress",subscription,value:Progress}`;
`{kind:"outcome",subscription,value:{state:"queued"|"sent"|"acknowledged"}|
{state:"applied"|"corrected",result:J}|{state:"rejected",reason:String}}`.
`{kind:"error",subscription,value:{kind:ErrorKind}}` is the SPEC-0036 terminal event
for any subscription kind; the data error variant remains a recoverable core
notification and never terminates.
Outcome payloads are explicit application API data; automatic diagnostics never
include them. Buffered events are filtered for current subscription/epoch before
each callback; cancel inside the first callback suppresses later stale callbacks.

For entity configuration the facade installs a Rust provider reading a one-shot
owned Captured value set by the entity_* or patch call; take it at most once and
clear on every success/error. Missing context fails rather than reusing a prior
identity/time. Configured direct writes use the entity entry points; ordinary
insert/batch/replace/remove/link/unlink against configured entity tables reject
missing context instead of silently producing unsynced writes. Unconfigured
ordinary tables retain CRUD semantics. Scope writes use command context and are
never also enqueued as entity diffs. Empty/same-value semantics remain API-0009.

## 5. Proposed-ID core seam and deterministic host helper

Retain existing Rust generator APIs unchanged. Proposed additive core helper is
`apply_batch_proposed(writes:Vec<(Write,Option<String>)>)`, preserving the existing
Write enum and every existing constructor, with ordinary and scoped
owner forwarding methods and a managed-link proposed-ID counterpart. Internal
batch lowering accepts an optional proposal **only** for an insertion; refactor
the existing allocator/validator rather than duplicate it. No proposal keeps
the exact old generator/counter path. A proposal bypasses generating another ID
but not empty/duplicate/schema/row/journal/atomic validation. Rejected operations
leave data/model counters/staged journal unchanged; application generator calls
already made outside core are not promised to rewind, matching current FnMut
generator custody semantics. Abort/drop preserves the owned generator for future
normal insertions.

Wrapper idGenerator is called before entering the native request for each explicit
insert or possible link creation. The produced String is sent as proposed_id;
no binding-owned ID counter. Empty, missing/throwing/exhausted generator values
reject, no silent random/default replacement. Batch samples all proposals before
its one atomic call; duplicate proposals reject in core. A redundant existing
link may discard an unused proposal; no promise about external callback count.
Configured automatic entity/managed-through entity creation rejects nonnull
proposals and retains its namespace/counter entity-ID allocator. A command's
surviving creations must still be `local:` IDs at finish; this seam cannot bypass
recovery-prefix rules. ADR-0035 explicitly supersedes TC-0016's historical
unprefixed command example: the command test injects and returns exactly
`local:test-fixed-id-001`, while ordinary unconfigured CRUD injects and returns
exactly `test-fixed-id-001`. TC-0016 preserves the historical wording and records
that approved supersession; the ID-policy decision is resolved.

## 6. Proposed passive per-table host merge seam

This section adds public **owned core planning**, not a host resolver or function
pointer trampoline. Existing synchronous Rust MergeHook and ordinary apply_pull
remain usable unchanged. Extract the existing custom-result validation block in
entity.rs into one internal shared validator, bracketed by existing tests.

Core planning methods corresponding to merge_prepare/reply/commit/abort own one
MergeSession per pull ticket. Under a short idle permit, prepare validates the
original pull request/page and requested table set, captures whole model state,
normalized schema identity, complete ledger expected value, generation/revision/
checkpoint and hook-table snapshot, and stages the same sequential page plan
that apply_pull uses. Release permit/borrow before returning. Never expose raw
private ledger/undo/key/SQL in a merge input; explicit inputs are application row,
comparison metadata, incoming change and existing core default result.
A requested foreign-hook table absent from the normalized schema or already
registered with a synchronous Rust hook returns InvalidCommand before staging;
there is no implicit callback precedence.

`MergeStep = {session:Handle,step:U,kind:"offer",table:String,record_id:String}` or
`{session:Handle,step:U,kind:"ready"}`. Step is checked monotonic index. Only tables
with a registered host hook produce offers; non-hook rows use the same default
resolver in core. Schema context comes from the owner's normalized schema cached
at open; current/default/incoming/metadata are separately owned read-only parts,
avoiding an overlarge combined frame. Metadata exact `{fields:{String:{
timestamp_ms:I,authoritative:Boolean}...},tombstone:{timestamp_ms:I,
authoritative:Boolean}|null}`. Default exact `{row:Row|null,metadata:Metadata}`.
Wire-budget failure aborts planning before any durable effect or callback.

Host hook receives immutable owned copies; may await and read application state,
but no core borrow/lock/global permit spans it. Its return is only Row|null.
Core supplies unchanged default metadata and runs the exact existing validator:
same existence/id, valid schema/bounds, no changed clocks/authority/tombstone,
no unrelated/tied/removed field edits. Existing default AuthorityRequired,
tombstone/deletion and protected-live-row checks run before offering; a hook
cannot bypass them. Later same-row page changes resolve against previous valid
staged proposals, not a parallel original snapshot. Explicit authoritative entity
correction keeps the API-0009 no-hook path; no invented correction clock.

Before resume and commit acquire a short idle permit and compare **all captured
model/schema/ledger/comparison state**, not only a page token. Any intervening
local write, queue receipt/reconciliation/cancel, reset, reconfiguration or schema
change makes the session stale: retire it and its pull ticket, return StalePull,
discard page bytes/proposals and fetch a new page from unchanged checkpoint.
Do not relabel old bytes with a new revision. Busy retains the exact session and
proposal for retry. Commit rechecks the expected state and existing guarded
SyncStorage plan atomically before any data/checkpoint/progress/event publication.
Failure publishes no staged data/terminal events; ordinary persistent-fault retry
uses original valid stage or fails stale after a real state change.

Wrong owner/step/reused/retired handles reject. Invalid hook return, thrown hook,
cancel, abort_pull, close/drop and reset dispose the complete stage; no leaked
permit or retained input. Hook-map changes apply to future sessions; an active
session uses its frozen host-handler references. Caller can retry an unchanged
invalid-return step only after explicit fresh prepare; never partially commit.
Test the new staged planner against ordinary core apply_pull and synchronous
Rust-hook result on the same pages, including repeated record IDs, delete/ties,
protected hidden no-ops, bad hooks and injected physical commit failure.

## 7. SDK surface packages testing and scheduling

Proposed local unpublished packages: `@hoyadb/client`0.1.0 (shared JS/TS + browser
entry), `@hoyadb/testing`0.1.0 (optional real-memory/fake helpers),
`@hoyadb/react-native`0.1.0 (thin RN transport peer adapter), SwiftPM product
`HoyaDB`0.1.0 with local XCFramework, Android Maven coordinate
`dev.hoyadb:hoyadb-android:0.1.0` in a task-local repository. Names are proposals
requiring root review, not availability/ownership/publication claims.

Each exposes openDatabase/Schema/Query/observe/command/queue/entity/pull/reset
concepts with native naming: Promise/callbacks, Swift async/throws and Kotlin
suspend plus explicit cancellation handle. No mandatory kotlinx-coroutines/Flow;
stdlib continuation alone is not advertised as Job cancellation integration.
JS owned-copy/UTF8 helpers accept admitted plain values and do not assume
structuredClone/TextEncoder/TextDecoder/WebCrypto/BigInt exists in Hermes;
portable bounded plain-value copy, Unicode UTF8 validation/byte counting and
canonical-decimal lexical i64 range checks avoid those requirements. Optional
bigint input is supported only if available, never required for string integer
input. Factory adapter injects a checked16-byte namespace CSPRNG capability;
browser uses crypto and native Swift/Kotlin/RN native platform entropy. Native owner
calls execute on a serial non-UI executor; app event executor explicitly selected.
That event executor must run submitted work serially in FIFO order for callbacks to
observe event order and for a SPEC-0036 terminal delivery to be last (Swift's default
DispatchQueue.main is serial); a concurrent executor may reorder callbacks and, after
a terminal delivery, suppress that listener's earlier pending deliveries.
Kotlin SDK suspend calls hand owned reply results to the managed executor before
resuming a plain continuation; an actual caller interceptor is honored. Arbitrary
continuation bodies must not occupy the shared serialized byte-reply channel.
This does not change the advanced raw transport callback's executor contract.

Native promise rejections carry React Native `code`; the RN transport translates
open, call and namespace rejections into fresh redacted errors with `kind` and an
identical static message. For open preserve only InvalidRequest, BackupPolicyFailure,
Busy and Closed; for call only InvalidRequest and Closed; for namespace only Closed.
BoundaryFailure, unknown, malformed, unreadable and operation-inappropriate codes
become BackendFailure. Host call Busy must not trigger core admission release waits.
Native Closed previously surfaced as BackendFailure and now remains Closed, including
background reports before the JavaScript owner learns of native invalidation. Never
copy native exception details. The shared JavaScript public-kind allowlist includes
BackupPolicyFailure; successful wire replies are unchanged (SPEC-0036 BK10).

The RN protected-open factory uses a restricted host profile:
`engine:{kind:"native",directory:"@hoyadb/default",name,backup_excluded}`.
The marker is not an import path or a new core engine. Native TurboModule open
first invokes the readonly capability on the **original exact bytes**, before
host JSON parsing or directory policy, preserving duplicate-key/raw-number/schema
validation. Then validate the marker, one-component name and actual32 binary key
bytes before calling Android directory getters or Apple filesystem/backup APIs.
Android `getNoBackupFilesDir`/`getFilesDir` may create their OS-managed parent;
they are not readonly queries. Select only that app parent's controlled child,
or Apple's controlled Application Support child. Reject arbitrary native roots
and other host engines; memory with key0 bypasses all directory/backup policy.
After determining the canonical child path, replace only the directory marker
in the accepted structural frame and repeat pure preflight **before child
creation or backup changes**, so path expansion cannot bypass wire bounds.
Create/revalidate the nonsymlink controlled child, apply/read back Apple backup
policy or verify Android selected-parent identity, then perform one key-bearing
C ABI open. The key is never serialized or sent through a second open channel.
Protection retains exactly the four declared fields and host_managed capability.
Low-level direct C ABI caller-root opens keep their existing contracts; Swift and
Kotlin SDK protected opens and this RN child use the same SPEC-0036 claim (Apple)
or BackupPolicyLock (Android). Apple Swift and RN iOS share the claim and error
algorithm, not the open lifecycle. The RN host runs the open's policy phase on a
per-module serial policy queue: key copy, the two read-only `hoya_v1_validate_open`
preflights (fresh facade, no shared runtime), child and claim. The key-bearing open,
every other C ABI call and the owner registry stay on the module serial queue, so a
contended claim does not delay other module calls. The final module-queue invalidation
check is the admission boundary. Invalidation before it rejects Closed with no
key-bearing open. After it, the open may complete, and the existing post-reply check
closes the late owner and rejects Closed. A JavaScript close does not cancel. The owned
prepared key copy is zeroized on every exit path and never encoded. The 2000 ms bound
covers one active claim's lock wait only. A queued open also waits for earlier opens'
whole policy phases, whose I/O is unbounded (DEF-0035 residual, not waived). The RN iOS claim is the separate
link unit `ios/HoyaBackupPolicy.mm`. The podspec `ios/**` glob includes it; any direct
compile of `HoyaNativeHost.mm` must link it too.

Native managed wrappers sharing one process/core also share one payload-free
admission-release broker. Android Kotlin and RN use the same serialized async
byte executor; Apple Swift and RN use the same post-C-ABI bus. Notify after a
finish, abort, cancel_request, close or reset attempt has returned, including an
error that may have released admission, only after the collector/native guard
has returned and outside the registry lock. Do not notify on every call or on
begin(false). Notifications only wake retry of the original core operation;
they confer no admission, add no host admission queue and perform no polling.
Native RN Codegen exposes `readonly onAdmissionReleased:
CodegenTypes.EventEmitter<void>` with no arguments, IDs, operation or data.
Low-level C/wire callers must cooperate with this broker when sharing managed
owners; direct calls must not bypass the managed admission/delivery lifecycle.

The shared JS `Channel` has optional synchronous
`subscribeRelease(listener: () => void): () => void`. Its returned unsubscribe
is synchronous and idempotent. Browser-only channels may omit it; native RN
factories require it for mixed-wrapper admission. Register before allocating a
command request and before the first Busy-capable owned-response retry; retain
the listener across that admission/retry lifetime. Registration failure or a
malformed unsubscribe rejects statically before a new request/ticket is created;
already-owned stages still use their existing finally cleanup. Capture the
local release epoch after registration and before each attempt, so a release
while begin's response is suspended cannot be lost. The callback only wakes the
existing local retry mechanism while its active flag, owner and generation
match. Unsubscribe when admission/operation settles and on cancellation, close
or reset, including all failure paths. Owned-response waits race the captured
owner-invalidation promise, so a foreign scope held indefinitely cannot retain
a listener after local disposal. Ignore late queued notifications from an
unsubscribed or replaced owner. Preserve the exact staged response/proposal;
never repeat transport or application mapping merely because admission is Busy.

Host sync scheduler samples jitter uniformly0..2500 basis points (injected in
tests), calls existing prepare_push, respects next_due_ms, wakes on finish/
receipt/reconcile/resume and never sends empty arrays. Core retains retries,
attempts/deadlines and FIFO admission within the ADR-0037 / SPEC-0037
coordinator scope (exclusive native database, otherwise conservative global). Transport owns timeout/cancel/auth/TLS;
library owns no network. Busy retains response/plan without repeating transport
or app mapper; StalePull refetches. In a managed sync pass each failed non-Busy
`receive_wire` receipt is reported once as a sync failure and the same pass
continues to reconciliation and any further page; only a Busy receipt retries the
owned response. Swift and Kotlin `pullOnce` wait for a Busy `begin_pull` through
the same release mechanism as the later owned pull stages. Host suspension delays work and does not
promise unprovided OS background services.

Managed SDK observation helpers register the callback before scheduling one
drain on the selected host executor/next turn, so the initial snapshot requires
no application write or manual drain. Committed mutations also schedule drains.
When an SDK owner exposes advanced raw calls, raw `execute("drain")` returns
Busy while a managed listener exists, registration is pending or a managed drain
is in flight. Raw observe/drain remains a manual mode when no managed stream is
owned. Swift's public `drainNotifications()` uses the same managed prefix,
liveness and epoch dispatcher and invokes selected callbacks rather than
consuming events invisibly. Concurrent or reentrant explicit managed drains
return Busy; automatic mutation signals coalesce one follow-up after the current
owned snapshot completes. Direct low-level runtime/wire users own their stream
and must not consume a managed owner's events behind its SDK.
After a write, retain at most one outstanding16ms host timer to drain any core
count deadline; further writes do not keep postponing that timer. The core alone
decides whether a count is due, and no callback/latency guarantee survives host
suspension. Injectable clocks/timers keep testing deterministic. Explicit drain
remains available; subscriptions/epoch are checked at delivery and close/reset
cancel pending host drains/timers. No polling loop or second throttle is added.
JavaScript uses a checked monotonic `performance.now` capability when available;
otherwise managed observation requires an injected uptime capability (the RN
adapter supplies native uptime). Never substitute the captured UTC clock for
count uptime. Missing clock/timer capabilities reject before registration and
do not prevent ordinary memory CRUD or explicit low-level drain.

SPEC-0036 delivery. Every managed decoder handles the generic error event before
kind-specific decoding, delivers it once to its own listener (JS Event
`{kind:'error'}`; Swift `.failure(.subscriptionTerminated(kind))`; Kotlin
`ObservedValue.Terminated(kind)` or `onTerminated(kind)`) and retires that listener
without another core call; recoverable data errors keep their listener. JS explicit
`drain()` and Swift/Kotlin `drainNotifications()` follow the same rules; raw
`execute("drain")` is pass-through only when the existing managed-stream
exclusion permits it; its Busy guard is unchanged. JavaScript invokes callbacks synchronously in
event order without awaiting returned values; callbacks are not serialized with
each other's asynchronous continuations, and a pending Promise never delays later
callbacks, drains or owners. A thrown exception or later rejection of a returned
thenable retires that listener once.

Each SDK accepts an optional per-database `onBackgroundError` receiving only
`{source:"observation"|"callback"|"sync"|"merge"|"timer",kind,token}`; never an
exception, message, command key, payload, row, domain value or key bytes. Facts
reported exactly once per owner epoch: terminal event (observation, token);
callback failure (callback, CallbackFailure, token, once per listener); failed core
unsubscribe after a callback retirement (observation, token); failed managed drain
request (observation, null; coalesced callers share one); failed automatic sync,
merge/pull or timer attempt (its source, null). A throwing terminal callback yields
both terminal and callback reports. Facts after close/reset of their epoch, including
late Promise rejections, are suppressed; failures inside awaited caller APIs are
returned, not reported; sink failures are contained. Swift/Kotlin `startSync`
onError adapters still receive sync failures; the undeclared JS `syncErrors` and
`callbackErrors` arrays are bounded to the latest 64. Public types: JS
`BackgroundSource`, `BackgroundError`, `HostOptions.onBackgroundError`; Swift
`HoyaError.subscriptionTerminated(String)`, `BackgroundError`, `Database.init(...,
onBackgroundError:)`; Kotlin `ObservedValue.Terminated`, `BackgroundError`, trailing
`onBackgroundError` and `onTerminated` parameters and `BackupPolicyLock` for hoyadb
wrappers only (SPEC-0036).

Testing entry point exposes memory open, fixed/manual clock, injected IDs/jitter
and fake transport. Fake handlers receive losslessly tagged production envelopes
via the owner-independent core json_decode helper, and return tagged result Json;
the helper attaches key or entity table/recordId/writeId and uses json_encode to
form canonical wire replies. Never JSON.parse opaque envelope numbers in JS or
roundtrip i64 through Number. These pure helpers retain stated1MiB wire overhead
limits; they do not own a database or implement schema/sync. Fake can reject/
delay/drop ACK; no array-position correlation or alternate database/resolver.
Memory factory requires an explicitly preloaded core runtime and performs no
implicit WASM worker asset fetch/provider initialization; bootstrap/import is a
separate named operation completed before the hermetic open measurement window.
No storage path/key/OPFS/filesystem/network operation is invoked by memory open.
Original TC-0016/17 scenarios
remain individually mapped, including ADR-0035's explicit ID-example supersession.

Consumer installation gates use actual packed npm tarballs/declarations/assets,
local SwiftPM/XCFramework and local AAR/RN autolink+Codegen from projects outside
the source tree. No relative spikes paths. Version/API/header/export snapshots,
upgrade fixture/no implicit reset, license/source/dependency manifest, examples
and rollback docs precede release readiness. Actual supported runtime rows require
behavior/security/persistence/async/callback tests, not just imports or compilation.

The browser security harness may build an explicit default-off `test-controls`
WASM feature to inspect, replace and remove only closed logical files whose name
starts `phase-seven-sdk-owned-` and contains no separator/NUL. These controls use
the installed pool's supported transfer/remove API, static errors and a64MiB
binary transfer cap; they do not edit physical pool metadata or clear the shared
`hoyadb-v1` directory. Pool capacity/assigned counts are readonly harness metrics.
The ordinary package build excludes the feature and verifies the generated
exports contain no test control. Record this instrumentation difference and
repeat package behavior/security checks on the ordinary artifact; the feature
never adds a wire SDK operation or a user-store import facility.

Android provider uses only reviewed sources/OS entropy and target-local linking
as RFD-0006; its exact build config/entropy shim, cipher/profile/canary/process and
16KiB native checks require security review before activation. Complete new RN/
Kotlin/native build graphs still require exact CHG4 approval. Swift/Kotlin/RN
target/tool availability is not support evidence. Remote publication/push/tag/
merge remains separately authorized.

Guarded reconciliation (SPEC-0039): JavaScript `CorrectionGuard
{table,id,expected:Fields|null}` and `reconcileGuarded(key,plan,guards)`; Swift
`CorrectionGuard` and `reconcileGuarded(_:plan:guards:) async throws`; Kotlin
`CorrectionGuard` and `suspend fun Database.reconcileGuarded(key, plan, guards)`.
Each sends one `reconcile_resolving` carrying the guards (§3.2); the core
`reconcile_guarded` operation is that same resolving path with its report
dropped, so policy and guard precedence are identical. The helper returns no
value; a KeptCurrent report is delivered through `onResolution`. The plan is
encoded exactly as for `reconcile`, and `expected` uses the existing lossless cell codec, keeping an
omitted field distinct from an explicit Null. The helper has the same
observation and sync-driver wake effects as `reconcile`. It returns core errors
unchanged, with no retry or fallback. The managed reconcile callback type is
unchanged and stays unguarded. A core RollbackConflict is reported once per key
per managed pass and leaves that entry pending while the pass continues to later
entries. It causes no retry, wake or timer by itself; another scheduled pass may
try the callback again. Callback failures and other core errors keep their
existing behavior. A successful reconciliation retains its existing wake
behavior: JS, Swift and Kotlin schedule another pass. Thus a conflicting X
followed by one successful unrelated Y may try X once more, then goes idle if
that pass makes no progress. The skip itself creates no extra pass in any SDK. Row-causal
dispatch continues withholding work blocked by the unresolved entry.

## 8. First reviewed implementation slice

Permit only after root has committed reviewed preparation: general facade engine
forwarding, schema/Column/Relation/all-seven-MigrationStep codecs, Cell/J/Query/
ordinary Write codecs and pure json_* helpers; memory/protected binary-key opens;
schema/CRUD/query/relations/observation dispatch with existing default IDs and
bounded response preflight/prefix rules. Node translation/reservation tests plus
real Rust facade memory/protected tests and actual preloaded WASM memory behavior
are required. Use two caller schemas, not a mocked DB or fixture promotion.

This slice adds no external runtime dependency and may proceed while exact RN/
Maven/Pods graph approval is pending. Proposed-ID helpers and passive merge APIs
must have their own compiling REDs after this contract is reviewed; unimplemented
later operations return a static explicit unsupported error in the intermediate
development artifact, never counted as complete SDK support. Existing suites
bracket extraction; root reports intermediate scope honestly.


## Additive migration integration capabilities

[SPEC-0048](../spec/SPEC-0048-entity-batch-aliases.md) defines
`entityBatchAliased(writes)` on the portable JavaScript host. Its insert wire shape
adds `alias:String|null` and `references:{field:earlierAlias}`; replace adds the
same references map. The new patch variant is replace-shaped with `op:"patch"`
and only the supplied fields, merged against the current staged row. Remove is
unchanged. The host supplies omitted alias/reference defaults. No arbitrary ID
proposal is admitted for configured entity inserts. Only real core IDs are
persisted; references never reinterpret ordinary strings. The complete reply is
budgeted before mutation, including aliases and preserved patch fields. Existing
`entity_batch` and its write parser stay unchanged.

[SPEC-0049](../spec/SPEC-0049-durable-entity-aliases.md) defines explicit
`configureEntitiesDurable(scope,tables)` and `resolveEntityId(table,id)` on both
the portable host and application API. These methods delegate to the shared core
and retain existing static errors. Ordinary entity configuration/default formats
remain unchanged. Resolution does not rewrite query expressions or install a
JavaScript mapping ledger. Native transport dispatch uses the existing ABI call;
no C symbol or Swift/Kotlin convenience API is added.

Implementation: [core batches](../../../crates/hoyadb-core/src/command/entity_batch.rs),
[core owner](../../../crates/hoyadb-core/src/command/entity_owner.rs),
[facade](../../../crates/hoyadb-bindings/src/entities.rs),
[reply budgeting](../../../crates/hoyadb-bindings/src/budget.rs), and
[portable host](../../../packages/client/src/client.js).
