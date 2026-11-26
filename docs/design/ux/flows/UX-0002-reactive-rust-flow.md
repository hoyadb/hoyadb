---
id: UX-0002
title: Rust query observation lifecycle and error handling
type: ux
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [ARC-0001]
satisfies: [REQ-0007, REQ-0008, REQ-0009, REQ-0042]
depends_on: [API-0005, SPEC-0027]
tags: [reactivity, phase-three]
---

# UX-0002: Observe and deliver local queries

Developer opens API-0004 Database, converts into_reactive, builds one query,
fetches once or awaits observe/observe_count/observe_with_columns, and retains
its Subscription. A host drain delivers Notification values on its own chosen
thread/runtime. Empty is Rows([]) or Count(0); initial is queued immediately,
subsequent successes follow committed changes. No network/loading UI exists.
Invalid declarations return typed registration errors; post-commit observation
failure is Notification Err, with explicit refresh_notifications for recovery.
Unsubscribe returns whether a live registration existed; final cancellation
releases its group. into_storage/drop closes all observations. Count requires
host scheduling at 16 ms intervals while pending. Consumer wrappers map owned
events to callbacks and check liveness for every delivery. Callback failures
are isolated and surfaced locally, never relabeled as failed committed writes.
No visual copy, CLI or accessibility surface is introduced; consuming UI owns
WCAG conformance. Error variants/handling are API-0005, diagnostics unstable.
TC-0029 tests every described state, callback cancellation and recovery.
