---
id: DEF-0011
title: Late sync replies can reach replacement owner transport or mapper
type: defect
status: active
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [REQ-0046, SPEC-0033]
depends_on: [API-0011, TC-0036, THR-0020]
affects: [REQ-0046, API-0011, THR-0020]
regression_for: [TC-0036]
tags: [sdk, lifecycle, sync, stale-callback]
---

# DEF-0011: Late sync replies can reach replacement owner transport or mapper

## Suspected cause

resumeOwned checks owner/generation before the operation but returns its awaited
value without rechecking afterward. The direct pending-list await has no guard
before invoking an application mapper. Neither finding implies rollback of an
already submitted durable operation; only stale host publication is at issue.

## Intended correction and acceptance

After a successful owned operation resolves, suppress its result if the captured
owner/generation is no longer current. Check liveness after awaiting a pending
list and before each application mapper/transport side effect. Preserve exact
Busy retry bytes, submitted durable effects and current-owner behavior. No
transport or mapper from the replacement owner may receive old work.

TC-0036 G09 gains compiling delayed-RPC close/reopen regressions before a fix:
hold prepare_push then replace owner/transport, and independently hold pending
reconciliation list then replace owner/mapper. Assert zero calls to replacement
callbacks from the old run, current work still runs, and old cleanup cannot
cancel or mutate the new run. Observe failures for this precise reason before
implementation. Existing transport-response and merge-hook stale tests remain.
Rollback is a code revert; no persisted or wire format changes.

## Late rejection publication contract

The same stale-publication contract also covers a delayed RPC rejection: an old
run must not append errors to replacement-owner sync state. Before extending a
fix, reproduce delayed prepare and pending-list rejection across close/reopen;
assert no stale error publication while a current-owner error remains visible.
Both rejections were reproduced: replacement-owner syncErrors received the old
BackendFailure. The first probe preceded follow-up prep429ea25 and is labelled
exploratory. After that commit, removing only the new live catch guard reproduced
the same two failures; restoring it passed all four stale cases. Current-owner
Protocol errors remain visible. No submitted durable effect is undone.

## Current correction and remaining verification

resumeOwned rechecks captured owner/generation after a successful await. sync
checks the pending list before each mapper and only publishes errors while its
owner/generation remains live. Replacement syncRun/syncing/wake flags and current
receive/reconcile/error behavior are pinned by the regressions. The full portable
JavaScript workspace passes82 tests; matching packaged/browser runtime evidence
is pending this checkpoint. The defect remains active until that verification.
