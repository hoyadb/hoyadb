---
id: DEF-0015
title: External wire receipt can leave a sleeping sync mapper pending
type: defect
status: active
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [REQ-0046, SPEC-0033]
depends_on: [API-0011, TC-0036]
affects: [REQ-0046, API-0011]
regression_for: [TC-0036]
tags: [phase-seven, javascript, scheduler, sync]
---

# DEF-0015: External wire receipt can leave a sleeping sync mapper pending

## Symptom and exploratory evidence

An actual WASM memory owner queued a command and started the SDK sync driver with
an application mapper and a transport returning an empty reply. Configured retry
delay60000ms kept the durable Sent command in a future retry window. The probe
first verified that the driver had settled: exactly2 prepare calls,1 push,
0 mapper calls and syncingfalse, with the retry timer scheduled. A valid Applied
wire outcome was then received through public receiveWire. After100ms it still
had exactly2 prepare calls,0 mapper calls and1 pending reconciliation. No network
success, later mutation or manual drain was used to advance the mapper.

The exploratory source/output are
`target/phase-seven-web/probe-receipt-wake-settled.{mjs,txt}`. This probe preceded
the defect preparation commit and is not a permanent post-preparation RED.
An earlier probe (`probe-receipt-wake.{mjs,txt}`) was confounded by an already
scheduled follow-up check and did invoke the mapper; retain it as unsuccessful
isolation, not evidence against the settled result.

## Root cause

API-0011 section7 requires the host scheduler to wake on receipt. JavaScript
receiveWire forwards through the notification-only helper, so it can commit and
deliver managed notifications without signalling wakeSync. A response handled by
the driver itself sets syncWakePending later in sync; an external public receipt
has no such surrounding step. Its local mapper consequently waits for an
unrelated wake or the old future retry timer. The core receipt and pending
reconciliation are intact.

## Intended correction and rollback

After a successful external wire-receipt operation, signal the existing sync
wake mechanism for the same captured owner/generation. Keep preparePush
notification-only so an empty due check cannot wake itself repeatedly. Place
notification and wake under the captured lifetime guard; a receipt resolving
after close/reset must not wake a replacement owner (DEF-0011). Do not resend
transport bytes, rerun a mapper for Busy, replace retry policy or mutate core
queue/deadline semantics. No ABI, persistence or dependency change. Rollback is a
code revert.

## Regression owed before implementation

TC-0036 G19 requires a compiling behavioral RED after this preparation is
reviewed and committed: start the real SDK/core driver, establish its sleeping
state with a future retry deadline, supply an external wire receipt, and await
local mapper/reconcile completion without waiting for that deadline or using an
additional external wake. Assert one original push, one mapper invocation, exact
local outcome, no early resend and no prepare self-wake loop. Preserve the
existing late-owner guard and current-owner behavior. The latest packaged
83-test/35-browser snapshot predates this correction and must remain historical.

## Resolution evidence

After preparation commit94bc84b, the permanent real WASM/core memory case failed
with `external receipt did not wake local reconcile before future retry`.
The helper now captures owner/generation and signals the existing wake only for
a successful matching `receive_wire`; prepare remains notification-only.
The same case passes in real memory and protected OPFS, and a controlled delayed
receipt cannot wake or drain a replacement owner. Matching verification passed
84 Node tests,16 memory groups,16 walkthrough cases,36 protected browser groups
across reload,9 security cases and1 initializer retry case. Full evidence and
source/artifact closure are in
[the implementation report](../../implementation/phase-seven-web-receipt-wake.md).
