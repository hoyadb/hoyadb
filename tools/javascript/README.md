# Local JavaScript SDK artifacts

For AI-assisted consumers, the [integration catalog](../../AI.md) and
[skill](../../skills/hoyadb-integration/SKILL.md) describe available tools,
dependency verification and identity/sync boundaries. The builder copies both
into each package with the same relative layout and records their source hashes.
Installed consumers read `AI.md` and `skills/hoyadb-integration/SKILL.md` from
their package directory; no runtime export or AI plugin is required. Other
repository-relative links in this README require the matching source checkout.

`@hoyadb/client` and `@hoyadb/testing` are MIT-licensed 0.1.0 packages for local consumer
verification. They are not published. The shared JavaScript entry is
`@hoyadb/client/host`; the browser entry also exports `createWebRuntime`. All
schema, query, relation, observation, command, sync and merge decisions execute
in the Rust facade/core. `testing` supplies capabilities, not another database.

## Application API

Use `@hoyadb/client` with `defineSchema`, `table`, `column`, `relation` and
`defineCommand` for plain application rows and arguments. Builder-branded schemas
open with automatic runtime bootstrap: a shared browser worker, or ordinary
packaged WASM on Node. `@hoyadb/client/node` is the explicit Node entry.
`@hoyadb/client/application` is the portable convenience layer for an injected
runtime. Existing `/host` tagged DTO callers remain unchanged.

The complete tested Todo app is `examples/application.mjs`. Build the
HTML docs and open `docs/public/dist/getting-started.html` for short, ordered
snippets: schema, database, query, observer, scoped handler, backend, sync and
cleanup. Do not copy the advanced bootstrap below into a beginner application.

Commands bind explicitly through `db.command(definition)`. Handler writes share
one core admission and commit atomically; outer owner writes return Busy while a
handler is admitted. `query.observe` immediately returns an unsubscribe function
with a `ready` Promise. That function resolves after the owner closed and after a
failed registration; other unsubscribe failures are returned. A recoverable
data error goes to the optional error callback, else to `onBackgroundError`,
and keeps the listener; a terminal error event is never passed to a status,
progress or outcome callback as a value. Entity tables are an explicit opt-in;
`table().link`/`unlink` use the entity entry points when the relation's through
table is configured. Plain transports
are encoded by the core codec; command-created IDs require explicit server
mappings, while omitted entity-created identity mappings are filled from stored
recovery context. Application `startSync` also schedules pull; advanced tagged
`startSync` stays push-only.

Node's default runtime is memory-only. Browser filename storage always requires
a caller-owned 32-byte key. No plaintext fallback, automatic key generation,
server authorization or release is provided.

## Command conflict policy

Commands default to `conflict:{default:'stop'}`. To keep current rows when an
eligible command's inverse proof conflicts, opt in with exact command names,
for example `conflict:{default:'stop',commands:{'edit-v1':'discard'}}`, and supply
`onResolution(key, resolution)` to observe that choice. The core still enforces
guards, entity protections, later-work exclusions and in-flight restrictions.
When supplying a policy, `default` is required; only `commands` may be omitted.
Invalid DTO shapes are rejected before open acquires an owner. The core remains
responsible for command-name validation and the current owner's admission rule.

`reconcileResolving(key, plan)` and `rollbackResolving(key)` return immutable
`{kind:'completed'}` or `{kind:'kept_current',discarded_keys,unmapped,skipped_writes}`.
`skipped_writes` is a full-width unsigned decimal string. The tagged host also
accepts explicit guards as `reconcileResolving`'s third argument. Existing
`reconcile`, `reconcileGuarded` and `rollback` return null after the same core
operation. `discard(key)` explicitly retires an eligible recovery closure while
keeping its current rows. A reported key may already have been history-trimmed.

