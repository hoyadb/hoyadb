---
id: TC-0005
title: Optimistic apply and the handler execution model
type: test-case
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [TP-0001]
verifies: [REQ-0010, REQ-0011]
tags: [test, sync, commands, concurrency]
---

# TC-0005: Optimistic apply and the handler execution model

## Purpose

Verify that a command applies locally and immediately (`REQ-0010`), and the
four properties `REQ-0011` guarantees: async handlers with awaited local
reads, one handler at a time, atomic ordered commit, and
read-your-own-writes without leaking partial state outside the handler.

## Not covered, deliberately

**Handler liveness (a handler that never resolves), finer-grained
serialization, and exclusion against concurrent entity-diff or pull-apply
writes have no scenarios** — all three are named as open in `REQ-0011` and
`SPEC-0008`, with no decided behaviour to verify.

## Scenarios

See `TC-0005-command-execution.feature`.

## Traceability

- Parent: `TP-0001`.
- Verifies: `REQ-0010`, `REQ-0011`.
