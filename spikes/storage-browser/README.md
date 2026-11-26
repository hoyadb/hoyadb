# Browser SQLite persistence and typed queries — SPEC-0023 / SPEC-0025

This standalone harness exercises `SqliteStorage` with the pinned SQLCipher,
rusqlite, sqlite-wasm-rs and OPFS SAH-pool graph. Core remains dependency-free.
Raw/reactive fixtures use one dedicated worker and one active connection; the
command fixture permits distinct owned stores in that worker. It provides no WAL
or concurrent-tab/store guarantee. Unprotected synthetic fixtures do not establish
protected-storage support; only supplied-key tests exercise that boundary.
See the [dependency requirements](dependencies.md) and technical contracts.

Use Rust 1.98.1, wasm-bindgen CLI 0.2.129 and a Clang/llvm-ar supporting wasm32.
The verified host has Homebrew LLVM 22.1.8 at the paths below; Apple Clang 21
cannot compile the backend. These command-local variables do not change global
compiler settings. On another host, supply verified local compiler paths.
From the repository root:

```sh
PATH="$PWD/target/phase-six-browser/cli/bin:$PATH" CC_wasm32_unknown_unknown=/usr/local/opt/llvm@22/bin/clang AR_wasm32_unknown_unknown=/usr/local/opt/llvm@22/bin/llvm-ar sh spikes/storage-browser/check.sh
python3 -m http.server 8766 --bind 127.0.0.1 --directory spikes/storage-browser
```

`check.sh` fetches locked dependencies
before offline builds, verifies formatting/Clippy for the browser target and
regenerates ignored bindings. It requires the tools above; it does not install
or upgrade them. The `spike-storage-browser` CI job compiles the adapter and
harness with the pinned Rust image and Debian Clang/LLVM build tools. CI does not generate JS
bindings or run a browser; `check.sh` additionally regenerates local bindings.
Existing CI jobs/tags are preserved. No remote CI result follows from a local run.

Open `http://127.0.0.1:8766/` in a real browser. The first run writes into a fresh
`hoyadb-spec23-<UUID>` OPFS directory, then actually terminates/recreates workers,
checks domain errors, malformed inputs, rollback, ownership and disposal, and
runs M01/M02/M04–M09 and T01–T07 from the shared Rust assertion module.
SPEC-0025 adds the identical shared P01–P10 typed-query/batch assertions used
by native and memory, P11 strict physical-value/v1-format/actual SQL-trigger
rollback fixtures, and P12 indexed lookup measurements at 256 and 2048 rows.
P12 reports initial population, populated index construction, added population,
first/repeated numeric and text lookup, typed/raw update, rollback, ordering and reopen costs separately, alongside actual EXPLAIN plans;
these are synthetic observations without a latency target. Closed conformance
fixture files are removed between independent cases to stay within the SAH pool.
M03 logical-table isolation runs separately; simultaneous independent instances
are explicitly excluded. The browser executor requires ready Send futures and
never parks a thread. Native/memory retain every original assertion and their
native executor.

Wait for `window.hoyaPersistence.needsReload === true`, then reload the actual
page at the same origin. The new worker reads all expected rows **before any
fixture insertion**, including rows written by a typed batch and every new
primitive bridge kind. `window.hoyaPersistence.done === true` and every result's
`passed === true` indicate the second stage completed. The page renders full
owned rows and errors as text. Cleanup removes only this run's unique directory
after disposing the adapter and acknowledging VFS shutdown, before terminating
the still-live worker (SPEC-0024/TC-0024). Failed runs retain
their namespace for diagnosis; no origin-wide cleanup is provided.

The worker serializes messages with a promise chain. It installs its handler
before any async module load; unique asset query keys prevent cached generated
modules from masking rebuilds while keeping origin/data unchanged. Requests contain a numeric
correlation `request`, operation `op` and (after open) an opaque random `handle`.
Supported test operations are `init`, `open`, `dispose`, `create_table`, `insert`,
`get`, `replace`, `remove`, `batch`, `conformance`, `fixture`, `inspect_fixture`,
`shutdown` and `simulate_failure`. Additional fixed test fixtures use
`typed_conformance`, `typed_physical`, `typed_index_evidence`,
`typed_fixture_write` and `typed_fixture_read`; these do not expose a general
JavaScript schema/query builder. Shutdown rejects a live adapter, closes
pooled sync handles after disposal, and retires that worker. Rows contain an
exact string `id` and `fields` array of
`{name, kind, value}`. Kinds are String, Integer (decimal text checked by Rust
`parse::<i64>()`), Boolean, finite Float (JavaScript Number), Timestamp (decimal
i64 text), Reference (exact string) and Null (null). Integer and timestamp
payloads never pass through a JavaScript Number. The dispatcher validates types
before calling WASM, builds entire batches before mutation, and frees temporary typed builders.
Rust returns owned snapshots; foreign error frames expose only an allowlisted static kind (API-0010), with no coordinates or raw cause.
Stale/disposed handles reject. No Rust borrow or object crosses a message or
await; synchronous SQLite work stays on the worker.

