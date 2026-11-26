---
id: API-0008
title: Command host spike over one shared Rust SQLite fixture
type: api
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-23
updated: 2026-10-23
parents: [ARC-0001, ARC-0002]
satisfies: [REQ-0011, REQ-0012, REQ-0015, REQ-0028, REQ-0043]
depends_on: [SPEC-0029, API-0001, API-0006, API-0007, ADR-0032, THR-0017, TC-0032]
tags: [command-sync, commands, host, browser, native, experimental]
---

# API-0008: Shared command host experiment

This is a test bridge over the real unpublished API-0006 CommandDatabase and
API-0007 SqliteStorage, compiled into both existing experimental hosts. It is
not the packaged binding SDK. Browser and native hosts must exercise the same
core owner, queue and physical storage; no shadow queue, replacement engine or
copied command algorithm may stand in for production behavior.

## Shared fixture and ownership

Add one std-only path module at `spikes/common/command_fixture.rs`. Its public
fixture surface is `CommandFixture::new()` and
`dispatch(&mut self, input: &str) -> String`. It owns checked monotonic opaque
owner, request and subscription handle registries and concrete
CommandDatabase<SqliteStorage> instances. A local `run_once` polls only this
concrete adapter's immediate-ready futures. Unexpected Pending returns a typed
fixture error; the bridge makes no promise about other asynchronous engines.

The WASM wrapper includes that module and exposes one `CommandFixture` object
with `rpc(&mut self, input: &str) -> String`. Its existing dedicated worker owns
the object, the OPFS VFS and all Rust stores. Native includes the same module in
its existing static library and calls it through the C entry point below.
Native adds the existing workspace SQLite adapter as a path dependency; this
reuses its already approved SQLite dependency and adds no external package.

Open permits multiple **distinct** owned synthetic files in one process/worker.
Reject an already-open path; independent simultaneous owners of one physical
store remain unsupported. Multiple queued requests on one owner exercise
same-store serialization. Two owners on different stores exercise app-wide
FIFO admission through API-0006's global coordinator. Different stores receive
different fresh 128-bit namespaces from their host platform entropy facility;
reopening supplies fresh entropy but reuses the persisted namespace.

Browser raw/reactive fixtures and command owners cannot be active together.
Command owners may coexist with each other in that single worker. Closing the
last owner precedes VFS shutdown. Native tests use a task-owned temporary
directory; browser tests use the existing unique task-owned OPFS namespace.
Fixture database basenames begin `phase-four-` and end `.sqlite`. Browser names
are literal basenames, with no URI/slash/NUL. Native fixture construction samples
`HOYADB_PHASE4_ROOT` once from the process-start harness environment, before any
RPC; there is no setter/RPC to replace it. It must name an existing canonical
absolute directory beneath the canonical platform temp_dir, with basename
`hoya-phase-four-<positive-decimal-harness-pid>-<32-lowercase-hex-nonce>`.
Reject missing/malformed/non-temp roots, symlink roots, noncanonical aliases and
relative/parent-directory components. Every native open path must be the exact
canonical root plus a direct-child fixture basename; reject child symlinks and
path aliases, including an alternative spelling of an already-open file.

The harness creates that unique root and passes its same canonical spelling to
independent writer/reader child processes through their startup environment.
The embedded harness pid identifies their owning parent, not the reader's own
pid. Reopen never claims ownership of an unrelated process's arbitrary directory.
Private unit fixtures may inject a root into the same validation function; the
production constructor always samples the environment. No fixture opens an
existing user store. Cleanup targets only that root's known fixture files or the
known OPFS namespace, after every owner closes.

## Lossless transport values

Every request and response is an owned UTF-8 JSON string, decoded with API-0006
Json, with a 1048576-byte total fixture RPC bound. This additional test-bridge
bound does not enlarge the independent core value/depth/node limits. The outer
wire contains only Strings, Booleans, Null, Arrays and Objects: **no numeric
JSON token**. Decimal Strings carry clocks, indices, capacities, counts and
handles. A wrapper may JSON.parse/Foundation-decode this outer format; it must
never parse opaque canonical envelope/domain text through JavaScript Number.

