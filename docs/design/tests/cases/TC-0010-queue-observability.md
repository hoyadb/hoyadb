---
id: TC-0010
title: Queue introspection, cancellation and the backlog threshold
type: test-case
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [TP-0001]
verifies: [REQ-0021, REQ-0022, THR-0006]
tags: [test, observability, offline-queue]
---

# TC-0010: Queue introspection, cancellation and the backlog threshold

## Purpose

Verify that aggregate and per-command state come from one call, that
`blockedOn` distinguishes a dependency-blocked write from an ordinary queued
one — the concrete gap `REQ-0021` closes — that cancellation is explicit and
cascades (`REQ-0022`), and that the backlog threshold bounds the queue
(`THR-0006`).

## Coverage boundary, stated

**Cancelling a `sent`-but-unacknowledged command is a real, unclosed race**
(`REQ-0022`). The scenario below asserts the best-effort behaviour hoyadb
claims, **not** that the backend is prevented from applying it.

## Scenarios

See `TC-0010-queue-observability.feature`.

## Traceability

- Parent: `TP-0001`.
- Verifies: `REQ-0021`, `REQ-0022`, `THR-0006`.
