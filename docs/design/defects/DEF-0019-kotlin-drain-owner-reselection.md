---
id: DEF-0019
title: A retired Kotlin managed drain can select the replacement owner
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [REQ-0046, SPEC-0033]
depends_on: [API-0011, TC-0036]
affects: [REQ-0046, SPEC-0033, API-0011, THR-0020]
regression_for: [TC-0036]
tags: [phase-seven, kotlin, observations, lifecycle]
---

# DEF-0019: A retired Kotlin managed drain can select the replacement owner

## Source observation and intake boundary

The uncommitted Android SDK's ManagedObservations.drain captures an owner/epoch
pair, then evaluates the injected uptime callback, then invokes Database.request.
That request independently selects the current owner. Closing and reopening the
same Database while the old clock callback is paused can therefore make the old
drain select the replacement native handle. The old listener state is already
closed, so its delivery path can discard the replacement owner's initial events
after the core has transferred them into the owned drain reply.

This is a source-verified concurrency candidate found during the benchmark quiet
window; no controlled Android/runtime reproduction has executed yet. No customer
data, external disclosure or measured failure rate is claimed. The injected
clock gives a deterministic test seam after old-target capture and before the
request's owner reselection. Existing initial-observer GREEN predates this new
regression and does not cover the close/reopen interleaving.

## Intended correction

Use one internal owned-request primitive that checks the expected owner epoch and
captures its handle atomically. Submit only that captured handle, never reselect
a replacement owner after a separate liveness check; recheck the epoch before
publishing the reply. A native attempt queued after the old owner is closed may
reject on that old handle, but must not reach the new owner. Apply the same
primitive to existing Busy-retained owned operations so their arguments cannot
move to a replacement owner. Keep normal advanced execute semantics, the core's
admission authority, raw transport and selected callback executor unchanged.
No additional host admission queue, polling scheduler, dependency or persistent
format change. Rollback is a source revert; no customer store is modified.

## Regression before correction

After this preparation is reviewed and committed, run a compiling actual Android
regression with a real memory owner and injected clock gate. Pause the old managed
drain after target capture, close/reopen, register a replacement subscription,
release the clock and await the retired job. Assert the new initial snapshot is
still delivered exactly once without another write or refresh, no old listener
runs and no new-owner event was silently consumed. A raw replacement subscription
may be used to independently inspect the real owned event queue, avoiding a fake
event producer. Test-only bounded waits are operational watchdogs, not a new SDK
latency promise. Preserve an observed RED before the request-primitive change.
Require current owner, pending-open, callback, command/FIFO, entity/passive and
transport suites to remain GREEN after the minimal fix.
