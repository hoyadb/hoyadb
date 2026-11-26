---
id: REQ-0018
title: Queued writes are persisted locally and tracked through explicit states across restarts
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0001]
tags: [sync, offline-queue, durability]
---

# REQ-0018: Queued writes are persisted locally and tracked through explicit states across restarts

## Pattern

State-driven, with an Optional-feature clause for transactional engines.

## Statement

THE system SHALL persist every queued write — command and entity-diff alike
— under a reserved local table or file name (`REQ-0033`), and SHALL track
it through the explicit states `queued`, `sent`, and `acknowledged`,
discarding it from the queue on acknowledgement.

WHILE the application is restarting after being killed, THE system SHALL
recover each queued write from its last persisted state rather than losing
it silently.

WHERE the local storage engine backing a platform supports transactions,
THE system SHALL commit a write's state transition and the local data
mutation it produced in the same transaction.

WHERE the engine does not support transactions, THE system SHALL track
state as precisely as that engine allows and SHALL NOT claim an atomicity
guarantee that engine cannot provide.

## Examples

Example 1 — the state machine (`legacy/docs/06-sync/offline-queue.md`):

```
execute() ──► [queued] ──send──► [sent] ──ack──► [acknowledged] (discarded)
                 ▲                  │
                 └──── retry ───────┘ (no ack received)
```

Example 2 — atomic coupling on the default engine: placing an order writes
the `orders` row and records `add-order-v1` as `queued` in one transaction.
There is no window in which the app can be killed having created the order
locally but not recorded the command — both happen, or neither does.

Example 3 — a non-transactional engine: the same sequence is tracked
best-effort, and hoyadb states that the atomicity guarantee does not hold
there (`NFR-0003`). This is an accepted limitation, not a gap left open.

## Open

`acknowledged` is durable receipt only, explicitly **not** the backend's
authoritative outcome (`REQ-0016`). What happens to still-unacknowledged
queued writes when the local database is reset — by a missing migration
chain (`REQ-0006`) or a membership reset (`REQ-0027`) — is **unaddressed
anywhere in the doc tree**. A real, disclosed gap.

## Traceability

- Parent: `OBJ-0001`.
- Under: `ADR-0016`.
- Satisfied by: `SPEC-0009`, `DM-0004`.
- Verified by: `TC-0008`.
