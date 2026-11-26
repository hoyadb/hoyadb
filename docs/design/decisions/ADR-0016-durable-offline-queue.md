---
id: ADR-0016
title: Offline writes go through a durable, state-tracked, locally-persisted queue
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0001]
tags: [sync, offline-queue, durability]
---

# ADR-0016: Offline writes go through a durable, state-tracked, locally-persisted queue

Source: `legacy/PLAN_DECISIONS.md` D8 — described there as "the single largest
open cluster in the whole plan" — folded into `legacy/VISION.md`'s "Sync — offline
queue" and `legacy/docs/06-sync/offline-queue.md`. Genuinely new ground: nothing
in `legacy/VISION.md` or `legacy/refined-vision/` designed this, despite "optimistic
local apply" implying a queue exists.

## Context and problem statement

`OBJ-0001` requires writes to work with no connection. That implies
somewhere to hold them, a retry policy, a durability story across app
kills, and an answer for writes that depend on an id the backend has not
assigned yet.

## Considered options

- **A durable, ordered local queue under a reserved table/file name**, with
  explicit states (queued → sent → acknowledged), transactional coupling to
  the local mutation where the engine supports it, id-mapping for dependent
  writes, and app-visible introspection.
- **An in-memory queue only**, accepting loss on app kill.
- **No queue** — fail the write when offline and let the app hold it.

## Decision outcome

In the context of surviving being offline between a write and its
acknowledgement, facing app kills, dependent writes and unbounded backlog,
we decided for **a durable, locally-persisted, state-tracked queue** and
against an in-memory-only queue and against failing offline writes, to
achieve recovery from the last known state after a restart rather than
silent loss, accepting that its atomicity guarantee is only as strong as
the underlying storage engine's transaction support.

### Consequences

**Good**
- Explicit states mean a restart inspects the last known state and recovers
  rather than losing a command silently.
- Where the engine supports transactions — true for the SQLite-family
  default (`ADR-0003`) — a command's state transition and its local data
  mutation commit together, so there is no window between "the order was
  created locally" and "the create-order command was durably recorded."
- A locally-created id is swapped for the backend-assigned id once the
  authoritative outcome (`applied`/`corrected`) is known — not merely on
  acknowledgement, which does not carry the real id — with a mapping so
  dependent writes get corrected too.
- A command referencing another command's unresolved local id is never
  transmitted, which is what makes rejection-cascade rollback simple: a
  dependent that was never sent has nothing in flight to cancel.
- Retries fire automatically when a new command is issued and can be
  explicitly requested by the developer; hoyadb may block further writes
  past a configurable backlog threshold rather than growing unbounded.

**Bad**
- **A platform whose engine lacks transactions gets best-effort state
  tracking with no atomicity guarantee beyond what that engine allows.** An
  explicit, accepted limitation — hoyadb does not fake a property the
  engine cannot back (`NFR-0003`).
- **Dependent-write ordering is worked through for exactly one parent→child
  hop and does not generalize.** A chain (A→B→C, where C is blocked
  transitively via B) and a command referencing *two* unresolved local ids
  at once are both uncovered. Neither is designed.
- Whether the dependency edges are durably persisted or reconstructed in
  memory at runtime is **unstated** — which matters for whether an
  app-kill mid-chain recovers correctly.
- The mechanism for detecting that an argument value *is* a reference to an
  unresolved local id (deep-scanning args vs. an explicit declaration) is
  **not fixed**.
- **The whole dependent-write section is scoped to commands only.** Whether
  an entity-diff write could ever safely reference an unresolved local id
  is unaddressed anywhere in the tree — which forces a table with no
  business logic onto the command path purely for the ordering guarantee
  (`legacy/docs/10-walkthrough.md` step 4).
- The backlog threshold's default value, and whether it is per-table or
  global, are **not fixed**.
- A **transaction/chain key** for grouping logically-related commands that
  do not reference each other's data remains genuinely open and undesigned.

### Confirmation

Confirmed by `SPEC-0009` (the queue), `DM-0004` (the persisted entry), and
the offline/restart/dependent-write test cases in `TP-0001`. The
uncovered cases above have no test cases because they have no decided
behaviour to verify.

## Traceability

- Parent: `OBJ-0001`.
- Referenced by: `SPEC-0009`, `SPEC-0013`, `SPEC-0015`, `DM-0004`.
- Depends on: `ADR-0010` (idempotency key reused on retry), `ADR-0011`
  (ack semantics), `ADR-0003` (engine transaction support).
