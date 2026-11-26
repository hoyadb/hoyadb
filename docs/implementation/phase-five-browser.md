# Phase 5 browser entity sync, pull and reset evidence

Status: final frozen-source optimized browser execution **PASS**. This report records the
experimental browser lane of [API-0009](../design/contracts/API-0009-entity-pull-reset.md)
and [TC-0034](../design/tests/cases/TC-0034-entity-sync-conformance.md).
The same release artifact passed all 24 Phase 5 report groups and all earlier
browser regression groups below. Exploratory runs and compiling behavioral REDs
are retained separately from that final completion gate.

## Artifact and commands

The executed runtime artifact uses the standard Cargo `release` profile, the
existing dependency lockfiles, wasm-bindgen 0.2.104 and the pinned Rust 1.98.1
toolchain. No custom optimization flags, payload reductions, relaxed deadlines
or browser security flags are used. Final acceptance applies to that optimized
artifact; it does not establish debug-profile runtime performance. Phase 4's
debug large-message timeout remains historical failed evidence in its report.

Run from the repository root:

```sh
export CC_wasm32_unknown_unknown=/usr/local/opt/llvm@22/bin/clang
export AR_wasm32_unknown_unknown=/usr/local/opt/llvm@22/bin/llvm-ar
cargo clippy --manifest-path spikes/storage-browser/Cargo.toml --target wasm32-unknown-unknown --release --offline --locked -- -D warnings
cargo build --manifest-path spikes/storage-browser/Cargo.toml --target wasm32-unknown-unknown --release --offline --locked
wasm-bindgen --target web --out-dir spikes/storage-browser/pkg spikes/storage-browser/target/wasm32-unknown-unknown/release/hoyadb_storage_browser_spike.wasm
node --test spikes/storage-browser/command-client.test.mjs
python3 -m unittest discover -s spikes/common -p 'test_command_http.py'
python3 spikes/common/command_http.py --port 8775
```

The loopback server serves the browser assets, mixed HTTP reference and typed
pull pages. It performs no authorization and does not implement a production
backend database. The [source/artifact manifest](phase-five-browser-source-sha256.json)
records 109 source hashes, unchanged before/after the build, and two generated
JS/WASM hashes. All 111 hashes were checked again after runtime and matched.
The [build/test output](phase-five-browser-test-output.txt) records commands,
tool versions and observed successful exit statuses. Exploratory manifests do
not stand in for that source closure.

## Unchanged shared assertions on OPFS

Open `http://127.0.0.1:8775/entity.html?shared`. The test-only indexed export
calls the shared Rust functions directly against actual SQLite/OPFS. It does
not copy the entity, comparison or queue algorithms into JavaScript.

| Index | Shared function |
| --- | --- |
| 0 | entity_sync::automatic_entities |
| 1 | entity_sync::admission_and_inverse |
| 2 | entity_sync::two_client_resolution |
| 3 | entity_pull::pull_atomicity_bootstrap_and_reset |
| 4 | entity_pull::reset_inventory |
| 5 | entity_pull::lifecycle_comparison_guard |
| 6 | entity_pull::pull_recovery_protection |
| 7 | entity_pull::progress_transition_guard |
| 8 | entity_pull::reset_modes_and_coordinator_guard |
| 9 | entity_sync::managed_links_and_mapped_rows |
| 10 | entity_sync::reconfiguration_guards |
| 11 | entity_pull::typed_correction_clocks |
| 12 | entity_pull::reconfigure_inventory |

Indices 0–8 retain their original meanings. Each case allocates unique owned
files, closes its owners and deletes only its exact generated prefix. Its named
file inventory must match before and after. The original SAH pool has 24 slots;
the harness does not grow it. Each shared case has the unchanged 60000 ms
worker deadline.

## Foreign host and lifetime checks

After the thirteen shared cases, the main thread exercises eight report groups:

1. Automatic create/patch and mixed command/entity HTTP arrays, offline retry
   with exact frozen bytes, identity mapping and dependent release.
2. Two actual GET pages, visible committed partial data, readiness and progress.
3. A held response meets a suspended command handler: Busy retains the original
   ticket/body, handler abort discards staging, and one fetched page is applied.
4. An intervening local mutation makes a held page stale; a fresh fetch succeeds.
5. PreserveRecovery refuses live recovery; explicit discard succeeds and retires
   old replies and callbacks only after commit.
6. Typed authoritative correction uses its supplied field clock; a later pull
   selects the newer backend value instead of a fabricated optimistic clock.
7. Close/reopen permits a replacement sync pump while an old transport is held;
   late completion cannot clear the replacement, and the old store retains recovery.
8. A Unicode page larger than 262144 bytes crosses sequential UTF-8 chunks;
   complete decoding/application and explicit total reset preserve the bounds.

Three additional report groups span **two actual page reloads**: an incomplete
page/checkpoint survives, the next page resumes and completes, then an offline
entity mutation/recovery survives a second reload. Default reset refuses that
recovery and explicit discard clears it. The final run passed **24/24 reports**
(13 shared + 8 host + 3 reload). The shared hidden-no-op recovery regression at
index 6 passed in 542 ms; every shared case restored the identical empty named
file inventory. The large pull retained its exact 300000-byte Unicode value.