Domain Json values use exact tagged objects:

| Node | Exact shape |
| --- | --- |
| Null | `{ "kind": "null" }` |
| Bool | `{ "kind": "boolean", "value": true }` |
| String | `{ "kind": "string", "value": "…" }` |
| Integer | `{ "kind": "integer", "value": "-9223372036854775808" }` |
| Float | `{ "kind": "float", "value": "42.0" }` |
| Array | `{ "kind": "array", "value": [taggedNode, …] }` |
| Object | `{ "kind": "object", "value": { "key": taggedNode, … } }` |

Rust checks exact keys, decimal i64 grammar/range, finite f64 and all normal Json
bounds. Integer strings never pass through f64. The Float String is intentionally
interpreted as finite f64 even when its decimal text is integral. Typed row
cells use the same scalar tags plus `timestamp` with decimal i64 String and
`reference` with String; arrays/objects are not row cells. Rows are
`{ "id": String, "fields": { "column": taggedCell, … } }`; managed fields
are never returned as application fields.

Small host constructors accept explicit decimal String or bigint for Integer;
a JavaScript Number is accepted only when Number.isSafeInteger. An unsafe Number
rejects before RPC/handler admission. Float constructors require a finite Number
or explicit finite decimal String. No implicit recursive conversion of arbitrary
JavaScript objects or new general-purpose JSON parser is introduced. Swift uses
Int64/String for integers and copies every incoming borrowed byte span.

Transport push envelopes remain canonical Strings throughout the host. The HTTP
body is `"[" + envelopes.join(",") + "]"`, including a single envelope. It is
never JSON.parse/re-stringified in JS. Response bodies remain raw Strings until
the shared Rust fixture parses them with Json. Array members correlate by
idempotencyKey, not position, following API-0001. Unknown/malformed fields,
unsafe conversions and unknown operations reject with redacted typed errors.

## RPC messages and exact operations

Each request is exactly `{ "op": String, "owner": String|null, "args": Object }`.
Open has owner Null; all other calls require a live owner String. Each operation
below accepts exactly its listed args, with optional fields explicitly marked.
Success is `{ "ok": true, "value": … }`; failure is
`{ "ok": false, "error": { "kind": String } }`. Error kinds are the existing
CommandError variant names, existing StorageError variant names when wrapped,
plus InvalidRequest, InvalidHandle and UnexpectedPending for bridge failures.
No private payload, path, row, identity, result or panic text appears in errors.

Handles are checked positive decimal Strings, allocated without reuse or wrap.
Request handles map to actual private CommandRequest values and their owner;
subscription handles distinguish data and sync subscriptions. A cross-owner,
closed, cancelled or stale handle cannot address another owner or a later scope.
Retain retired request handles as unusable only until their owner closes;
monotonic global allocation prevents reuse after removal. Opening another owner
does not reset handle allocation. Request/response objects are copied by value.

| op | args | success value |
| --- | --- | --- |
| open | path, namespace (32 lowercase hex), options, version (1/2 String), authored (Boolean) | owner handle |
| close | empty | Null; abort active scope and cancel queued work |
| request | empty | request handle |
| cancel_request | request | Null |
| begin | request, definition, value (tagged Object args), identity (tagged value or Null for missing provider), now_ms | Boolean admitted |
| scope_args | request | copied tagged Object |
| scope_context | request | `{ identity: taggedNode, now_ms: String }` |
| scope_fetch | request, query | application row Array |
| scope_insert | request, table, fields | application row |
| scope_replace | request, table, id, fields | application row |
| scope_remove | request, table, id | Boolean |
| scope_batch | request, writes | ordered public before/after changes |
| finish | request, result (taggedNode) | `{ key: String, result: taggedNode }` |
| abort | request | Null |
| fetch | query | committed application row Array |
| direct_batch | writes | ordered public changes; Busy during an active scope |
| observe_data | query, mode (rows/count/columns), columns | subscription handle |
| observe_status | empty | subscription handle |
| observe_outcome | key | subscription handle |
| unsubscribe | subscription | Boolean |
| drain | now_ms | `{ events: [ownedEvent], has_more: Boolean }` |
| queue_status | empty | payload-free status projection |
| prepare_push | now_ms, jitter_basis_points | `{ envelopes: [String], next_due_ms: String|null, has_more: Boolean }` |
| receive | key, outcome (Null receipt or tagged outcome below) | Null |
| receive_wire | body (raw HTTP response String) | independent `{ key, ok, error? }` entries |
| pending_reconciliations | empty | keyed owned outcomes for application plan building |
| reconcile | key, plan | Null |
| rollback | key | Null |
| cancel | key | Null or RollbackConflict with durable cancellation preserved |
| forget | key | Null |

