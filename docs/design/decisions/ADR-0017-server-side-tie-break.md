---
id: ADR-0017
title: Identical-timestamp conflicts are broken server-side — first received wins
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, conflict-resolution]
---

# ADR-0017: Identical-timestamp conflicts are broken server-side — first received wins

## Context and problem statement

`ADR-0007`'s per-field last-write-wins default has no defined behaviour
when two writes to the same field carry an identical timestamp. Client
clocks are neither synchronized nor trusted as the tie-break authority.

## Considered options

- **Break the tie server-side by real arrival order** — the first write the
  backend receives wins, the second is ignored.
- **Use a secondary sortable id** (e.g. the idempotency key's ULID ordering)
  as the tie-break.
- **Trust the client timestamp** with higher precision.

## Decision outcome

In the context of two writes to one field carrying the same timestamp,
facing the fact that only the backend observes real arrival order, we
decided for **server-side tie-break by arrival order, first received wins**
and against a secondary sortable id and against higher-precision client
timestamps, to achieve a tie-break grounded in something actually
observable rather than in an untrusted client clock, accepting that hoyadb
can only document this — it has no server-side component of its own to
enforce it in.

### Consequences

**Good**
- The tie-break lives where the information is. `clientTimestamp` travels
  in the entity-diff payload for LWW comparison but is explicitly *not*
  trusted as the tie-break authority.
- A secondary sortable id would have reintroduced the same client-clock
  trust problem, since the client mints it.

**Bad**
- **This is a backend-implementation recommendation hoyadb documents, not a
  mechanism hoyadb runs.** hoyadb has no server-side component. If a
  backend implements it incorrectly, hoyadb cannot detect that.
- Backend-side transactions are *recommended* to prevent a double-apply
  race under true concurrency (two writes arriving close enough that a
  naive read-then-write applies both). Recommended, not enforceable.
- "First received wins" means the second write is silently ignored — the
  same category of loss `ADR-0007` already accepts for per-field LWW.

### Confirmation

Confirmed by `SPEC-0011` and the conflict test cases in `TP-0001`, which
verify the client-side half (the payload carries `clientTimestamp` and does
not attempt a local tie-break). The server-side half is verifiable only in
a backend implementation, which is outside hoyadb.

## Traceability

- Parent: `OBJ-0003`.
- Refines: `ADR-0007`.
- Referenced by: `SPEC-0011`, `API-0001`.
