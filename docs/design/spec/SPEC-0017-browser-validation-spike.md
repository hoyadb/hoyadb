---
id: SPEC-0017
title: Run the validation spike in a real browser
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-20
updated: 2026-10-21
parents: [ADR-0008, ARC-0002]
satisfies: [REQ-0001, REQ-0003, REQ-0007, REQ-0008, REQ-0034, REQ-0036]
depends_on: [SPEC-0016]
tags: [spike, wasm, browser]
---

# SPEC-0017: Run the validation spike in a real browser

Origin: operator instruction to proceed after the host slice. This draft
defines the next Stage 0 slice. The operator explicitly approved the proposed
wasm-bindgen dependency and reviewed locked transitive dependencies on 2026-10-21.

## Scope and proposed implementation

Prove roadmap exit criterion 2 with a thin WASM bridge that delegates to the
existing `spikes/cross-platform` Rust core. The core retains its dependency-free
API and owns insertion and query semantics. Proposed new paths are
`spikes/cross-platform/web/` for the bridge crate and its browser harness.
Do not create a production workspace, public package or JavaScript database.

Expose an independently owned store through generated bindings. Translate
strings, booleans and signed 64-bit totals without losing precision. The
browser harness invokes insertion and querying from JavaScript, compares full
returned rows without relying on ordering, and prints actual rows and named
pass/fail results to the browser console and page. Use synthetic fixtures only.
Generated artifacts and build outputs are ignored and reproducibly generated.

This slice does not add persistence, async handlers, native
bindings, mobile support or sync. A fresh store after a reload is expected
because this engine remains in memory; record that limitation explicitly.
Browser success is not full Stage 0 completion.

## Approved dependency

The operator approved adding `wasm-bindgen = "=0.2.104"` only to the bridge
crate, matching the locally installed `wasm-bindgen` CLI 0.2.104. Commit its resolved transitive dependencies
in the bridge's lockfile; review resolved versions and licenses before use.
No npm packages, bundler, serialization framework or `web-sys` dependency is
needed: the harness performs browser DOM/console operations in JavaScript.

Primary sources checked in this session:

- [Tagged manifest](https://raw.githubusercontent.com/wasm-bindgen/wasm-bindgen/0.2.104/Cargo.toml):
  version 0.2.104, Rust minimum 1.57, MIT OR Apache-2.0. The bridge itself
  inherits the experiment's Rust 2024 toolchain needs; the dependency's
  minimum is not a claim about the project's minimum supported version.
- [Official no-bundler example](https://wasm-bindgen.github.io/wasm-bindgen/examples/without-a-bundler.html):
  `--target web`, ES module loading and asynchronous initialization.
- [Official numeric mappings](https://wasm-bindgen.github.io/wasm-bindgen/reference/types/numbers.html):
  i64 uses JavaScript BigInt, and out-of-range BigInts can wrap. The bridge
  must validate range before conversion rather than rely on that coercion.
- [Upstream releases](https://github.com/wasm-bindgen/wasm-bindgen/releases):
  project has releases; 0.2.104 is a compatibility proposal for installed
  tooling, not a claim that it is the latest or security-audited version.

Required browser capabilities: WebAssembly, ES modules, BigInt and local HTTP
module loading. Serve only the experiment directory on loopback. Loading local
assets is a harness concern; the core must not acquire networking or telemetry.
Record the actual browser/runtime and build commands after execution.

## Acceptance cases

Use the host S01–S05 fixture intent. These are browser integration tests at
the exported boundary, derived by equivalence partitioning (match/nonmatch,
empty/nonempty) and boundary-value analysis for integer conversion. The
existing TP-0001 remains the formal full-product test plan; this is a partial
spike proof, not execution of its full acceptance suite.

```gherkin
Feature: Browser execution of the shared Rust spike
  @REQ-0001 @REQ-0003
  Scenario: B01 — typed insert and equality query
    Given a WASM store initialized in a real browser
    When JavaScript inserts pending and complete rows with distinct typed values
    Then querying pending returns exactly the pending rows with unchanged fields
    And the actual rows and result are visible in the console and page

  @REQ-0003
  Scenario: B02 — empty and nonmatching results
    Given a new store and another store containing only complete rows
    When each is queried for pending rows
    Then each query returns no rows

  @REQ-0034
  Scenario: B03 — store isolation and disposal
    Given two independently created WASM stores
    When a pending row is inserted into one store
    Then the other store remains empty
    And both handles can be disposed without an error

  @REQ-0001
  Scenario: B04 — integer precision and range
    Given a store initialized in a real browser
    When JavaScript inserts the signed 64-bit minimum and maximum totals
    Then querying returns those exact totals without precision loss
    And totals one below the minimum and one above the maximum are rejected
    And the store remains unchanged after each rejection

  @REQ-0007 @REQ-0008 @REQ-0036
  Scenario: B05 — browser subscriptions and disposal
    Given a browser store with a pending-row subscription
    When JavaScript drains its initial event then inserts a matching row
    Then the next drain invokes a JavaScript callback with token and count
    And the callback can re-enter the store to read the complete matching row
    And performance.now measures and prints write-to-callback latency
    When the subscription is removed and another row is inserted
    Then a later drain delivers no event for that subscription
```

The bridge exports owned row snapshots and a drain of owned event snapshots;
a JavaScript wrapper invokes callbacks only after the mutable WASM call returns.
This intentionally follows the native poll/drain experiment and does not claim
Rust-initiated push scheduling. JavaScript drains initial events explicitly.
B05 callbacks re-enter only for reads; mutation, disposal and recursive drains from callbacks are not part of
this slice. Unsubscribe/disposal checks occur between drains.
Validate BigInt type/range in the JS insert wrapper, pass decimal text into Rust,
and parse to i64 there as a second boundary check before any mutation. Generated
bindings must never coerce a wide BigInt into i64 first. Snapshot wrappers and
store handles are explicitly freed. Direct generated calls accept decimal text,
not arbitrary JS object values. Browser reload starts a fresh memory store.

Success requires all five browser cases, existing host tests, formatting,
Clippy and the WASM build to pass. Capture red output before implementing
the bridge and green output afterward. A missing server or unrelated compile
error is not sufficient behavioral red evidence. Use a small JavaScript browser
harness with assertions instead of adopting a test framework for this slice.

## Boundary and security review

The new boundary is same-origin synthetic JavaScript data entering a WASM
store. STRIDE review: spoofing/elevation (no identity or privileges exposed),
tampering (typed owned input; reject out-of-range integers), repudiation (no
audit claim), disclosure (synthetic fixtures only, no secrets or persistence),
denial of service (bounded test data; no resource-exhaustion guarantee).
Render values as text, never HTML. Keep ownership explicit and release handles.
No unsafe raw-pointer ABI, remote scripts, authentication, sync or personal
data is introduced. THR-0010 and REQ-0036 track precision and lifecycle risks
and their bounded mitigations. This review does not establish a production security floor.

## Rollback and delivery

Remove the isolated bridge/harness and its generated-output ignores through a
reviewed revert; the existing host core and tests remain intact. There is no
user-data migration or published artifact. Deliver this spec as a separate
feature change through the governed flow; do not merge the host feature or
move integration history without authorization.
Review and commit these cases before bridge tests or implementation.
Dependency approval is now recorded above.
