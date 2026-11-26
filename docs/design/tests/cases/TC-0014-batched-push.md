---
id: TC-0014
title: Array-always push with independent per-write resolution
type: test-case
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [TP-0001]
verifies: [REQ-0029]
tags: [test, sync, transport, batching]
---

# TC-0014: Array-always push with independent per-write resolution

## Purpose

Verify that push is always an array — including for a single write — that
each entry resolves independently with no cross-write atomicity, that
outcomes correlate by key rather than array position, and that a missing
outcome leaves its write retryable (`REQ-0029`).

## Coverage boundary, stated

The known `recordId` collision — two queued entity-diff writes to the same
record in one array — has **no scenario**, because closing it is undesigned
(`ADR-0026`, `SPEC-0010`). It is a design gap, not a test gap.

## Scenarios

See `TC-0014-batched-push.feature`.

## Traceability

- Parent: `TP-0001`.
- Verifies: `REQ-0029`.
