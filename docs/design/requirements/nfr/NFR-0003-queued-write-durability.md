---
id: NFR-0003
title: A queued write survives an app kill, atomically where the engine supports transactions
type: nfr
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0001]
tags: [durability, offline-queue, storage]
---

# NFR-0003: A queued write survives an app kill, atomically where the engine supports transactions

## ISO/IEC 25010:2023 characteristic

Reliability (sub-characteristics: Fault tolerance, Recoverability).

## Planguage

- **Tag**: queued-write-durability
- **Ambition**: Killing the app mid-write never silently loses a user's
  write, and never leaves the local data and the queue disagreeing about
  whether it happened.
- **Scale**: Number of queued writes lost, or left inconsistent with the
  local data mutation they produced, after an app kill at an arbitrary
  point during the write.
- **Meter**: The kill-and-restart cases in `TP-0001`, which interrupt at
  each state boundary — after the local mutation, after the state
  transition, and between them — then reopen the database and inspect
  recovered queue state (`REQ-0018`).
- **Past**: n/a — new capability.
- **Must**, on an engine that supports transactions (the SQLite-family
  default, `SPEC-0002`): **0** lost, **0** inconsistent. The state
  transition and the local data mutation commit in one transaction, so
  there is no window in which one happened without the other.
- **Must**, on an engine that does not support transactions: **0** lost
  where the engine's own guarantees permit detection; **no atomicity claim
  is made**, and inconsistency between the mutation and the state
  transition is possible. This is an explicit, accepted limitation
  (`ADR-0016`), not an unmet bar.
- **Target**: same as Must. Raising the non-transactional case would
  require a guarantee the engine cannot back, which hoyadb declines to
  fake.

## Rationale

`ADR-0003` makes the concrete engine pluggable per platform, so a uniform
durability guarantee across all engines is not available. ADR-0016's own answer
is explicit: "if the engine backing a given platform supports transactions,
we wrap the command's state transition and its local data mutation in one
... If a platform's engine doesn't support that, we don't fake a guarantee
we can't back."

## Open

Beyond this queue-specific guarantee, hoyadb commits to **no** additional
durability guarantees — no fsync policy, no broader crash-safety bound
(`ADR-0024`). Ordinary local writes outside a command's transaction carry
only whatever the underlying engine provides, unstated by hoyadb.

What happens to queued writes when the local database is reset — by a
missing migration chain or a membership reset — is **unaddressed**
(`REQ-0006`, `REQ-0027`).

## Traceability

- Parent: `OBJ-0001`.
- Under: `ADR-0016`, `ADR-0024`.
- Satisfied by: `SPEC-0009`, `SPEC-0001`.
- Verified by: `TC-0008`.