Options are exactly API-0007's capacity/history/initial_delay_ms/multiplier/
max_delay_ms fields as decimal Strings. Definition is exactly name, arguments
and reference_paths: arguments maps field names to `{kind, optional}` using
API-0006 ArgumentType names; each path part is exactly `{key:String}` or
`{index:decimalString}`. No dynamic handler code runs in Rust.

Query is exactly table, predicates, order and limit. Predicates are an Array of
`{field, operator, values}` using API-0003 operator names and tagged row cells.
Order is Null or `{field,direction}` with Asc/Desc; limit is Null or a nonnegative
decimal String. Optional navigation is represented by a separate `source` field
only when present, exactly `{table,id,relation}`; it creates the existing related
QueryBuilder before applying predicates/order/limit. No SQL text crosses RPC.

Writes are the API-0004 ordered insert/replace/remove operations encoded as
`{op,table,fields}`, `{op,table,id,fields}` or `{op,table,id}`. Row-field maps use
the tagged row-cell representation. Public changes are exactly table, id,
before and after, with Null for an absent row. A reconciliation plan is exactly
`{writes,mappings}`: writes contain `{op:"put",table,id,fields}` or a remove,
and mappings is a local-ID to authoritative-ID String Object.

An RPC outcome is exactly `{state:"applied"|"corrected",result:taggedNode}` or
`{state:"rejected",reason:String}`. HTTP outcomes keep API-0001's status and
idempotencyKey fields; receive_wire accepts only command outcomes, not entity sync
entity-diff members. It validates the response array structure before applying
members. Each known member then invokes receive independently; a typed error on
one member cannot roll back another member's durable receipt. It never invokes
the application plan builder itself. Raw response limits are checked before
any receipt mutation; malformed JSON causes no receipt mutation.

Events carry kind and subscription only for routing. Data events carry the
existing owned rows/count/error values; counts are decimal Strings. Status is
the API-0006 payload-free projection with decimal count fields and state names.
Outcome values carry only state and optional tagged result/reason, without key
or HTTP code. pending_reconciliations is an explicit application-internal read,
not a default status/diagnostic projection. Reports use synthetic fixtures and
must not log credentials or arbitrary application payloads.

## Bounded replies preserve committed truth and owned delivery

The RPC bound applies before a mutator can report an unencodable success.
Add a shared pure sizing helper at `spikes/common/command_budget.rs`, included
by the fixture. Check a complete success frame (byte, node and depth bounds)
before finish, scoped insert/replace/batch, direct_batch and receive_wire mutate.
finish sizes its actual tagged result with the fixed 66-byte key. Generated
row IDs use the default generator's maximum 25-byte width; explicit IDs use their
actual escaped representation. Batch sizing includes existing before-images and
possible earlier same-batch images through read-only committed/scoped fetches;
a conservative bound may reject a batch before mutation, but cannot report an
oversized reply only after committing it. receive_wire preflights the maximum
per-key success/error reports before applying any member. Size errors retain
staging/tickets for explicit abort and cannot commit rows/envelopes/counters.
Small constant replies need no speculative sizing. Never increase the RPC bound
or truncate a value to hide an oversized result.

