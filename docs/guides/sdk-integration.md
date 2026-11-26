# Local SDK integration

Platform SDK packages remain unpublished. [API-0011](../design/contracts/API-0011-general-platform-sdk.md)
defines ownership and scope boundaries. Use current package declarations and the
platform-specific acceptance commands; host checks do not establish mobile support.

Build the ordinary WASM artifact and local JavaScript packages using the
[WebAssembly instructions](../../crates/hoyadb-web/README.md) and
[JavaScript instructions](../../tools/javascript/README.md). Install the two
resulting local tarballs into a consumer. No published npm version is required.
The browser entry is `@hoyadb/client`; the portable host entry is
`@hoyadb/client/host`. Testing capabilities live in `@hoyadb/testing`.

## Run a real retained-memory store

This Node module explicitly preloads the bundled WASM before creating the
runtime. Both opens use the core's memory adapter. The retained handle preserves
committed rows across owner close/reopen, and is dropped separately.

```javascript
import {readFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
import init, {Facade} from '@hoyadb/client/wasm';
import {createRuntime, defineSchema, j} from '@hoyadb/client/host';
import {openMemoryDatabase} from '@hoyadb/testing';

await init({module_or_path: await readFile(new URL(
  import.meta.resolve('@hoyadb/client/wasm/hoyadb_web_bg.wasm')
))});
const facade = new Facade();
const runtime = createRuntime({
  open: (input, key) => facade.open(input, key),
  call: input => facade.call(input)
});
const schema = defineSchema({version: '1', tables: {notes: {
  columns: {title: {kind: 'String', optional: false}}, relations: {}
}}});
const store = await runtime.createMemoryStore();
const options = {
  runtime, schema, store, namespace: '00000000000000000000000000000001',
  idGenerator: () => 'guide-note'
};
let db;
try {
  db = await openMemoryDatabase(options);
  await db.insert('notes', {title: j.string('Offline 東京 🌿')});
  await db.close();
  db = await openMemoryDatabase(options);
  assert.deepEqual(await db.query('notes').fetch(), [{
    id: 'guide-note', fields: {title: j.string('Offline 東京 🌿')}
  }]);
  assert.deepEqual(await db.protection(), {
    encrypted: false, cipher: 'none', version: 'not_applicable',
    backup: 'not_applicable'
  });
} finally {
  await db?.close();
  await runtime.dropMemoryStore(store);
  facade.free();
}
```

Memory handles belong to one runtime and permit one open owner at a time.
Failed authored migrations preserve the retained original. A process or worker
exit loses memory stores. Explicit preload is outside any hermetic memory
measurement. The direct generated WASM TypeScript declarations additionally
require `ESNext.Disposable`; the portable host declarations do not. Runtime
cleanup here uses `free()`.

## Open protected browser storage

Call `createWebRuntime()` from `@hoyadb/client` once, then pass that runtime,
the schema, `engine: {kind: 'opfs', name: 'app.sqlite'}` and a caller-owned
32-byte `Uint8Array` as `encryptionKey` to `openDatabase`. A protected open
requires a dedicated worker and OPFS. Memory opens skip OPFS initialization.
The key travels in a separate binary open channel; keep key acquisition and
retention in the application. There is no plaintext fallback.

The fixed `hoyadb-v1` pool supports one active worker per origin, even with
different logical store names. Share the runtime for multiple owners. After a
transient pool initialization failure, a later explicit open can retry; the SDK
does not replay the failed key-bearing open. The default `createWebRuntime`
shares one live runtime. If its worker retires, a later no-argument call creates
a fresh worker, and concurrent replacement opens share that new attempt.
Existing handles remain closed; pending requests, owners and callbacks are not
migrated or replayed. An application-owned worker supplied through
`createWebRuntime({worker})` remains the application's responsibility and does
not replace the default runtime. Browser backup control reports `unsupported`.

## Keep values and clocks lossless

Use tagged cells such as `j.integer('9223372036854775807')`,
`j.timestamp('-9223372036854775808')`, `j.float('42.0')` and
`j.reference('record-id')`. Canonical decimal strings preserve integer widths;
unsafe JavaScript integer Numbers are rejected. Business JSON uses
`j.object`/`j.array` and the JSON tags, rather than timestamp/reference cells.

Command and entity writes capture the application's `context` and UTC `clock`.
Observed counts use a separate monotonic clock. Browser defaults use
`performance.now`; other hosts must provide `monotonic` and host timers when
that capability is unavailable. Managed observations deliver their initial
snapshot on the next host turn and retain one pending 16 ms count flush after
writes. Close/reset invalidates subscriptions and stale callbacks.

`db.execute(definition, args, async scope => result)` stages through scoped
CRUD across awaits and commits once at finish. The scope exposes `args` and
immutable captured `context`, and has no nested execute method. Independently
captured outer commands queue FIFO; awaiting one from its own active parent is
unsupported. Ordinary CRUD can use an injected ID generator. Surviving command
creations require `local:` IDs; automatic entity creations keep the core
allocator.

