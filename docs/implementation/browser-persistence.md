# SPEC-0023 browser SQLite persistence — executed 2026-10-22

The same production Rust `SqliteStorage` now compiles for native bundled SQLite
and wasm32-unknown-unknown with the approved SQLite WASM backend. The standalone
[worker harness](../../spikes/storage-browser/README.md) uses OPFS SAH-pool;
CRUD, strict format/field validation and SQL transactions remain shared Rust.
Core stays dependency-free. This closes the bounded browser persistence
experiment; it does not complete roadmap Phase 1 or SPEC-0001/SPEC-0002.

## Reproduction and runtime

Run `spikes/storage-browser/check.sh` with the target compiler variables in its
README, then serve only its directory on loopback port 8766. No npm, custom VFS,
serialization dependency or production JS package is introduced. See the
[locked graph review](../../spikes/storage-browser/dependencies.md) for versions,
licenses, checksums and enabled features.

Observed host: x86_64 macOS, Rust 1.98.1, wasm-bindgen CLI 0.2.104, Homebrew
Clang/LLVM 22.1.8. Apple Clang 21.0.0 failed because it lacks a WASM backend;
selecting the already installed LLVM through command-local target variables
resolved this. No global compiler configuration/install changed.

Actual browser: Paseo 0.12.0 / Electron 44.2.0 / Chrome 152.0.7977.76 at
`http://127.0.0.1:8766`. Page and dedicated workers reported secure context true,
`crossOriginIsolated = false`. `curl -sI` observed HTTP/1.0 200, Python 3.14.8
SimpleHTTP/0.6, Content-type text/html, Content-Length 273, Last-Modified and
Date headers; no COOP or COEP headers. No browser security setting changed.
The backend's bundled SQLite header reports version 3.51.1; native SQLite
remains 3.53.2. These are different builds behind the same tested subset.

## Acceptance observations

The [complete captured results](browser-persistence-results.json) contain exact
owned rows, original duplicate-error coordinates, initialization capabilities,
malformed-input errors, fixture schemas and lifecycle results.

| Cases | Executed result |
| --- | --- |
| W01 | Writer worker terminated; recreated worker read complete committed typed rows from OPFS. Unicode, embedded NUL, empty/SQL-looking names, signed i64 extrema, both booleans, omitted fields, removed rows and empty-field rows matched. |
| W02 | Actual page reload, then a newly initialized worker read the same snapshots **before any insertion**. Only the test namespace was retained in sessionStorage, not rows or a database snapshot. |
| W03 | Ordered insert/replace/remove prefix failed at duplicate insert with exact RowAlreadyExists table/ID. Prefix and later writes were absent; original complete rows survived immediately, worker recreation and page reload. |
| W04 | Original shared M01/M02/M04–M09 and T01–T07 Rust assertion functions passed in the worker, with ready Send futures. M03 logical-table isolation passed separately; simultaneous independent instances explicitly excluded. Native/memory M03 and all original assertions remain intact. |
| W05 | Actual capability check and real OPFS initialization passed. Simulated init-unavailable branch returned synthetic BackendFailure and exposed no adapter; recorded separately from actual APIs. No real storage-denial/quota test is claimed. |
| W06 | Unknown operations, stale/disposed handles, malformed rows/fields/batches, nontext and out-of-range i64 inputs rejected before writes. Owned snapshots could be changed in JS without changing stored state. Subsequent valid reads worked; malformed prefixes/rows remained absent after recreation. |
| W07 | Unrelated sentinel database and unsupported version-2 private marker rejected; their complete recorded schema and sentinel/marker data remained unchanged. Empty/memory/URI opens rejected. Simulated open/write failures returned BackendFailure before invocation, without partial writes; these are not real VFS/COMMIT failures. |
| W08 | Explicit dispose, worker termination/recreation and reopen preserved rows. Stale handles rejected; subsequent insert and batch removal succeeded. Final page-reload cleanup uses acknowledged pool shutdown before deleting its fresh UUID namespace and terminating the worker (SPEC-0024). |
| W09 | A fresh task Cargo cache fetched both locks, then passed offline root browser-library compilation/Clippy and harness compilation/Clippy/fmt; bindings generated with CLI 0.2.104. Real-browser checks used generated bindings without COOP/COEP. Native/memory and existing spike regressions below passed. |

Initial RED is recorded in commit `1c4eb34` and the harness RED.md: actual browser
worker loading failed because the worker did not exist, and the production WASM
build failed with rusqlite E0432 because no browser backend was selected. These
were absence checks, not a running stub's failed transaction. Subsequent GREEN
exercised actual SQLite/OPFS and worker/page lifecycle, not a Node mock.