Policy installation completes before open returns. `setConflictPolicy(policy)`
changes the active owner only; reopen reapplies the constructor's configuration.
`onResolution` runs once for each current-epoch KeptCurrent result. Its thrown
errors and rejected thenables produce redacted callback-failure facts through
`onBackgroundError`, without changing the commit or uninstalling the callback.
Close/reset suppress late reports from the retired epoch. When explicit cleanup
retry after an open failure is required, retain a `Database` instance and call
its `open` and `close` methods; a factory rejection cannot return that instance.

## Atomic prepared entity writes and saved IDs

The portable `/host` database exposes `entityBatchAliased(writes)` for ordered
parent/child inserts with explicit alias reference maps, mixed existing-ID
replace/remove, partial patch and ordinary marker writes in one core transaction.
The application and host databases expose `configureEntitiesDurable(scope,tables)`
and `resolveEntityId(table,id)` for explicit fresh-store durable ID mapping.
Existing entity configuration and `entityBatch` stay unchanged. The
[SDK integration guide](../../docs/guides/sdk-integration.md#prepare-related-entity-writes-atomically)
shows the wire-level example and opt-in/storage limitations. These methods need
fresh local artifacts; host forwarding tests alone do not certify mobile use.

## Build and verify

First build `crates/hoyadb-web/pkg` using the parent platform SDK Rust/WASM instructions.
`build.mjs` refuses to package when a Rust source or the web lock is newer than
those generated files, and leaves the previous `dist` untouched. This is a
modification-time guard against a certain mismatch, not build provenance.
Then run from the repository root, with the already installed Node22/npm11 and
TypeScript7.0.2 tools:

```sh
npm ci --offline --ignore-scripts --no-audit --no-fund
npm test
node tools/javascript/scripts/build.mjs
node tools/javascript/scripts/check-build.mjs
node tools/javascript/scripts/packed-consumer.mjs
```

The last command packs both artifacts, installs only those local tarballs into a
fresh OS temporary directory, typechecks a consumer, runs the actual bundled
WASM memory engine, and removes that exact directory. No registry or spike path
is used. The direct generated WASM type export uses the optional
`Symbol.dispose` declaration; its TypeScript consumer includes
`ESNext.Disposable` alongside `ES2022,DOM`. The portable `./host` declaration
does not require that extra library. Runtime disposal still uses `free()` and
never requires an application Symbol.dispose polyfill. `dist` and `node_modules`
are generated. Add `--serve` to the last command for the independent installed
browser consumer on loopback8878; terminate its reported parent process to stop
the owned server and remove its temporary tree. The source package has no JS
runtime dependency beyond testing's matching local client.

## Advanced tagged host: explicit bootstrap and engine choice

For browser production code, call `createWebRuntime()` once before opening
owners. It memoizes one dedicated worker/facade runtime. Different database
owners share that runtime and core FIFO. Browser protected storage requires a
caller-supplied32-byte `Uint8Array` encryption key; there is no plaintext fallback
or SDK-generated key. The worker initializes OPFS only for protected opens. A transient initialization
failure clears only that failed attempt; a later explicit open may retry using a
fresh key without replaying the failed open.
The caller's key is copied for the binary open channel and never placed in an
ordinary JSON request. A killed worker closes its channel; callers explicitly
bootstrap a replacement runtime rather than replay old requests automatically.

A real memory test uses an explicitly preloaded runtime. Node can import the
bundled WASM module from `@hoyadb/client/wasm`, read the separately exported
`@hoyadb/client/wasm/hoyadb_web_bg.wasm` asset, initialize it, construct `Facade`,
and pass its `open`/`call` methods to `createRuntime`. Complete that bootstrap
before measuring hermetic operations. The executable independent consumer shows
this sequence. Then `openMemoryDatabase({runtime,schema,namespace,...})` uses the
real core memory adapter and touches no key, path, entropy, network or OPFS.
Use an explicit32-hex testing namespace. Production namespace/jitter entropy is
an injected16-byte CSPRNG capability; the browser default uses `crypto`.

`runtime.createMemoryStore()` retains the actual in-process adapter across
close/reopen and authored migrations. Pass its handle as `store` to the memory
factory. One owner leases it at a time; drop is explicit and rejects while open.
The store disappears with its runtime/process. Memory protection reports exactly
`false / none / not_applicable / not_applicable`.

## Values, scopes and transport

`defineSchema` accepts the exact API-0011 DTO. `j.integer`/`j.timestamp` accept
canonical decimal strings, optional bigint, or safe integral Numbers. Strings
preserve fulli64 widths. `j.float('42.0')` keeps Float kind. `j.object`/`j.array`
box business JSON; typed reference/timestamp cells are not business JSON tags.
Queries build lazy DTOs and delegate every predicate/order/navigation to core.

Supply `context:()=>j.object(...)` and `clock:()=>canonicalMilliseconds` for
commands/entities. A command's async handler receives scoped CRUD, owned `args` and immutable
`context`, and no nested execute. Ordinary insertion/link proposals use an
optional injected `idGenerator`. A surviving command creation still requires
`local:`; ADR-0035 approves the exact `local:test-fixed-id-001` command example,
while ordinary CRUD retains exact unprefixed `test-fixed-id-001`.
Automatic entity creation retains the strict core allocator.

Transport receives a canonical opaque envelope-array body and returns an opaque
wire-outcome body. Applications own networking, TLS/auth, timeout and cancellation.
The SDK samples bounded jitter, delegates retries to core, and never sends an
empty push. `createFakeTransport` uses core `json_decode`/`json_encode`, supplies
losslessly tagged envelopes to business handlers and attaches exact command or
entity correlations. A handler returns `{status:'applied',result:J}`,
`{status:'corrected',result:J}` or `{status:'rejected',reason:String}`; entity
receipt-only may return `{status:'acknowledged'}`. Override/reject/delay `push` to
simulate transport failure. No opaque business numeric JSON crosses host Number.

`setMergeHooks({table:async input=>rowOrNull})` configures future pull sessions.
Inputs `{schema,current,metadata,incoming,default}` are immutable owned copies;
core computes the default and validates every returned row. Hook changes cannot
rewrite comparison authority, existence, IDs, tied/unrelated/removed cells or
schema bounds. Busy retries retain the exact proposal; stale sessions discard the
old page and fetch again from the unchanged checkpoint. Close/reset invalidates
in-flight hook callbacks and subscriptions. Managed observers schedule their
initial snapshot on the next host turn and one unpostponed16ms pending-count
flush after writes. `monotonic` and `timers:{set,clear}` are injectable; the guarded
default uses `performance.now`. Hosts without that capability must supply uptime
before registering an observer. UTC context clocks are separate.

Native channels used alongside Swift/Kotlin wrappers must additionally expose
`subscribeRelease(listener:()=>void):()=>void`. The platform adapter registers
the payload-free shared release event synchronously and returns an idempotent
unsubscribe. This only wakes an existing core admission/retry attempt; it
supplies no admission decision or data. The SDK disposes these subscriptions
on admission, completion, cancellation and owner invalidation. Browser-only
channels may omit the capability. Native adapter packages own the platform bus;
the portable client imports no React Native event library.

## Limits and evidence

ABI1 frames admit at most1,048,576UTF8 bytes,64 containers and100000 nodes. Tagged
values/envelopes consume that budget, so the binding can reject a value below a
core business maximum. A received server result whose tagged form cannot fit one
`pending_reconciliations` frame is refused with LimitExceeded before the
receipt is stored. `pendingReconciliations()` and `queueStatus()` make one call
when the list fits and otherwise follow the core's `*_page` key-cursor prefixes.
Large readonly fetch is complete or LimitExceeded;
drain/push return bounded owned prefixes without truncating snapshots. Errors
expose a static `kind`; diagnostic snapshots contain only the ten approved
count/Boolean fields. Explicit data APIs return application data.

The consumer directory is test code with caller schemas, not package dispatch.
`browser.html` uses ordinary assets; `control.html` uses a separate default-off
test-controls artifact to inspect only closed task-owned files. Assert those
exports absent from normal packages. Distinguish runtime checks, translation
mocks, build-only results and pending targets. Node channel mocks alone do not
establish a mobile runtime.
