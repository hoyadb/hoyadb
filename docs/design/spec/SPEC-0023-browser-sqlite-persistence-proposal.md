---
id: SPEC-0023
title: Propose a single-worker SQLite OPFS persistence experiment
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-21
updated: 2026-10-21
parents: [ARC-0002, ADR-0008]
satisfies: [REQ-0034, REQ-0039, NFR-0003]
depends_on: [SPEC-0001, SPEC-0002, SPEC-0017, SPEC-0022, API-0002]
tags: [proposal, browser, wasm, sqlite, opfs, persistence]
---

# SPEC-0023: Browser SQLite persistence proposal

Origin: the operator requested completion of Stage 0 and Phase 1. Browser
execution of the memory spike does not establish a persistent web adapter.
The operator explicitly approved the exact browser backend/VFS/helper candidates
and reviewed locked transitive dependencies on 2026-10-21. This approval is
separate from the earlier native dependency approval. Review the resolved graph
before building. Reviewed committed cases remain a gate before implementation.
The isolated bridge/harness lives at `spikes/storage-browser/`, excluded from
the production workspace; the shared adapter is extended by target selection.

## Proposed dependency and compatibility boundary

Keep native `rusqlite = "=0.40.2"`, default features disabled, with bundled
SQLite unchanged. On `wasm32-unknown-unknown`, propose the same rusqlite version
with default features disabled and `ffi-sqlite-wasm-rs` enabled instead of the
native bundled configuration. Explicitly pin the following compatible candidates
and review the complete resolved graph before adoption:

| Candidate | Proposed role and checked manifest evidence |
| --- | --- |
| `sqlite-wasm-rs = "=0.5.1"` | MIT, Rust minimum 1.82; wasm-bindgen ^0.2.104 and js-sys ^0.3.81. Constrains rusqlite's optional browser backend range to this reviewed candidate. |
| `sqlite-wasm-vfs = "=0.1.1"` | MIT; depends on sqlite-wasm-rs ^0.5.1, wasm-bindgen ^0.2.104, js-sys/web-sys ^0.3.81 and wasm-bindgen-futures ^0.4.54. Use its OPFS SAH-pool module. |
| `wasm-bindgen = "=0.2.104"` | Retain the already approved bridge version and matching CLI; browser persistence broadens its use and requires review. |
| `js-sys = "=0.3.81"`, `web-sys = "=0.3.81"`, `wasm-bindgen-futures = "=0.4.54"` | MIT OR Apache-2.0; these exact companion manifests require wasm-bindgen =0.2.104. Propose only the features actually needed. |

These are manifest-level compatibility findings, not an observed successful
resolution, compilation or browser run. VFS 0.1.1 unconditionally includes
IndexedDB helpers and Tokio synchronization even when only OPFS is selected;
there is no SAH-pool-only feature to remove them. Review all locked runtime and
build transitives, licenses, checksums and enabled features rather than approve
only the visible package names. Do not add serialization, npm runtime packages,
another executor or a custom VFS without a further reviewed decision.

VFS 0.1.1 is older than the checked latest 0.3.0. The latter requires
wasm-bindgen ^0.2.128 and documents a different VFS/backend generation. Selecting
0.1.1 is a bounded compatibility experiment, not a recommendation that it is
current, security-audited or production-ready. Any upgrade requires explicit
approval and rechecking the exact dependency graph/CLI compatibility.

## Proposed behavior and reuse

Extend the unpublished SPEC-0022 `SqliteStorage` adapter by target-specific
dependency selection, reusing its production Rust CRUD, strict private-format
validation, typed decoding and SQL transaction/batch implementation. Keep core
dependency-free. Do not reuse the Stage 0 row-copy persistence implementation
or replace SQLite persistence with a memory snapshot written by custom code.