VFS initialization alone is asynchronous. `clear_on_init` is explicitly false;
the VFS directory is a validated fresh test namespace. Rust opens explicitly
against `opfs-sahpool`, preventing the backend's default memory VFS from being
used before initialization. Actual API capability/init results are separate
from `simulate_failure` init/open/write branches. Those return synthetic
BackendFailure **before invocation**; they do not test real quota denial,
VFS failure, disk failure, failed commit, crashes or power loss.

Secure loopback and standard browser security are used. No COOP/COEP headers
are added, no isolation or security flags are changed, and there is no memory
or IndexedDB fallback. Required APIs are dedicated workers, WebAssembly, ES
modules, BigInt, OPFS directory access and synchronous file access handles.
Only synthetic local fixtures cross the boundary; they remain until this test
run cleans up. There is no telemetry or production-data input.

reactivity (`SPEC-0027` / `API-0005` / `TC-0029`) adds the production Rust
`ReactiveDatabase` owner to the same SQLite worker. `tests.js` runs the shared
R01–R10/R12/R15 cases and five R11 projection partitions, then real main-thread
callback assertions before its existing persistence/reload sequence. The
standalone `reactive.html` runs those reactive checks in a fresh owned namespace
and acknowledges VFS shutdown/cleanup without requiring a reload. Both pages
use the generated bindings from `check.sh`.

`reactive-client.js` is an experimental test wrapper with a fixed pending-orders
query, not a published JavaScript SDK. It checks listener and owner-generation
liveness before every extracted event, rejects recursive/concurrent drains,
allows callback reads/writes/unsubscribe/close, and isolates callback exceptions
in its local `callbackErrors` array. Count delivery remains host-driven with
API-0005's 16 ms interval; there is no automatic timer. Worker errors and owned
row/count snapshots cross structured-clone messages; i64/timestamp values and
u64 subscription handles retain checked decimal text. A fixture-only adapter
can fail one projected read to exercise actual core post-commit error delivery
and explicit refresh; this is not a simulated VFS or commit failure.

Executed reactivity evidence is under `results/`: `phase-three-red.json` records the
unsupported new operation before its bridge implementation;
`phase-three-green.json` records the standalone reactive checks;
`phase-three-full-stage1.json` and `phase-three-full-stage2.json` preserve the
full default harness and actual page reload/cleanup. The build manifest records
SHA-256 hashes of the runnable Rust/JavaScript/HTML/locked graph and generated
bindings used for those checks. Timing samples include the actual runtime,
origin and explicit drain cadence; they are observations, not a latency target.
The browser spike targets WASM; the native adapter remains validated by the
workspace's native checks.

Review-resolution regressions (`SPEC-0028` / `TC-0031`) extend both pages with
shared Q01–Q03 assertions, a 33,002-operand actual-worker fetch/projection probe,
and mixed row/column/count callback failures. The probe records `POOL.list()`
before and after selection; shared cases compare inventory after closing and
removing their fixture databases. These checks accompany the adapter's required
connection-local memory TEMP setting, without changing persistent storage.
The shared oversized-value partition covers all primitive kinds, Null, duplicate
values, multiple predicates, numeric extrema and Unicode.
Five shared cases execute as separate indexed worker requests, retaining the
full harness's original 20-second request deadline. Navigation/reverse-dependency
coverage uses 1,105 targets; a separate minimal 4,105-row owner fixture exercises
TEMP window materialization and intersecting operand sets. Case timings and
inventory comparisons are recorded individually in the final review evidence.

The test-only `fail_next_query` wrapper isolates full-row materialization faults
from projected count work. Main-thread checks require repeated explicit error
callbacks, exact recovery without another write, count registration from valid
keys, cancellation of dirty row listeners and unchanged count deadlines.
`review-resolution-*` evidence files retain this run separately from the earlier
`phase-three-*` artifacts. The RED browser run reproduces mixed-listener count
poisoning before the core correction; the SQL large-set correction was already
in that browser build. Native tests recorded the pre-correction SQL-size RED.