## Integrate the authoritative backend

Applications supply transport, TLS/authentication, timeouts and business
reconciliation. Push accepts an opaque canonical envelope-array body and returns
an opaque outcome-array body. Core codecs preserve business integers and exact
command/entity correlation. The SDK delegates durable retries and deadlines to
core and does not send empty batches. `createFakeTransport` from the testing
package uses the same codec and correlation rules for application handlers.

Configure entity tables explicitly. `setMergeHooks` receives immutable owned
inputs and returns a row or null; core validates the proposal and keeps merge
authority. An awaited hook holds no native registry lock. Busy retries retain
the proposal, while stale pulls refetch from the unchanged checkpoint. Reset
defaults to preserving recovery work; discarding it is an explicit application
choice.

Frames are limited to 1 MiB UTF-8, 64 containers and 100000 nodes, including
tags and envelopes. Readonly fetch returns the complete result or
`LimitExceeded`. Drain and push return bounded owned prefixes. A server result
whose tagged form does not fit one frame is refused with `LimitExceeded` for
that receipt before it is stored, even when its raw JSON is small (tags double
the nesting depth). Error objects
expose a static `kind`. See the executable
[consumer fixtures](../../tests/packaging/javascript) for relations,
migrations, managed observations, commands, mixed sync and passive merges.

### Sync integration practices

The following advice uses the declared [JavaScript application API](../../packages/client/application.d.ts),
whose transport exchanges plain values, unlike the tagged host transport above.
Match it to the installed release. The current [application driver](../../packages/client/src/application.js)
and [API-0012](../design/contracts/API-0012-javascript-application-binding.md)
define the supported behavior; application queues and UI scheduling are not SDK APIs.

**Keep network waits outside application read/write gates.** If a compatibility
layer serializes local operations, holding its gate around the entire `sync()`
call also blocks offline saves and reads while transport waits. Releasing every
gate for the whole sync is not a substitute: receive, reconciliation and pull
application can then interleave with a logical read assembled from several
pages or with asynchronous model materialization.

For such a layer, the public transport `push`/`pull` callbacks provide a boundary
around the external request. An application-owned reservation may release its
gates during that request and reacquire them before the callback returns or
throws to the SDK. Keep a consistent acquisition order, preserve the complete
logical read, and do not await an operation that needs a gate still held by its
caller. On account change, close or cancellation, retire the reservation and
prevent late responses, reacquisitions and materialization from changing the
replacement owner's data. This is an integration pattern to test, not a Hoya
lease API or a cross-process snapshot guarantee. Stopping SDK work does not
necessarily abort application HTTP or undo a server commit. Test a delayed
request alongside a local save, response arrival during a paged read, rejection,
and retirement while reacquisition is waiting.

**Treat `sync()` as a pass, not a queue-drained promise.** The current driver
captures `has_more` when preparing a batch, before HTTP and reconciliation.
A parent mapping can make a child eligible after that batch was prepared;
the child may need a later pass. Read `queueStatus()` after the pass and use
its authoritative `live` counter (`'0'` in this JS API) for outbound completion.
Zero outbound work alone does not certify pull completion or backend convergence.
The current application pass applies one successful pull page; use
`syncProgress()` to distinguish partial bootstrap from readiness. A
`PullConflict` is not a successful pull: settle the overlapping recovery work
before refetching, without relabelling the failed page as applied.