Every ordinary RPC and host group retains its 60000 ms watchdog. Upload chunks
are at most 262144 UTF-8 bytes; the complete body and outer RPC frame remain at
most 1048576 bytes. Escaping may require smaller chunks. Raw response text stays
opaque in foreign code until Rust's production PullPage decoder. Context is
captured in each mutation request, and the existing lossless tagged values
preserve i64 extrema, Unicode/NUL and integral Float identity.

Success additionally requires VFS shutdown acknowledgement and removal of the
owned namespace. `window.hoyaEntities` records results, timings and cleanup;
its test names alone do not establish acceptance.

## Behavioral RED and intermediate checks

The ignored local `target/` evidence preserves compiling behavioral failures:

| Regression | Observed failure | Evidence |
| --- | --- | --- |
| Shared entity operations | Existing dispatcher rejected entity configuration | phase-five-host-entity-red.txt |
| Shared pull/reset operations | Existing dispatcher rejected the new operations | phase-five-host-pull-reset-red.txt |
| Entity-only receipt | Command acknowledged response was incorrectly accepted | phase-five-host-receipt-red.txt |
| Typed correction | Existing plan parser rejected the entity write | phase-five-host-correction-red.txt |
| Actual worker typed correction | Old WASM rejected the entity reconcile write | phase-five-browser-correction-red.json |
| JS entity/pull/reset methods | Compiling functional stubs failed three assertions | phase-five-client-red.txt |
| Reset generation | Old transport completion cleared a replacement pump | phase-five-client-generation-red.txt |
| Close/reopen generation | An old suspended transport prevented a new pump | phase-five-client-reopen-red.txt |

These cases have targeted GREEN evidence and permanent regression assertions.
The final lightweight rerun passed **14/14 Node tests** and **5/5 Python HTTP
tests** (`phase-five-client-final.txt`, `phase-five-http-final.txt`). Earlier
exploratory actual-browser runs passed the original nine shared cases plus
then-current host/reload checks and cleaned their namespaces. Those runs predate
the final thirteen-case export and core fixes; they are not final source closure.

## Final verification record

| Gate | Final result |
| --- | --- |
| Release build and wasm-bindgen | PASS; release build 1m 11s |
| Browser WASM Clippy, warnings denied | PASS; 25.07s |
| Phase 5 thirteen shared + host/reload groups | 24/24 PASS; two actual reloads |
| Phase 4 fourteen shared + command host/reload groups | 27/27 PASS; three actual reloads |
| Legacy persistence initial page + actual reload | 36/36 then 12/12 PASS |
| Legacy reactive groups | 17/17 PASS |
| Source/artifact hash closure and namespace cleanup | 109 source + 2 generated hashes matched; all owned namespaces cleaned |

The earlier browser matrix runs on the **same final artifact**: `command.html?shared`
(including unchanged configured-500 capacity and large-message cases),
`index.html` with its requested actual reload, and `reactive.html`. The configured-500
case retains its 900000 ms deadline; ordinary command RPCs retain 60000 ms,
HTTP retains 5000 ms, and the aggregate large-message watchdog remains 900000 ms.
The configured-500 case took 39.662 s and restored the identical empty pool
inventory. The aggregate large-message scenario took 9.011 s. These are measured
synthetic runtimes on this machine, not latency or throughput guarantees.

[Final browser results](phase-five-browser-results.json) contain the 24 Phase 5
and 27 command report groups, timings, capabilities, cleanup and historical
typed-correction RED. The [legacy results](phase-five-browser-legacy-results.json)
contain the 36 initial, 12 reload and 17 reactive groups. Together these are
**116 passing runtime report groups**, plus the separate 14 Node and five Python
tests. The final browser was Paseo 0.12.0 / Chrome 152.0.7977.76 / Electron 44.2.0
on Mac Intel, at loopback origin 8775 with crossOriginIsolated=false.

Two attempts to create the command tab failed during automation-host tab
registration; the inventory showed no registered tabs. Creating an origin tab
recovered the browser surface. Its ordinary persistence/reload cleanup completed,
then navigation on the registered tab ran the complete command matrix. These
setup failures remain in the result diagnostics; no assertion failed and no
source, artifact, workload, security setting or deadline was changed. All owned
browser tabs were closed after their namespace cleanup. After the native lane
confirmed completion and the parent authorized cleanup, the exact owned HTTP
process was stopped with SIGINT; its owning session exited successfully and
port 8775 no longer accepted connections. The result records that cleanup.

## Limits

This is one experimental browser/worker OPFS target. It establishes neither a
packaged JavaScript SDK nor other browsers/mobile platforms, production backend
membership/authorization, multi-writer safety, power-loss durability, universal
latency or safe tombstone compaction. Pure/client first-arrival fixtures check
the selected authoritative result, not a deployed server transaction. No runtime
dependency, network publication or user-store operation is added by this lane.
