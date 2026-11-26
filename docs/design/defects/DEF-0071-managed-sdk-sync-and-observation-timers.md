---
id: DEF-0071
title: Swift sync deadline spin and stranded count updates in the managed SDKs
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-10
updated: 2026-11-10
parents: [SPEC-0033]
affects: [API-0011]
regression_for: [TC-0036]
tags: [swift, android, review, regression]
---

# DEF-0071: Swift sync deadline spin and stranded count updates in the managed SDKs

## Symptom

Swift `SyncDriver` kept the previous deadline when `prepare_push` failed, so a failing cycle rescheduled itself with zero delay. In both SDKs the single 16 ms count timer was not re-armed when core answered that the window had not elapsed, stranding the last count of a burst. The Kotlin SDK could discard a new subscription's initial snapshot when a managed drain raced registration. Swift kept `entityContext` and merge hooks across close and reset.

## Acceptance

A failed cycle never reuses an earlier deadline; one follow-up timer is armed when a deadline drain precedes the core window; drain events during registration are buffered for the new listener; close and reset clear captured context and hooks.

## Root cause and fix

The Swift spin was reproduced as 2312 `prepare_push` calls in 300 ms and the stranded count against the real native library; both pass after the change (`swift test`: 106 tests, 0 failures against the existing prebuilt framework). The Kotlin observation corrections were exercised only on a host JVM harness with scripted native calls, not on a device. After `reset`, Swift callers must configure entities again, as Kotlin already required.

## Follow-up

Swift now matches Kotlin for raw `reconcile`/`reconcile_guarded`/`rollback` execution (resolving operations with `onResolution`), continues a managed pass after a failed non-Busy receipt, and waits on a Busy `begin_pull`; API-0011 states each rule. `swift test` reported 114 tests, 0 failures against a framework rebuilt from the then-current sources.