A thin test-only WASM/JavaScript bridge initializes the VFS asynchronously
inside one dedicated worker, then opens one `SqliteStorage` connection against
that VFS. The checked candidate API is
`sqlite_wasm_vfs::sahpool::install(&OpfsSAHPoolCfg, bool).await`; the boolean
chooses the default VFS, not whether to clear stored data. Configure a dedicated
test-owned OPFS directory and explicitly retain `clear_on_init = false`.
Only after successful VFS initialization may the adapter open its database.
The browser filename is a literal VFS database name, not a native filesystem
path or URI; document this target-specific interpretation in API-0002 before
implementation. No production JavaScript consumer API is promised here.

One worker serializes access to one active connection. No Rust database handle,
borrow or connection crosses to the main thread. JavaScript messages carry
owned synthetic fixture data and correlation identifiers; reject unknown
operations, malformed payloads, stale/disposed handles and out-of-range i64
values before mutation. Use checked decimal text or validated BigInt conversion
so generated bindings cannot wrap a wide integer before validation. Return
complete owned rows and stable existing core error variants; diagnostic message
text remains unstable. Render fixture results as text, never HTML.

VFS/open failure, unavailable required APIs and storage denial return an explicit
BackendFailure result and expose no usable adapter. The harness labels the
unsupported condition; it never silently selects SQLite memory, IndexedDB or
another engine. Missing capability is a recorded limitation, not a passing
persistence case. Existing nonempty incompatible databases remain rejected
without initialization/reset, retaining SPEC-0022 ownership/version checks.

CRUD and batches still perform synchronous SQLite work on the dedicated worker
and return already resolved Send futures. Async VFS initialization does not
change the Storage trait into nonblocking I/O. Prove compilation of this bound
on the selected target; do not add unsafe Send/Sync implementations to force
JavaScript objects across threads. Browser connection/thread transfer remains
unsupported even though native ownership-transfer tests exist.

W05/W07 failure injection is confined to the test worker's message dispatcher.
A named harness-only `simulate_failure` request identifies `init`, `open` or
`write` and returns a synthetic BackendFailure before invoking the corresponding
VFS/adapter operation. It exposes no usable connection on init/open failures,
performs no write and changes no stored data. Required-capability simulation
returns an explicit unsupported result through the same harness dispatcher;
actual capability checks and real initialization run separately. No production
adapter flag, alternate VFS, browser-security modification or fake commit is
introduced. Reports label these as simulated protocol/error-branch evidence,
not as actual VFS quota, commit-failure or power-loss tests. Native SQL failure
injection remains separate executed evidence for the shared transaction code.
The smallest worker protocol/harness bindings may translate core rows/mutations
through checked typed field builders and owned snapshots without adopting a
serialization framework. Document the concrete names after implementation;
malformed input is rejected before an adapter call.

## Runtime and scope limits

Require a secure context, dedicated-worker execution, WebAssembly, ES modules,
BigInt, OPFS `getDirectory` and synchronous file access handles. Serve synthetic
test assets on loopback; production HTTPS deployment is not delivered here.
The selected SAH-pool candidate documents no COOP/COEP requirement; verify that
the harness works without cross-origin isolation and record actual headers and
`crossOriginIsolated`. Do not infer this property for other SQLite VFS options.

The candidate is single-threaded, with one active connection and no WAL or
concurrent-tab/store guarantee. Reserve the harness-owned OPFS namespace only
for this experiment. Never clear or migrate unrelated origin data, open an
existing user database, or change browser security settings to pass a test.
Clean process/worker/page restart evidence is not power-loss, eviction, quota,
private-browsing, hostile-file or multi-instance durability evidence. Encryption,
keys, backups, queries, subscriptions, command queues and schema migrations
remain outside this slice; accepted full-product obligations remain in force.

## Acceptance, review and delivery gates

TC-0023 enumerates real-browser worker, restart, boundary and error cases.
Review and commit this proposal, REQ-0039/THR-0013, cases and the necessary
API-0002 target clarification first. Obtain explicit operator approval for the
runtime graph before fetching/building/adopting those dependencies (now
recorded above).
Then capture meaningful RED for absent behavior, implement minimally, and
record GREEN with actual browser version, origin, headers, worker lifecycle,
complete typed rows and errors. WASM compilation or a Node run alone does not
satisfy these browser cases.

