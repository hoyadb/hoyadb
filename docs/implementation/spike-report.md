# Stage 0 validation report — executed feasibility findings

Date: 2026-10-21. Scope follows [the roadmap](../../ROADMAP.md#phase-0--the-validation-spike).
All seven roadmap exit questions now have recorded experiment results and
limitations. This closes the bounded Stage 0 experiment, not production support
or approval to merge. Browser persistence is a recorded negative result for the
selected memory path; Phase 1 must supply its persistent browser adapter.

## Exit evidence

| Roadmap criterion | Observed result |
| --- | --- |
| 1. Host core | 11 Rust tests pass for typed rows, status filtering, membership events and tracked scope lifecycle. |
| 2. Real browser | SPEC-0017 B01–B05 pass in Chrome 152.0.7977.76 / Electron 44.2.0 / Paseo 0.12.0 on macOS. The WASM bridge calls the same core and prints complete typed rows in page/console. |
| 3. Native binding | Real x86_64 macOS Swift host calls the same Rust core through a separate C ABI static library (original memory mode plus approved native SQLite mode). N01–N05 pass. |
| 4. Persistence | SPEC-0021: real Swift writer PID 92818 and independent reader PID 92912 both exit 0 with exact typed rows restored from SQLite 3.53.2. Browser memory sentinel is absent after reload; no browser persistence mechanism is present. |
| 5. Reactivity | Native N07 passes using explicit drain delivery. One insertion-to-callback sample: 25,075 ns, OS thread 66234674, including immediate drain scheduling. Browser B05 passes with explicit JavaScript drain/callback and reentrant read; one write-to-callback sample was 3.5 ms including immediate drain scheduling. |
| 6. Async handler mechanism | Native N08–N09 pass for a platform-driven handler with real suspension, staged re-entry, outside isolation, atomic insertion publication, abort and one active scope per store. Global/cross-store serialization, other writers and stuck-handler liveness are not resolved. |
| 7. Findings report | This report answers every numbered exit question and records sourced dispositions for the roadmap open questions, affected artifacts, limitations and next production gates. |

## Reproduction and TDD

Run commands from [the core README](../../spikes/cross-platform/README.md) and
[the native README](../../spikes/cross-platform/native/README.md). Tool output
confirmed Rust 1.92.0-nightly, Apple Swift 6.3.2, Apple clang 21.0.0 and
x86_64 macOS. The initial host/native slices added no external runtime dependencies. The operator
subsequently approved wasm-bindgen 0.2.104 and rusqlite 0.40.2 with reviewed locked
transitives. Browser implementation uses only wasm-bindgen beyond the local core;
no npm runtime dependency was added.

Observed RED/GREEN steps:

1. Original five host cases failed at missing insert/query operations, then passed.
2. Native N01–N05 compiled/linked and failed because the stub returned no live
   store; after implementation all five passed. Three Rust error tests failed
   at panic/poison/allocation stubs, then passed.
3. Five core event/scope cases failed on missing observation, then passed.
   New Swift N07–N09 compiled/linked and failed on missing native observation,
   then passed after the bridge was implemented.

Formatting, Clippy with warnings denied, Swift compilation with warnings as
errors, traceability and RTM checks have passed. A Clippy test-module placement
finding was corrected. Browser B01–B05 also passed after five expected missing-operation failures. Native restart persistence also passes as recorded below. Crash/power-loss and mobile execution remain untested.

## Boundary findings

The separate C ABI bridge preserves the core's Rust-owned strings and typed
values. Callers receive copied row snapshots through synchronous borrowed byte
visitors and must copy bytes during the callback. Checked integer store handles
avoid dereferencing caller-provided store pointers. Null/UTF-8/boolean input,
stale handles and ordinary panic handling have tests; arbitrary pointer validity
and foreign unwinding remain caller preconditions, tracked by THR-0009.

The notification mechanism is poll/drain, not push. Core owns status matching
and membership, then queues subscription token/count signals. The native bridge
extracts events, releases its lock, rechecks liveness before each callback and
invokes Swift on the drain caller's thread. Reentrant reads and unsubscribe work.
Queued counts describe enqueue time; re-query returns current rows. Multiple
commits before draining do not create historical snapshots. This experiment
does not establish observer sharing or general updates/deletes.

The handler mechanism is platform-driven, one of the roadmap's candidate shapes.
Swift opens a core scope, runs developer code across an await, then explicitly
finishes or aborts it. Core state rejects a second scope and enforces staged
visibility. A busy scope rejects instead of queueing another handler. Native
runtime ownership contains no held Rust reference or mutex across the await.
Scope abandonment, timeout/cancellation, cross-store coordination and interactions
with entity-diff/pull writes remain unknown. No transport or server was added.

## Open questions and affected design

| Roadmap question | Current disposition |
| --- | --- |
| React Native / craby event path | Current [Signals documentation](https://craby.rs/docs/guides/signals) describes asynchronous Rust-to-JS notifications with optional typed payloads, multiple listeners and cleanup. Its [limitations page](https://craby.rs/docs/limitations) still excludes `NativeEventEmitter`, but explicitly offers Signals. This supersedes the roadmap's earlier inference that there is no event path. Not adopted or executed here; RN thread affinity, re-entry and latency require a separate experiment. |
| UniFFI host callbacks / JS target | Mozilla lists [JS/RN bindings as third-party](https://github.com/mozilla/uniffi-rs/blob/main/README.md#third-party-foreign-language-bindings), separate from its built-in Swift/Kotlin targets. Official [async guidance](https://mozilla.github.io/uniffi-rs/latest/futures.html) supports async foreign traits; the RN generator documents [async TypeScript callbacks](https://jhugman.github.io/uniffi-bindgen-react-native/idioms/async-callbacks.html) and cooperative cancellation signals. This documents a candidate mechanism, not hoyadb re-entry, ownership or D17 correctness. Tool/version selection and execution remain required before adoption. |
| Direct WASM in React Native | No RN runtime was tested. RN [bundles Hermes per RN version](https://reactnative.dev/docs/hermes). Hermes's [WASM feature request](https://github.com/facebook/hermes/issues/429) remains open; a [maintainer comment](https://github.com/facebook/hermes/issues/429#issuecomment-2356481723) recommends prior wasm2js conversion, and a [later comment](https://github.com/facebook/hermes/issues/429#issuecomment-2646786285) points to an external integration. Direct wasm-bindgen compatibility is not established; the browser proof cannot be claimed as RN support. |
| Desktop Node N-API path | [Node-API](https://nodejs.org/api/n-api.html) supplies a stable addon ABI and thread-safe callback scheduling. Its usefulness versus the tested WASM path remains unmeasured. Not selected or executed; Swift already proves the required native desktop target. A Node/Electron packaging decision remains binding-breadth work. |
| rusqlite WASM / browser persistence | The [0.40.2 manifest](https://raw.githubusercontent.com/rusqlite/rusqlite/v0.40.2/Cargo.toml) offers `ffi-sqlite-wasm-rs`, targeting optional `sqlite-wasm-rs ^0.5.1`; the approved `default-features = false` disables that default feature. The [0.5.1 backend documentation](https://docs.rs/sqlite-wasm-rs/0.5.1/sqlite_wasm_rs/) describes memory storage by default and a separate `sqlite-wasm-vfs` dependency for dedicated-worker OPFS SAH-pool or relaxed IndexedDB storage. Neither listed persistent VFS supports multiple connections. No such browser integration was built here. Selecting a backend/VFS, reviewing and approving added dependencies, then testing reload persistence remain necessary for Phase 1 web storage. |
| rusqlite versus redb default | The operator approved rusqlite 0.40.2 with bundled SQLite and default features disabled for native persistence work. This selects the implementation candidate for that experiment; it does not by itself establish cross-platform compatibility or close SPEC-0002's production engine decision. Record the observed native results and browser limitations before proposing the governed default-engine update. |
| Stuck handlers / fine-grained serialization / other writers | Unresolved. The executed platform-driven scope enforces one active handler per store and rejects a second. An abandoned scope can therefore block subsequent scope creation; no watchdog or cancellation policy exists. Finer serialization and exclusion of entity-diff/pull writes require explicit Phase 4 policy and tests. The mechanism permits host-side lifecycle handling but does not implement it. |
| Indexing / pagination / observer sharing | Not implemented. Status membership is tracked in core for the insertion-only fixture; no general indexing choice is established. Indexing and sorting/pagination beyond the documented single limit remain Phase 2 design work; subscription sharing remains Phase 3 work. These mechanisms were outside the minimal spike. |
| Multiple tabs/windows/processes on one store | No shared-store concurrency result is established by the browser or Swift tests. Official [SQLite browser guidance](https://www.sqlite.org/wasm/doc/trunk/persistence.md) distinguishes SAH-pool from VFS options intended for concurrent tabs, with different worker/header/locking constraints. Those JS-bundle options do not establish equivalent Rust VFS support. Supported topology, lock behavior and cross-instance notification propagation remain explicit design/validation work. |
| Concrete encryption library | The [rusqlite 0.40.2 feature documentation](https://raw.githubusercontent.com/rusqlite/rusqlite/v0.40.2/README.md) describes SQLCipher and crypto-provider variants; plain bundled SQLite is not that feature. The WASM backend documents a separate SQLite3MultipleCiphers feature, which does not prove native SQLCipher format compatibility. No cipher, provider or key-management mechanism is adopted or tested here. Selection and additional dependency approval remain Phase 6 work; no at-rest security claim is made. |

Primary sources checked on 2026-10-21 while investigating candidates are linked
with each finding above. Earlier boundary references also include
[Rust linkage](https://doc.rust-lang.org/reference/linkage.html),
[Swift C interoperability](https://www.swift.org/documentation/articles/wrapping-c-cpp-library-in-swift.html),
and [UniFFI callbacks](https://mozilla.github.io/uniffi-rs/latest/types/callback_interfaces.html).
Candidate documentation is evidence of available approaches, not executed proof
or approval to add those candidates. Future-phase decisions remain open where
the spike does not supply the evidence needed to choose them.

Findings inform ARC-0002, SPEC-0001/0002, SPEC-0006 and SPEC-0008. No accepted ADR
body has been changed. SPEC-0016/0018 and TC-0018 track this experiment; existing
full-product cases remain only partially exercised. Stage 0 findings review recommends keeping the Rust core, the proven browser
WASM boundary and a native SQLite candidate for the next foundation slice.
The platform-driven handler and explicit drain mechanism work for the bounded
fixtures, with host scheduling/lifecycle obligations retained. No wholesale
architecture replacement or accepted-ADR body change is indicated by this
experiment. Production adapter conformance, browser persistence, concurrency
policy, encryption and remote CI remain later gates.

2026-10-21 follow-up: the operator explicitly requested starting Phase 1 as far
as possible. The separate [storage foundation](phase-one-foundation.md) starts
the dependency-free workspace/memory subset without promoting spike code or
closing these gates. All 14 Rust spike tests and eight Swift scenarios also
passed under the workspace's newly pinned Rust 1.98.1 toolchain.


## Browser execution — SPEC-0017

Reviewed cases and boundary requirements were committed in `1aa1ff7` before code.
All five browser cases failed at the intended missing insert/query/observe
operations, then all passed after the thin Rust bridge was implemented. B04
round-trips i64 minimum/maximum as BigInt, rejects wide/non-BigInt values in the
JS wrapper and rejects invalid decimal text directly in Rust, without mutation.
The actual synthetic Unicode row appears as text on page and in console.

The host drains owned token/count snapshots, then calls JavaScript after the
WASM mutable borrow has ended. B05 can re-enter for a read, suppresses irrelevant
writes and stops after unsubscribe. Callback mutation/disposal/recursive drains
are outside this browser slice. The sample is not a performance guarantee.

A memory-only sentinel is retained until pagehide, then freed. Page reload
reruns the harness with `hoyaReloadRowsBefore = []`; there is no browser storage
mechanism in this bridge (no OPFS/IndexedDB/localStorage). Browser persistence
is therefore a negative finding for this selected memory path, not evidence
that SQLite cannot work on the web. A browser persistent adapter remains needed
for roadmap Phase 1 storage.

Reproduce using [the browser README](../../spikes/cross-platform/web/README.md).
The lockfile records all 14 external packages: MIT OR Apache-2.0, except
unicode-ident with additional Unicode-3.0. Cargo fetched checksum-verified
packages from crates.io; licenses were inspected via resolved metadata.
This is dependency/license review, not a vulnerability audit or release approval.


## Native restart persistence — SPEC-0021

Reviewed spec/cases and storage publication threats were committed in `37fc08d`
before tests. Three Rust persistence cases failed at the stub's zero handle;
two rejection cases already passed. A real Swift writer compiled and linked
with warnings denied, then exited 1 because persistent open was unimplemented.
After implementation all five persistence cases and three original error tests
pass, as do 11 shared-core Rust tests and the original eight Swift scenarios.

The writer inserts Unicode/i64-extreme rows through the existing C ABI, commits
two staged rows and aborts another scope, closes and exits. A separate reader
restores and checks every typed field. The observed writer PID was 92818 and
reader PID 92912; both exited 0. A subsequent independent reader PID 93566 also
passed. The harness-owned file was `/tmp/hoya-swift-persistence.UKQPls/verified.sqlite`;
reproduction creates its own fresh path, not this historical fixture.

SQLite runtime is 3.53.2. Rust uses the pinned toolchain; Swift/clang use an
explicit x86_64 macOS 26.5 deployment target for this bundled C library, tested
on macOS 26.7.1. Other deployment floors remain unvalidated. All native/core
formatting and Clippy checks and Swift warnings-as-errors builds passed.

Persistent writes clone the same core Store, apply existing matching/staging
semantics, commit all committed rows in one SQLite transaction, then publish the
candidate. A SQL-trigger failure returns status 4 and preserves memory, disk,
queued events and the active scope; retry succeeds after removing the trigger.
Restoration validates types/boolean range/unique IDs before registering a handle.
The experiment costs O(total rows) per persistent commit and does not prove
crash/power-loss durability, encryption, concurrent writers or queue correctness.

The native lockfile has 11 external packages. Resolved metadata licenses are
MIT or MIT/Apache-2.0; checksum-verified Cargo downloads were used. Only the
native spike depends on rusqlite; both Rust cores remain dependency-free.
See [native reproduction commands](../../spikes/cross-platform/native/README.md).