If the app chooses manual sync instead of `startSync`, continuation must be
finite and progress-aware. One application strategy is to capture the original
unsettled keys and allow another pass only when some of that work has settled
and unsent queued work is now unblocked. Do not extend that budget with every
new write, treat completion-history eviction as progress, or stop merely because
the capacity-level `blocked` flag is true. Stop when no ready queued work
remains, progress stops, or the owner changes; do not spin on work awaiting a
dependency, outcome, local recovery or retry deadline.
Core owns retry eligibility and backoff: do not poll `sync()` until empty, parse
write IDs as clocks, or rewrite frozen envelopes. Test parent/child/grandchild
waves, a child created during the parent's HTTP request, no-progress responses,
and cancellation. The public [status types](../../packages/client/index.d.ts)
and [dispatch contract](../design/contracts/API-0006-command-owner.md#dispatch-and-receipt)
remain the authority; this strategy does not introduce an app outbox.

**Replay the same outcome for the same accepted write.** Store the first
decision with the domain mutation and return a structurally identical outcome
on retry, preserving correlation, mappings, fields and timestamps. Rebuilding
it from today's row, adding a new deletion timestamp or changing an already
received result into a confirmation can produce `Protocol`. Return newer state
through pull. Neither bounded client completion history nor a guessed offline
duration establishes a safe server receipt expiry: losing deduplication can
apply an old update again or create a second record. Retention needs an explicit
protocol rule. See [receive and reconciliation](../design/contracts/API-0006-command-owner.md#authoritative-local-apply).

**Distinguish confirmation from correction.** An empty `writes` plan retains
mapping/reference validation but skips the inverse proof. It is useful when
the server accepted the submitted values and there is nothing to correct;
it is not a blanket translation for every `applied` or `corrected` outcome.
A full echoed row can interfere with a later local edit, while dropping a real
normalization can lose the server's decision. Decide once before storing the
receipt, and prove the choice against the installed engine.
The public `ApplicationPending` contains the retained outcome and identity
context, not the original frozen changes. A client mapper cannot infer that
comparison after restart from the current row alone. Classify where the frozen
request and stored result are both available, without inventing a second outbox.

For an entity backend using one timestamp for all returned fields, a possible
confirmation policy compares every submitted value and removal with the stored
result and requires the server stamp to be no older than the captured
`clientTimestamp`. Parse that timestamp strictly; do not substitute the write
ID's clock. This policy assumes the matching entity contract and default merge
rules: an authoritative equal-time value wins, but an older pull need not
replace a newer local value. A backend-owned timestamp field may therefore need
a real correction even if other values match. Retain typed corrections for
normalization, deletion, deduplication, upload filenames, or uncertain clocks.
Test a later same-row edit before the create reply, equal/future/backward clocks,
lost replies and reopen, and the eventual authoritative pull; successful local
retirement is not sufficient evidence of convergence.

Correction writes also have a scope boundary: a child outcome cannot freely
correct an unrelated parent, such as its favorite-child field. Typed entity
corrections outside the recovery closure remain refused. Read the
[closure and guard contract](../design/contracts/API-0006-command-owner.md#authoritative-local-apply)
and [entity correction rules](../design/contracts/API-0009-entity-pull-reset.md)
before designing such a result; declaring a reference is not permission to
overwrite the parent. Current contracts include a restricted later-write
fallback, so reproduce failures on the actual installed artifact rather than
assuming an older pre-release failure describes newer source. An empty plan is
not a general recovery procedure for an existing failed correction.

**Measure the operation and the visible result separately.** Record handler,
local commit, observation/materialization and destination-frame timings using
bounded diagnostic fields. An automation tap duration can include animation or
settling waits and is not a database latency measurement. Verify both displayed
ordering and server state after pending work settles, then verify a real process
restart. Keep transient recovered conflicts in the evidence; distinguish
retained-memory tests, native builds and actual device runs, and identify the
installed artifact when private versions are reused. Do not disable animations
or correctness checks just to shorten the automation number.


## Prepare related entity writes atomically

The portable host's `entityBatchAliased` allocates configured entity IDs inside one
core transaction. Give a parent insert a temporary alias, then reference it from
later writes through explicit Reference fields:

```javascript
await db.configureEntities('account-scope', ['nodes']);
const {changes, aliases} = await db.entityBatchAliased([
  {op: 'insert', table: 'nodes', alias: 'parent', fields: {title: j.string('Parent')}},
  {op: 'insert', table: 'nodes', fields: {title: j.string('Child')},
    references: {parent_id: 'parent'}}
]);
const parentId = aliases.parent;
```

This assumes `nodes.title` is a String column and `parent_id` is a declared
Reference column or to-one relation to nodes. Aliases are temporary and ordered;
only the returned real IDs enter rows or sync. Proposed IDs on configured entity
inserts remain invalid. Existing-ID replace/remove and ordinary local marker
writes can share the batch. Use `{op:'patch',table,id,fields}` for partial edits:
core reads the current staged row and preserves omitted fields. Supplied
same-value fields still record sync intent. A failed operation or over-budget
reply rejects the whole batch.

Rows inserted before entity configuration need application IDs. Without an
`idGenerator` or a proposed ID, the core allocates `local:<counter>`, a prefix
reserved for command-created rows awaiting a server ID. Configuring a table that
already holds such a row is rejected (`Protocol`) because no entity write could
address it afterwards. Supply `idGenerator`, propose IDs, or remove those rows
before calling `configureEntities` or `configureEntitiesDurable`; seeded rows
with any other ID are accepted.

For newly adopted stores that need saved IDs to survive server remapping and
sync history pruning, explicitly call
`configureEntitiesDurable(scope,tables)` instead of ordinary configuration.
It selects [format 4](../design/spec/SPEC-0049-durable-entity-aliases.md); existing
configured format-2/3 stores are rejected without conversion. Keep calling it
after every reopen: ordinary configuration of an opted-in store is rejected the
same way. Resolve a saved
reference with `await db.resolveEntityId(table, savedId)` before an ordinary query.
Both host and application APIs expose this resolver; queries are not rewritten
automatically. Configuration and reset behavior remain as specified by the core.
Rebuild local native/WASM artifacts after these source changes before installing
them in a consumer.

See the [batch contract](../design/spec/SPEC-0048-entity-batch-aliases.md) and
[shared memory/SQLite regression](../../tests/conformance/entity_aliases.rs).
