---
id: TC-0008
title: Queue state machine, crash recovery, and dependent writes
type: test-case
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [TP-0001]
verifies: [REQ-0018, REQ-0019, NFR-0003]
tags: [test, sync, offline-queue, durability]
---

# TC-0008: Queue state machine, crash recovery, and dependent writes

## Purpose

Verify the queue's states and its persistence across an app kill
(`REQ-0018`), the transactional coupling that `NFR-0003` guarantees on a
transaction-capable engine, and the dependent-write hold plus local-id swap
(`REQ-0019`).

## Coverage boundary, stated

`NFR-0003`'s **non-transactional** engine case is verified only as "no
atomicity is claimed" — there is no scenario asserting atomicity there,
because hoyadb deliberately does not promise it.

**No scenarios exist for**: dependent chains beyond one hop, a write
referencing two unresolved ids, whether dependency edges survive a restart,
or what a database reset does to queued writes. All are named open in
`REQ-0019` and `SPEC-0009` with no decided behaviour to verify.

## Scenarios

See `TC-0008-queue-durability-and-dependents.feature`.

## Traceability

- Parent: `TP-0001`.
- Verifies: `REQ-0018`, `REQ-0019`, `NFR-0003`.
