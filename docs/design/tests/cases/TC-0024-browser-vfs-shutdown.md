---
id: TC-0024
title: Browser VFS releases pool handles before namespace deletion
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [TP-0001]
verifies: [REQ-0039]
regression_for: [DEF-0001]
depends_on: [SPEC-0024]
tags: [browser, lifecycle, regression]
---

# TC-0024: Explicit VFS release

```gherkin
@REQ-0039
Feature: Acknowledged shutdown releases the test worker's pool
Scenario: L01 a live adapter prevents shutdown
  Given a real worker owns committed OPFS rows through one live adapter
  When shutdown is requested
  Then BackendFailure is returned without releasing that adapter or changing its rows
  And subsequent reads return the same complete rows
Scenario: L02 VFS handles close before worker termination
  Given the adapter is explicitly disposed but the worker and VFS remain alive
  When deletion of that worker's unique test namespace is attempted
  Then NoModificationAllowedError demonstrates the pool still owns sync handles
  When shutdown is requested and acknowledged
  Then stale reads and reinitialization are rejected
  And deletion of only that namespace succeeds while the same worker is still alive
  And only then is the worker terminated
```

This deterministic regression targets pool ownership rather than depending on
an intermittent worker-termination race. The first delete is restricted to the
fresh synthetic namespace; the actual browser refuses it while handles are
held. No required assertion is skipped if the browser behaves differently.
Full TC-0023 data/rollback/reload assertions remain required before cleanup.

Executed RED in commit 5ad3967, then GREEN on Chrome 152 / Electron 44.2.0 /
Paseo 0.12.0 at secure loopback port 8767 without cross-origin isolation.
Complete typed rows and lifecycle results are recorded in
[the browser report](../../../implementation/browser-persistence.md).