prepare_push retains the actual core-produced frozen strings/deadline in an
owned ephemeral buffer and returns the largest complete prefix that fits one
success frame, plus has_more. While a snapshot remains, continuation calls use
that same buffer without invoking core preparation again or changing deadlines.
Before returning cached bytes, exclude records no longer Sent (for example a
cancelled or acknowledged member). A single envelope must fit, as pinned by an
accepted-limit record test: the core frozen-copy record budget already includes
the quoted envelope plus more metadata than the one-envelope reply frame. No
second envelope-construction/state algorithm or shadow durable queue is added.
Hosts consume all has_more prefixes at shared idle, sending each nonempty array
without a JSON Number roundtrip, then honor retry deadlines. Wakes occurring
while transport/mapper work is in flight are coalesced and replayed at idle;
a successful finish or transport completion wake must not be lost.

drain extracts one owned core event snapshot, retains it and returns bounded
prefixes of complete events. has_more means only that frozen snapshot remains;
after its final prefix, callback-generated core events wait for a later explicit
drain. An empty pending snapshot is distinct from no snapshot, including when
unsubscribe removes every remaining event. Unsubscribe removes that token's
cached events; close drops all cached values. A single event too large to fit
returns LimitExceeded without consuming it; the integrator can cancel that
registration and use a bounded query. This is an experimental message-size
limit, not a new core query/observation ceiling. Host drain loops retain one
recursive-drain guard and check owner/token liveness before every callback,
including between prefixes. No row snapshot is split or silently discarded.

Before implementation, permanent shared real-SQLite tests must reproduce:
near-bound finish reporting failure after a real commit; aggregate drain losing
complete events; and aggregate prepared pushes failing after Sent persistence.
Then prove preflight leaves rows/queue/counters/staging unchanged, cached prefixes
preserve exact event/envelope bytes and order, a single oversized event remains
until cancellation, before-image batch bounds reject before mutation, and
receive_wire report bounds reject before receipts. JS and Swift tests prove
prefix consumption, cancellation/close and callback-write deferral across pages,
multiple bounded POST arrays, retained Busy response/plan and coalesced wakes.

## Foreign handlers and disposal

The JS/Swift wrapper configures a trusted-context provider and captures its
identity plus one clock sample before begin. The provider may return opaque
Null via its tagged representation; missing provider is distinct. The wrapper
passes copied args/context into a developer async handler only after admitted
true. false means pending, with no handler invocation. A shared host dispatcher
drives pending requests FIFO when finish/abort/cancel/close releases admission;
it does not busy-poll or block the worker while waiting for a host promise.
Committed fetch/observe/drain requests remain available during handler awaits.
If a transport response or application plan returns while admission is busy,
retain that owned response/plan until the shared FIFO becomes idle. Wait on a
release signal and recheck owner generation/liveness; Busy alone must not invoke
the transport or plan builder again or discard a received result.

The handler's scope methods invoke ordinary scoped Rust operations through RPC.
Success calls finish once; a thrown error or explicit cancellation calls abort
in finally. If cancellation occurs while a finish reply is delayed, distinguish
already committed success from an abandoned ticket using the owned reply; do
not falsely report durable writes as rolled back. Disposing an active owner
aborts its staging; later completion fails a liveness/generation check and cannot
finish another request. A never-settling promise/task holds its slot until the
integrator explicitly aborts or closes. No implicit watchdog is introduced.

Host wrappers drain committed events after transitions, then invoke callbacks
outside all Rust calls/borrows/locks. Check owner generation and subscription
liveness before **every** callback, including a previously extracted batch.
Callbacks may read, unsubscribe or close. JS catches callback exceptions so
unrelated live callbacks continue; do not turn callback exceptions into failed
durable command commits. Recursive/concurrent host drains reject. Closing or
replacing an owner invalidates pending callbacks and queued host handlers.

The native C entry point is:

```c
typedef void (*hoya_command_reply)(void *context, const uint8_t *bytes, size_t length);
int32_t hoya_command_rpc(const uint8_t *request, size_t length,
    hoya_command_reply reply, void *context);
```

