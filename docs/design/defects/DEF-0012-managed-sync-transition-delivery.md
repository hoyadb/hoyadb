---
id: DEF-0012
title: Managed sync transitions can leave notifications undelivered
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [REQ-0046, SPEC-0033]
depends_on: [API-0011, TC-0036]
affects: [REQ-0046, API-0011]
regression_for: [TC-0036]
tags: [phase-seven, swift, observations, sync]
---

# DEF-0012: Managed sync transitions can leave notifications undelivered

## Symptom

An actual memory-backed Swift owner commits a command, registers a managed outcome
subscription and receives its initial Queued event. Calling preparePush commits
Sent in Rust, but the callback receives no Sent event until another operation
drains the stream. The exploratory compiling regression timed out waiting for
Sent, without a receipt, another write or manual drain. Its original evidence is
preserved at target/phase-seven-native/swift-prepare-observe-red.txt; this probe
preceded this defect record and is not presented as a post-preparation RED run.

The JavaScript agent independently reproduced the same preparePush symptom on
the actual WASM facade/memory engine: initial Queued, still Queued after prepare
and a100ms wait, then Sent immediately after explicit drain. Its exploratory
source/output are target/phase-seven-web/probe-prepare-observation.{mjs,txt}.
JavaScript receiveWire is source-suspected to omit managed delivery scheduling
but has not been reproduced at intake. It requires independent behavioral RED
before any correction.

## Root cause

Swift Database.execute schedules managed delivery for an explicit operation set.
prepare_push is absent although its queue state changes are observable. The
public preparePush wrapper forwards through execute and consequently never
schedules the resulting event. The core transition and event queue are intact. JavaScript preparePush and
receiveWire call the raw channel rather than the managed notification path;
actual prepare behavior is reproduced, while wire receipt delivery remains
a hypothesis awaiting its own regression.

## Intended correction

Schedule managed observation delivery after a successful prepare_push call.
If the wire-receipt regression confirms the same gap, schedule its committed
notifications through the same managed path.
Keep it separate from waking the sync driver: the driver's own due-queue check
must not repeatedly schedule itself. Preserve opaque envelopes, due deadlines,
explicit raw drain ownership and existing receipt/mapper behavior. No persisted
format, wire grammar or dependency changes are required; rollback is a code
revert.

## Regression and verification plan

TC-0036 G05 now explicitly covers Queued initial delivery followed by preparePush
and automatic Sent delivery with no other operation. After this preparation is
reviewed and committed, remove only the exploratory trigger fix and observe the
same behavioral failure, then restore the minimal correction and run all Swift
sync cases, strict formatting and the full SDK suite. Existing managed sync retry
cases guard against duplicate transport/mapper calls and scheduler self-wakes.