Reuse applicable M01–M09/T01–T07 conformance assertions through test-only
helpers without weakening native/memory cases. Enumerate the actual browser
mapping: M03's simultaneous independent-instance portion does not fit the
one-active-connection topology and must not be claimed passed. Its logical
table isolation can be checked separately; sequential reopen is a different
case. Native thread transfer Q08 and native file/lock fixtures do not establish
browser behavior. No complete cross-platform conformance claim follows from
this subset.

Required regressions after implementation: workspace tests/fmt/Clippy, existing
host/native/browser spike suites, browser-target adapter compilation and lint,
locked clean-cache preparation, traceability/RTM checks and focused security
review. Preserve existing runner tags. Add repeatable CI compile commands only
with the approved graph; record separately whether real-browser automation and
the remote GitLab pipeline actually ran. Do not push, merge or publish.

Open compatibility questions to answer with evidence: does this exact graph
resolve with the 0.2.104 CLI; does the shared SQL format/open implementation
work through this VFS; are ready Send futures usable without cross-thread JS
ownership; which target methods need clarification; do error and disposal paths
release handles sufficiently for worker recreation; and what exact browser
capability/security restrictions affect initialization and persistence?

Rollback removes only this experiment and its target-specific dependency and
harness changes through review. Preserve native/memory implementations,
allocated IDs and any test-owned data needed for diagnosis. Cleanup may remove
only the unique namespace created by the current harness, never origin-wide
storage. No released data format, user migration or package publication exists.

## Primary evidence inspected for the proposal

Published package manifests/source were read without installing packages:
[rusqlite 0.40.2 manifest](https://raw.githubusercontent.com/rusqlite/rusqlite/v0.40.2/Cargo.toml),
[sqlite-wasm-rs 0.5.1 package](https://static.crates.io/crates/sqlite-wasm-rs/sqlite-wasm-rs-0.5.1.crate),
[sqlite-wasm-vfs 0.1.1 package](https://static.crates.io/crates/sqlite-wasm-vfs/sqlite-wasm-vfs-0.1.1.crate),
[VFS 0.3.0 comparison](https://static.crates.io/crates/sqlite-wasm-vfs/sqlite-wasm-vfs-0.3.0.crate),
[js-sys](https://static.crates.io/crates/js-sys/js-sys-0.3.81.crate),
[web-sys](https://static.crates.io/crates/web-sys/web-sys-0.3.81.crate),
[wasm-bindgen-futures](https://static.crates.io/crates/wasm-bindgen-futures/wasm-bindgen-futures-0.4.54.crate).
Runtime guidance:
[backend VFS/thread limitations](https://docs.rs/sqlite-wasm-rs/0.5.1/sqlite_wasm_rs/),
[synchronous access handle](https://developer.mozilla.org/en-US/docs/Web/API/FileSystemSyncAccessHandle),
[OPFS directory access](https://developer.mozilla.org/en-US/docs/Web/API/StorageManager/getDirectory),
[SQLite VFS distinctions](https://www.sqlite.org/wasm/doc/trunk/persistence.md#vfs-opfs-sahpool).

## Execution checkpoint — 2026-10-22

The bounded experiment is implemented and locally verified; see
[the execution report](../../implementation/browser-persistence.md),
[full typed results](../../implementation/browser-persistence-results.json) and
[harness commands](../../../spikes/storage-browser/README.md). The shared adapter
explicitly selects `opfs-sahpool` on WASM because the backend otherwise installs
a memory VFS by default. Root native configuration remains unchanged. The
matching locked graph and CLI build successfully with a WASM-capable Clang;
actual worker recreation/page reload preserve exact rows and batch rollback.
No production package, full-platform conformance, encryption, real VFS failure
or remote CI result follows from this experiment. Artifact acceptance and
operator review remain separate from executed evidence.