Pointers are call-scoped readable spans. Validate UTF-8/length and required
callback before dispatch; hostile invalid native pointers remain unsupported.
The callback receives one owned-response snapshot borrowed only until callback
return; Swift copies it immediately. Release the fixture registry mutex before
invoking this reply callback. It runs synchronously on the caller thread and
must not throw/unwind across C. Status 0 means a delivered JSON reply (which may
contain a typed RPC error); 1 means invalid pointer/length/callback/UTF-8 preconditions,
2 means panic/poison containment. Panic containment is not rollback assurance.
Application callbacks run after Swift has copied/decoded the reply, outside C.

## Fixture schema and lifecycle

Version 1 has three application tables:

| Table | Application fields and relations |
| --- | --- |
| customers | country:String; orders HasMany orders.customer_id |
| orders | label:String, total:Integer, ratio:Float, active:Boolean, placed_at:Timestamp, notes:optional String; customer optional ToOne customers via customer_id; items HasMany items.order_id |
| items | label:String, quantity:Integer; order required ToOne orders via order_id |

Version 2 adds optional String `extra` to orders. open's authored=true selects
that explicit migration; authored=false supplies no migration and therefore tests
the existing missing-chain reset only when recovery is resolved. Version 1 uses
no migration. All opens invoke the actual CommandDatabase::open global idle
permit and normalized lifecycle. Tests explicitly close an owner before a
same-file reopen; after a rejected version-2 open they reopen version 1 and
compare application/recovery state. An unsuccessful open allocates no live
fixture handle, and no disposed owner is claimed preserved. The ordinary fixture
does not expose arbitrary schema mutation or SQL execution.

Platform entropy, host timers and HTTP live only in host wrappers. A fake
transport and the opt-in task-owned loopback POST /hoyadb/commands reference
consume the same nonempty array bytes. Use browser fetch and Swift URLSession;
configure an explicit request timeout and cancellation. Loopback credentials are
synthetic and payload logging is disabled. The shared stdlib reference server is
`spikes/common/command_http.py`, bound only to 127.0.0.1 and accepting bounded
nonempty command arrays at POST /hoyadb/commands. Its synthetic result includes
an authoritative id and an echo of args, preserving i64 and Float JSON kinds.
The fixture-only X-Hoya-Fixture header selects applied (default), mixed,
partial, duplicate, conflict or unavailable responses; normal members are
returned in reverse order to exercise key correlation. mixed yields applied,
corrected and rejected examples; partial omits members, duplicate repeats equal
outcomes, conflict appends a different outcome for one key, and unavailable
returns 503 with no receipt. A stdlib test must first fail against a compiling
stub, then prove one/many arrays, exact numeric echo, reversed/missing/duplicate/
conflicting members and rejected empty/entity-diff requests. The harness serves
browser assets from the existing storage-browser directory with standard
loopback CORS; it logs neither bodies nor credentials and changes no browser
security settings. This proves local HTTP integration,
not production TLS/authentication, backend idempotency or physical power loss.

## Behavioral acceptance before implementation — TC-0032 C20 host matrix

Shared fixture tests run as real Rust tests through both existing spike crates.
Write compiling functional-stub RED before implementing RPC routing. Host tests
must then run real suspended JS/Swift handlers; Rust mocks do not satisfy them.

An additional WASM **test-only** export, `command_conformance(index: u32)`, runs
one existing shared acceptance function unchanged over the real OPFS adapter.
The worker operation is exactly `{op:"command_conformance",index:Number}` in its
existing test-control protocol, outside the lossless production RPC. Require an
integer index 0–13 and no active raw/reactive/command owner. Its owned result is
`{case:String,files_before:[String],files_after:[String]}`; a label is returned
only after assertions and fixture cleanup succeed. Indices 0–7 call the eight
public atomic sync cases from tests/conformance/sync_storage.rs in source order;
8 calls command_owner::owner_contract; 9–13 call command_sync::{dispatch,
correction_inverse,graph,zero_history,capacity_boundary}. Include these shared
source modules directly, without duplicating their assertions or engine logic.

