---
id: TC-0023
title: Browser SQLite worker preserves exact rows across worker and page restart
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-21
updated: 2026-10-21
parents: [TP-0001]
verifies: [REQ-0034, REQ-0039, NFR-0003]
tags: [proposal, test, browser, wasm, sqlite, opfs, persistence]
---

# TC-0023: Proposed browser persistence cases

These cases require SPEC-0023's reviewed committed preparation and explicit
runtime dependency approval before test implementation/execution. All data and
OPFS paths belong to a fresh unique harness namespace; no existing origin data
is reset. Real-browser results must include complete typed rows and original
error coordinates, rather than counts or successful WASM compilation alone.

```gherkin
@REQ-0039 @NFR-0003
Feature: Single-worker browser SQLite persistence
Scenario: W01 exact committed data survives worker recreation
  Given a real browser with one dedicated worker and one OPFS SQLite connection
  When that worker creates two logical tables and commits inserts replacements and removals
  And the acknowledged writer worker is terminated and a new worker opens the same test database
  Then complete rows preserve Unicode embedded NULs empty and SQL-looking names signed i64 extrema and both booleans
  And empty-field rows exist omitted fields and removed rows remain absent and unrelated rows remain unchanged
  And the new worker reads SQLite through the production Rust adapter rather than a retained memory store
Scenario: W02 exact committed data survives a page reload
  Given W01 committed fixtures in the harness-owned OPFS namespace
  When all workers are terminated and the page is reloaded at the same origin
  And a new worker opens that namespace with clear_on_init false
  Then complete rows match the prior expected snapshots before any fixture insertion
  And the harness reports actual browser origin worker lifecycle and visible typed results
Scenario: W03 failed batch remains rolled back after worker and page restart
  Given committed rows across two logical tables
  When one batch applies a valid insert replacement and removal prefix then a duplicate insertion and a later mutation
  Then the first exact RowAlreadyExists is returned with its original table and ID
  And every original complete row is unchanged and no prefix or later mutation is visible
  When the worker is recreated and subsequently the page is reloaded
  Then the same rollback state is read from OPFS before any fixture insertion
Scenario: W04 applicable shared CRUD and batch outcomes agree
  Given the browser mapping of the shared memory and native SQLite conformance assertions
  When the applicable single-connection CRUD and batch assertions run in a real worker
  Then values owned snapshots exact domain errors replacement removal ordering and rollback match API-0002
  And the report names every exercised M01 to M09 and T01 to T07 assertion and any excluded portion
  And M03 simultaneous independent instances is explicitly excluded rather than claimed passed
Scenario: W05 unsupported or denied storage exposes no substitute adapter
  Given a deterministic test-owned initialization seam reporting a required OPFS capability unavailable or access denied
  When initialization is attempted
  Then BackendFailure and a visible unsupported or storage-denied result are reported
  And no usable adapter memory fallback alternate VFS or partial writes are exposed
  Given a real browser worker with its actual capabilities recorded
  When initialization is attempted without a failure seam
  Then actual capability success or failure is recorded separately from the simulated branch
Scenario: W06 malformed and stale boundary inputs preserve persistent state
  Given a worker with known committed rows and a checked test bridge
  When malformed messages unknown operations stale or disposed handles invalid typed values or out-of-range i64 inputs are submitted
  Then each request is explicitly rejected before any mutation
  And complete original rows remain unchanged immediately and after worker recreation
  And subsequent valid requests still work without retaining a Rust borrow across messages
Scenario: W07 persistent initialization preserves ownership and reports failures
  Given test-owned incompatible database bytes or an unsupported private-format marker
  When that database is opened through the worker
  Then BackendFailure is returned before resetting stamping or modifying its schema or data
  And no usable adapter or silent memory fallback is exposed
  Given the harness dispatcher simulate_failure open or write branch for a dedicated fixture
  When that branch returns a synthetic error before any VFS or adapter operation
  Then the simulated BackendFailure is distinct from domain errors and no partial row or successful-batch acknowledgement is returned
  And the report does not claim an actual VFS failure was exercised
Scenario: W08 supported single-owner lifecycle releases storage
  Given one live worker connection owns the dedicated test database
  When the connection is explicitly disposed and the worker is terminated
  And a new worker initializes the same namespace and opens the same database
  Then complete committed rows are readable and subsequent CRUD and batches succeed
  And no old handle is usable and no simultaneous-connection support is claimed
Scenario: W09 exact locked graph builds and preserves native regressions
  Given the operator-approved reviewed graph and matching wasm-bindgen 0.2.104 CLI
  When locked dependency preparation and browser compilation lint and generation run with a fresh task Cargo cache
  Then the selected versions and enabled features match the approved graph
  And native bundled SQLite memory and existing spike checks still pass
  And the real browser worker runs without requiring COOP or COEP headers or cross-origin isolation
  And actual headers and crossOriginIsolated are recorded without disabling browser security
```

W04 maps M01/M02/M04–M09 and T01–T07 where their assertion helpers are
worker-compatible. M03 logical-table isolation remains applicable, but its
simultaneous independent-instance assertion is not. Do not remove or weaken
the complete native/memory M03 test. A helper relying on native thread parking
cannot be claimed executed unchanged in a browser; review any test-only
executor adaptation and retain meaningful ready-future assertions.

W05 uses injected capability/initialization results only in test harness code;
it must not monkeypatch browser security settings or introduce a production
bypass. W07's incompatible data and failure injection are restricted to
test-owned fixtures and an explicit test-only seam; real VFS errors must be
recorded separately, without treating a simulated failure as evidence for
quota exhaustion, crash or hostile-file protection. Define the smallest seam
and review it before implementation; do not expose it to consumers.

W01/W02/W03 require observations after terminating/recreating actual workers
and after an actual page navigation/reload. An in-worker reopen, Node process,
browser console mock or replayed fixture result is insufficient. A write is
successful only after the real SQLite transaction commits. Record ordinary
restart durability only, not power-loss or browser-eviction durability.

The operator approved the runtime graph on 2026-10-21. Execution on 2026-10-22
passed W01–W08 in a real worker/browser and W09 locally, including a fresh
locked-cache build. Complete rows, exact error coordinates, actual reload and
limitations are in [the execution report](../../../implementation/browser-persistence.md).
M03 simultaneous instances, real storage denial/VFS failure, crash/power-loss
and remote CI are not claimed. W05/W07 simulations are labeled explicitly.
