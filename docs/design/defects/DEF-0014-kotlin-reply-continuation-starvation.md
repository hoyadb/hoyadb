---
id: DEF-0014
title: Inline Kotlin continuation can starve the shared native reply channel
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [REQ-0046, SPEC-0033]
depends_on: [API-0011, TC-0036]
affects: [REQ-0046, API-0011]
regression_for: [TC-0036]
tags: [sdk, kotlin, async, callbacks]
---

# DEF-0014: Inline Kotlin continuation can starve the shared native reply channel

## Symptom and evidence

An actual Android managed observation returns its subscription, but application
code using a plain stdlib continuation then waits for its initial callback. That
callback cannot arrive until the wait times out. The static operation-only trace
in target/phase-seven-kotlin/observation-trace.txt shows observe completing,
drain being submitted, the initial wait failing, and only then drain completing.
Initial command/observation failures are exploratory evidence before this record;
invalid earlier command-name fixtures are not counted as valid command proof.

## Root cause

nativeExchange resumes its caller inline on HoyaNativeTransport's one serialized
reply executor. An EmptyCoroutineContext continuation can execute arbitrary
application code there, blocking delivery of the next owned native reply. Native
storage and core admission are intact; the host continuation occupies the shared
reply channel after its own reply has been collected.

## Intended correction

Decode/copy the bounded reply, then hand SDK continuation resumption to the
existing non-UI managed executor outside the common native/reply channel. Honor
an actual caller ContinuationInterceptor. Keep raw HoyaNativeTransport's callback
contract, shared release broker and core admission unchanged; add no polling or
second admission queue. Do not weaken the genuine awaited-command/initial-event
expectations merely to make the test asynchronous by accident.

## Regression before correction

After reviewed preparation is committed, run a focused actual-Android compiling
RED: a plain stdlib continuation resumes from an SDK call, waits for a later
managed callback, and another owner's native call still progresses. Retain
finite test watchdogs and exact operation-only ordering. Then require initial
observation and validly named suspended command/cancel regressions to pass on
the minimal handoff fix, together with current owner/transport suites. Fake
pending-open fixtures await completion rather than assuming inline callbacks.
No persisted format, dependency or wire change; rollback is source revert.