Every fresh adapter uses a unique phase-four-conformance-*.sqlite filename in
the current owned OPFS namespace. At most the shared owner's simultaneous stores
are open; never raise the existing 24-slot SAH pool to accommodate the fixture.
After that case has closed its adapters, delete only that exact test prefix and
compare the pool's named-file inventory to its pre-case inventory. Failure,
interruption or cleanup failure is not a PASS. Run cases independently so normal
host RPC retains its existing per-call bound. The configured-500 capacity case
is an explicit long-running test with a finite 900000 ms host deadline; the other
shared cases use 60000 ms. Record its actual wall time rather than reducing the
500 boundary or weakening assertions to fit a short RPC deadline.

Runtime evidence identifies the Cargo profile and exact generated WASM/glue
hashes. An optimized --release artifact may be used with unchanged sources,
dependencies, workloads, assertions and deadlines; record its complete harness
run separately from debug runs. A debug timeout remains failed debug evidence;
a release PASS establishes behavior only for the tested release artifact.
Existing debug compile/lint/container gates remain required.

```gherkin
Feature: C20 the real host drives one portable command owner
 Scenario: H01 suspended handler reads staging while committed readers remain usable
  Given a real JS Promise or Swift continuation paused after an order insert
  When the host fetches committed orders and requests a second handler
  Then committed fetch contains no staged order and the second handler has not begun
  When the first inserts two items and finishes
  Then scoped relation reads saw the order and both items and one committed snapshot contains all three rows
  And the next same-store handler sees the complete first commit
 Scenario: H02 two distinct owners share FIFO admission and cancellation releases it
  Given three host execute requests across two distinct stores in one worker or process
  When the first suspends and a queued request is cancelled
  Then only the admitted handler has run and uncancelled handlers begin in request order
  When the active handler throws is explicitly aborted or its owner closes
  Then its staging is absent and the next live handler proceeds
  And stale finish abort scope and cross-owner tokens reject unchanged
 Scenario: H03 foreign values preserve precision and trusted context before mutation
  Given i64 extrema integral and nonintegral floats Unicode NUL optional Null and nested identity
  When they cross JS and Swift RPC and a real handler captures them
  Then rows frozen envelopes and copied context preserve their exact logical values
  And mutation of the host context copy cannot change the persisted envelope
  When an unsafe JS Integer Number invalid UTF8 duplicate keys missing context or oversized value is supplied
  Then it rejects before the handler or local mutation
 Scenario: H04 committed callbacks retain ownership and disposal semantics
  Given data status and outcome watchers with extracted owned notifications
  When a callback reads unsubscribes another watcher closes its owner or throws in JS
  Then no Rust lock or owner borrow spans that callback and later cancelled callbacks do not run
  And surviving callbacks see complete committed state and no outcome contains a wire key or HTTP code
  And recursive drains reject without replacing committed success
 Scenario: H05 restart retains queued sent received and reconciliation work
  Given task-owned native writer and reader processes or a real browser page reload
  When restart happens before and after the atomic command commit
  Then reopen has neither rows nor command before commit and both after commit
  And original keys namespaces counters frozen retry bytes and due times survive
  When receipt without outcome or a result pending local reconciliation survives restart
  Then acknowledged work is never resent and the exact result can still be reconciled
 Scenario: H06 fake and actual HTTP use the same command-only array seam
  Given offline commits then a failing fake transport and a task-owned loopback HTTP server
  When the host sends one and multiple eligible commands and receives reordered partial duplicate or conflicting results
  Then each push is a nonempty ordered array and key correlation resolves independent members
  And integer extrema and Float kinds reach Rust without a JS Number roundtrip
  And backoff governs host wakes retries reuse exact frozen bytes and acknowledged members never resend
  And guarded reconciliation rollback cancellation and lifecycle gates preserve unrelated data
```

Record runtime/build/source hashes, actual commands and bounded latency samples,
not only a test-name count. Browser evidence includes the real worker/OPFS and
reload stages; native evidence includes real Swift suspension and independent
process reopen. Keep interrupted/failed runs distinct from final PASS evidence.
Source rollback leaves recovery stores intact; old readers must reject them
rather than removing command metadata to regain compatibility.
