---
id: TC-0007
title: Command outcomes, corrections and rollback
type: test-case
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [TP-0001]
verifies: [REQ-0016, REQ-0017]
tags: [test, sync, commands, error-handling]
---

# TC-0007: Command outcomes, corrections and rollback

## Purpose

Verify the transport-agnostic outcome enum and that acknowledgement is
tracked separately from acceptance (`REQ-0016`), and that a correction or
rejection reaches the app through its ordinary subscriptions with a working
rollback that cascades to blocked dependents (`REQ-0017`).

## Scenarios

See `TC-0007-outcomes-and-rollback.feature`.

## Traceability

- Parent: `TP-0001`.
- Verifies: `REQ-0016`, `REQ-0017`.