## Regression and review

Executed locally after the target extension:

- Workspace: 40 tests; format and Clippy with warnings denied; core/memory WASM.
- Shared isolated spike: 11 tests, format, Clippy and WASM compilation.
- Native spike: eight Rust tests, format, Clippy and rebuilt static library.
- Swift: N01–N05 and N07–N09; separate persistent writer PID 29233 and reader
  PID 29060 both exited 0 with exact restored rows. One N07 latency sample was
  18,829 ns, including immediate host drain; no benchmark guarantee follows.
- Existing browser bridge: B01–B05 passed before and after actual reload;
  its memory sentinel remained absent before reinsertion.
- Browser experiment: target build/Clippy/fmt and JS syntax checks; fresh-cache
  locked preparation and offline build via check.sh passed.
- Design traceability: zero violations; RTM check current; no accepted ADR body
  changed. Remote GitLab CI was not run or published.

Focused self-review checked SQL-value binding reuse, explicit OPFS selection
(no backend-default memory fallback), retain-on-init, complete batch validation
before mutation, checked Rust i64 parsing, temporary builder disposal, correlation
and stale-handle rejection, one serialized worker/connection, exact typed domain
errors, no unsafe project code, text-only diagnostics and namespace-only cleanup.
No blocking finding remains in this bounded slice. Operator review/merge gate
remains separate; no independent agent was started during this quota-conscious
resume. CI-ready compile commands are in check.sh; wiring them to GitLab awaits
validation of a WASM C compiler in that runner/container.

## Remaining gates

No concurrent tabs/processes, WAL, crash/power-loss durability, eviction/quota,
private mode, hostile-file protection, encryption or backup exclusion is proved.
Failure simulations do not close those gaps. Browser connection transfer is
unsupported; only owned messages cross threads. SQLite calls execute
synchronously before returning already ready Send futures.

Roadmap Phase 1 still includes storage query/change responsibilities, complete
schema/type representation and default-engine obligations. Implementation-plan
Stage 1 still has production bridge/handle contracts and remote CI review open;
clock/ID/fake-transport seams are introduced when their first tests require them.
Schema/query behavior is sequenced into plan Stage 3 / roadmap Phase 2, live
query semantics into Stage 4 / roadmap Phase 3, and encryption/queue guarantees
into later reviewed slices. These dependencies must be resolved explicitly;
passing this CRUD/batch subset does not complete REQ-0034 or the full product.

## Clean-source lifecycle finding and correction — DEF-0001

A git archive of a19d223, without untracked files/build outputs, passed the
browser compile script, root 40 tests, formatting and Clippy. The archived
browser also passed exact worker/page data and rollback checks, then cleanup
failed with actual NoModificationAllowedError. This archive run was **not**
fully green. Earlier root runs had completed cleanup successfully.

Investigation read the approved VFS's pool ownership and pause_vfs implementation.
TC-0024 then deterministically proved deletion is blocked after SQLite disposal
while the worker/pool remain alive; shutdown failed at the missing dispatcher
branch in regression RED (5ad3967). The fix retains the utility, refuses shutdown
with a live adapter, and acknowledges synchronous pool release after disposal.
Deletion must now succeed before terminating that same worker. L01/L02 and the
full persistent-state checks pass in the real browser at port 8767. Namespace
markers survive failure, and the run UUID is visible for diagnosis. No deletion
error is ignored or retried, and no delay or global storage reset was added.

Worker/generated asset URLs use fresh test query keys to avoid cached modules
masking a rebuild. Loading occurs within the serialized init request after the
message handler is installed. Origin and OPFS namespace stay unchanged across
restarts; the page reload still reads before any insertion. A separate mutation
check in the task-owned archive omitted explicit OPFS selection and failed with
`unexpected adapter before OPFS installation`, confirming the no-memory-fallback
assertion detects that defect. The tracked adapter was never mutated.

The remaining full Phase 1 query/index decision is presented in RFD-0001; no
indexing direction is adopted without the operator's choice.

The earlier failed archive namespace was identified at port 8766 by its unique
harness prefix and file modification times matching that run (23:34:19–25 UTC).
The corrected harness reopened it, checked every expected complete row before
inserting anything, then acknowledged VFS shutdown and removed only that UUID
namespace. [Recovery results](browser-cleanup-recovery.json) preserve the actual
namespace/check list. The later complete port-8767 run also cleaned its own
namespace successfully. No origin-wide storage operation was used.