command sync (`SPEC-0029` / `API-0006`–`API-0008` / `TC-0032`) adds
`command.html`, the shared `spikes/common/command_fixture.rs`, and the
experimental `command-client.js`. This runs the actual Rust
CommandDatabase<SqliteStorage>; no host queue or entity engine substitutes for
the core. Raw/reactive fixtures and command owners are mutually exclusive;
multiple distinct command stores share the worker and application FIFO.

`check.sh` is the debug compile/format/lint gate. Before opening `command.html`
for the final command sync runtime profile, additionally generate the optimized
bindings with the exact commands at the end of this file. Then run the synthetic
command-only HTTP fixture from the repository root and open the command page:

```sh
python3 spikes/common/command_http.py --port 8774
node --test spikes/storage-browser/command-client.test.mjs
```

`http://127.0.0.1:8774/command.html` runs real main-thread suspended handlers,
FIFO/cancellation/disposal/callback checks, numeric precision, a failed transport
followed by real fetch and host timer retry, independent keyed HTTP outcomes,
and three actual page reloads. Each run owns a fresh OPFS namespace and cleans
it only after every owner closes and VFS shutdown acknowledges. The intentionally
suspended pre-commit handler is abandoned by its first reload. A later reload
recovers receipt-only state separately from the authoritative outcome. Local
reconciliation then permits the authored version-2 fixture migration.

`command.html?shared` first executes all 14 unchanged shared atomic-storage,
owner and sync assertions on actual OPFS. Each indexed case closes its stores,
deletes only its own exact unique fixture names, and requires identical
pre/post pooled-file inventory. The pool remains 24 slots. The configured-500
capacity case has an explicit 900000 ms test deadline; other worker requests and
ordinary host scenarios retain 60000 ms. The separate aggregate large-message
host scenario also has a finite 900000 ms watchdog; its individual RPCs remain
60000 ms and HTTP requests remain 5000 ms. Its total wall time is recorded
separately. These are harness bounds, not production SLOs or
a handler watchdog. Background browser throttling affects measured timer delays.

The command RPC uses only strings/booleans/null in its outer JSON and tagged
values for domain integers/floats, including integer extrema. JS never parses
canonical push envelopes or HTTP domain bodies. Host handlers await outside
Rust, with explicit abort for a never-settling handler. A response or application
reconciliation plan already received is retained across a temporarily Busy
scope and resumed on release, without repeating the HTTP request or mapper.
The wrapper checks generation/subscription liveness before every foreign
callback, and isolates callback exceptions from durable commit results.

## entity sync entity/pull/reset experiment

API-0009 extends that same production owner and shared Rust fixture with explicit
entity table configuration, automatic entity mutations, mixed command/entity
responses, bounded chunked pull, progress and caller-selected reset. The same
loopback server serves `/hoyadb/changes`; its synthetic pages and arbitrary
entity outcomes do not implement production authorization or a server database.

After the normal compile gates, the standard release artifact can be generated
using the release build/bindgen commands above. Run the HTTP fixture and open
`http://127.0.0.1:8774/entity.html?shared` (or the chosen loopback port).
The harness requests fifteen shared cases before the foreign host checks, using
the original 24-slot pool and exact per-case file cleanup. The two additional
review regressions cover tombstone mapping targets and pull after rolled-back
sent cancellation. The model harness also dispatches the same-migration to-one
backfill review regression. Compile checks establish registration, not fresh
browser execution evidence; historical packets below retain their original scope.
Main-thread checks use automatic entity writes, mixed HTTP retry/mapping,
visible partial bootstrap, Busy/StalePull, progress callbacks, multi-chunk UTF-8
uploads, explicit reset and two actual page reloads. Per-worker RPC deadlines
remain 60000 ms. `window.hoyaEntities` exposes the owned synthetic report.

Context accompanies each entity mutation in the same RPC, rather than a separate
mutable context-setting call. Pull bodies remain opaque text until Rust's strict
decoder; chunk offsets count UTF-8 bytes, and no partial upload commits. Reset
changes foreign generation/callback ownership only after the core transaction
succeeds. Late old-generation sync completion cannot clear a replacement pump.
These are experimental bindings; packaged SDKs and later roadmap phases remain
outside this harness. Final execution claims require source/artifact manifests
and completed cleanup, rather than the existence of this page or its test names.


