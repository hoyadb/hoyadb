---
id: ADR-0010
title: Every command carries a client-generated, time-sortable idempotency key
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, commands, idempotency, reliability]
---

# ADR-0010: Every command carries a client-generated, time-sortable idempotency key

## Context and problem statement

Two independent review passes flagged that nothing distinguished a resent
command (a retry after a lost ack) from a genuinely new one, risking a
double-apply of something like "add to basket." Since the offline queue
retries on its own (`ADR-0016`), this is not an edge case.

## Considered options

- **A client-generated idempotency key on every command**, time-sortable
  (ULID-leaning), that a backend may deduplicate against.
- **A server-assigned request id** handed out before the command is sent.
- **Content hashing** of the envelope to detect a duplicate.
- **No mechanism** — treat double-apply as the backend's problem entirely.

## Decision outcome

In the context of telling a retried command apart from a new one, facing
the double-apply risk an automatically-retrying queue creates, we decided
for **a client-generated, time-sortable idempotency key attached to every
command** and against a server-assigned id, content hashing, and doing
nothing, to achieve a cheap, offline-generable identity for each command
that survives retries, accepting that hoyadb attaches and carries the key
faithfully but does **not** mandate that any backend actually deduplicates
against it.

### Consequences

**Good**
- Generable entirely offline, which a server-assigned id is not — decisive
  given `OBJ-0001`.
- Time-sortable (ULID-leaning) so keys carry useful ordering for debugging
  and for the per-command status array (`ADR-0028`).
- A retry resends the *same* key rather than minting a new one, which is
  what makes resending an already-applied write safe (`REQ-0020`).
- Also used as the correlation field for per-write outcomes in a batched
  response (`ADR-0026`).

**Bad**
- **Deduplication is the backend's choice, not hoyadb's guarantee.** ADR-0010's
  answer explicitly leaves it to "the backend decide to implement it or
  not." So the threat of a double-apply is reduced, not eliminated —
  `THR-0003` records exactly this residual risk.
- Content hashing was not chosen, so two genuinely distinct commands with
  identical arguments are correctly treated as distinct — which is right,
  but means the key is the *only* thing preventing a double-apply.
- The idempotency-key store is the backend's own database, so its retention
  policy is the backend implementer's responsibility; hoyadb only requires
  a key stay valid as long as a client can plausibly stay offline. **Neither
  number is fixed** (`ADR-0028`).
- An entity-diff write carries **no** idempotency key, because
  create/update/delete against a specific `recordId` is naturally
  idempotent to resend. This creates a known correlation gap for two
  queued writes to the same record in one batch (`ADR-0026`).

### Confirmation

Confirmed by `DM-0002` (the envelope carrying the key), `SPEC-0008`, and
the retry/idempotency test cases in `TP-0001`.

## Traceability

- Parent: `OBJ-0003`.
- Referenced by: `SPEC-0008`, `SPEC-0009`, `DM-0002`.
- Mitigates the client-side half of: `THR-0003`.
