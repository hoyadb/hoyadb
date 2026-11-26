# TC-0023 RED — 2026-10-22

Before implementation, the committed test harness at loopback port 8766 ran in
Paseo 0.12.0 / Electron 44.2.0 / Chrome 152.0.7977.76 on macOS. Runtime reported
secure context true and crossOriginIsolated false. The first worker request
failed (`Worker failed: undefined`); server returned 404 for the absent
`worker.js`. No persistence result passed.

Separately, `cargo build -p hoyadb-storage-sqlite --target
wasm32-unknown-unknown --offline --locked` exited 101: rusqlite 0.40.2 emitted
E0432, unresolved import `libsqlite3_sys`. The current native-only dependency
selection cannot build the production adapter on this browser target.

## TC-0024 lifecycle regression RED — 2026-10-22

After the a19d223 clean-source data/reload assertions passed, namespace cleanup
intermittently failed with NoModificationAllowedError. SPEC-0024/TC-0024 and
DEF-0001 were committed in a00936b before the regression test change.

The deterministic regression at loopback port 8767 again passed W02/W03 exact
rows, then disposed the SQLite adapter while keeping its worker alive. Deleting
its unique namespace returned actual NoModificationAllowedError, proving the
VFS pool retained handles. The subsequent `shutdown` request failed with
BackendFailure `stale or disposed handle` (there is no shutdown branch). This is meaningful behavior RED, not a
missing import or compiler failure. The namespace marker is retained on failure.

A separate mutation check in a task-owned source archive omitted explicit OPFS
selection. The existing initialization assertion failed with BackendFailure
`unexpected adapter before OPFS installation`, proving it detects the backend's
implicit memory VFS. The tracked implementation was never mutated.

## TC-0025 typed storage RED — 2026-10-22

SPEC-0025/API-0003/TC-0025/THR-0014 were committed in `8bf2a55` before
this test change. The new primitive bridge case ran at loopback port 8768 in
Paseo 0.12.0 / Electron 44.2.0 / Chrome 152.0.7977.76, with secure context true
and crossOriginIsolated false. Existing W01/W03/W04/W05/W06/W07/W08 assertions
reached before it passed. Inserting the new finite Float/Timestamp/Reference/
Null fixture failed with `BackendFailure: unknown field kind`, the expected
missing bridge behavior. This was a real worker execution, not a compile test.

P13 separately asserted that `.gitlab-ci.yml` contained the browser harness
WASM build command. It failed with `AssertionError: P13 RED: configured CI has
no browser adapter/harness compile path`. An unmounted probe of the exact
pinned Rust image verified rustc 1.98.1 and found no Clang or llvm-ar. Installing
Debian build tools inside an ephemeral container supplied Clang/LLVM 19.1.7;
an actual wasm32 C object compiled successfully before mounting source read-only.

## TC-0026 data model browser RED — 2026-10-22

SPEC-0026/API-0004/TC-0026 were committed in 52bb980, with recoverable-open
clarification 2c26b16 before implementation. The new real-browser shared model
assertion at loopback port 8769 failed after the baseline W04, shared P01–P10,
physical P11 and P12 checks passed. The request for `model_conformance` returned
BackendFailure `stale or disposed handle`: the dispatcher had no data model branch.
This is expected missing bridge behavior, not a compilation failure. An earlier
run timed out during the unchanged baseline conformance; an idle rerun reached
the new request without changing its 20-second timeout or any assertion.

The successful native restart regression first failed because the writer did
not support the new `model` scenario, then passed with separate writer/reader
processes and exact migrated rows, relation navigation and next-ID continuity.

## reactivity reactive boundary — TC-0029

Before adding the WASM reactive owner or dispatcher operations, the real browser
loaded `reactive.html?reactivity=red` at `http://127.0.0.1:8770`. Initialization
succeeded, but `reactive_open` failed with `BackendFailure: stale or disposed
handle`, because the existing dispatcher had no reactive operation. The initial
open assertion failed; its fresh OPFS namespace was shut down and removed.
The actual runtime and result are retained in `results/phase-three-red.json`.
After implementation, the same open boundary plus shared core/projection and
main-thread callback cases pass in `results/phase-three-green.json`. Full
baseline/reload evidence and the source/generated hash manifest are alongside it.

## Review-resolution row/count isolation — TC-0031 Q03

Before the core row/count failure isolation correction, the actual worker at
`http://127.0.0.1:8772/reactive.html?review=failure` committed `local:1`, then the
fixture-only full-row read failed. Default and per-column listeners correctly
received BackendFailure, but the equivalent count listener incorrectly received
the same error instead of Count(1). The assertion expected count events
`[0, 1]`; the actual events were Count(0), BackendFailure. Shutdown and removal
of that run's unique OPFS namespace succeeded. The raw output is preserved in
`results/review-resolution-failure-red.json`. The SQL large-set fix was already
present in this build; this RED proves the independent observation defect.