The final command sync runtime also uses the standard optimized WASM profile. The
ordinary no-argument `check.sh` above remains the debug build/format/lint gate.
A debug large-message run exceeded its unchanged 60-second local RPC watchdog;
that failure remains in the evidence. The optimized run changes neither source,
dependencies, assertions, payloads, RPC/HTTP bounds nor browser security. From
the repository root, these exact additional commands generate the release
bindings before opening the same pages:

```sh
export CC_wasm32_unknown_unknown=/usr/local/opt/llvm@22/bin/clang
export AR_wasm32_unknown_unknown=/usr/local/opt/llvm@22/bin/llvm-ar
cargo clippy -p hoyadb-storage-sqlite --lib --target wasm32-unknown-unknown --release --offline --locked -- -D warnings
cargo build -p hoyadb-storage-sqlite --lib --target wasm32-unknown-unknown --release --offline --locked
cargo clippy --manifest-path spikes/storage-browser/Cargo.toml --target wasm32-unknown-unknown --release --offline --locked -- -D warnings
cargo build --manifest-path spikes/storage-browser/Cargo.toml --target wasm32-unknown-unknown --release --offline --locked
wasm-bindgen --target web --out-dir spikes/storage-browser/pkg spikes/storage-browser/target/wasm32-unknown-unknown/release/hoyadb_storage_browser_spike.wasm
```

The final manifest identifies the exact release artifact and complete source
hashes. Debug shared/diagnostic records keep their own manifests and outcomes;
a release PASS does not relabel a failed debug run. Each profile runs the full
unchanged shared/host/reload/legacy matrix. These measurements are synthetic
unpublished-build observations, not production latency or throughput promises.

## security supplied-key OPFS validation — API-0010 / TC-0035

The browser builder requires the exact wasm-bindgen CLI version declared by
the locked bridge graph. Its presence in a local cache is not supply-chain
approval. Keep CLI verification separate from database runtime acceptance.

Run the debug compile/Clippy gate above first. Before opening the final runtime
pages, generate optimized bindings with the exact approved graph:

```sh
CC_wasm32_unknown_unknown=/usr/local/opt/llvm@22/bin/clang AR_wasm32_unknown_unknown=/usr/local/opt/llvm@22/bin/llvm-ar cargo build --release --manifest-path spikes/storage-browser/Cargo.toml --target wasm32-unknown-unknown --offline --locked
target/phase-six-browser/cli/bin/wasm-bindgen --target web --out-dir spikes/storage-browser/pkg spikes/storage-browser/target/wasm32-unknown-unknown/release/hoyadb_storage_browser_spike.wasm
python3 spikes/common/command_http.py --port 8775
```

Open `http://127.0.0.1:8775/security.html`. The page supplies a fixed synthetic
32-byte key from application code on every open, never from queue state. It
runs unchanged shared typed/model/reactive cases, five large-operand/retry
review cases (including TEMP/SAH inventory checks), plus 14 command and 15 entity
cases through one scoped encrypted factory. `security_conformance` is a test-only
worker control with group/index and exact lowercase key hex; normal worker
ownership gates apply. A scope clears its owned zeroizing key buffer on every
normal or error return. A WASM panic requires worker termination. The foreign
request and JS memory copies are not claimed to be erased.

The page verifies encrypted capabilities, an actual immutable context through
await and its outgoing envelope, bounded diagnostics, complete durable retries,
physical pooled main/sidecar canaries, wrong-key/unkeyed/legacy nonmutation,
authenticated first-page tampering and a real page reload. It stops only its own
worker and removes only its fresh namespaced synthetic files. The postcommit
crash barrier terminates a worker with an open owner after successful committed
reads, then supplies the key to a new worker. This is distinct from a precommit
crash test. SAH-pool physical metadata reserves the first 4096 bytes; the tamper
probe changes a byte inside the first encrypted SQLite page after that header.
Browser backup exclusion is explicitly unsupported. No `persist()` substitute,
OS backup guarantee, key generation/rotation, mobile or FIPS claim is made.

The runtime publishes `window.hoyaSecurity`; successful completion requires
`done`, `cleaned`, and every result's `passed` to be true after its actual reload.
The capacity-500 shared command case alone keeps its reviewed 900000ms bound;
other RPCs remain 60000ms. Prior plaintext phase pages must also pass on these
same generated artifacts. Capture source/WASM/JS hashes before runtime and
recheck them afterward; do not relabel older evidence as this binary.
