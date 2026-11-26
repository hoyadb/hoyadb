---
id: SPEC-0009
title: Offline queue — durability, ordering, dependent writes and retry
type: spec
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [ARC-0001, ADR-0016]
satisfies: [REQ-0018, REQ-0019, REQ-0020, NFR-0002, NFR-0003]
depends_on: [DM-0004, SPEC-0001]
tags: [sync, offline-queue, durability]
---

# SPEC-0009: Offline queue — durability, ordering, dependent writes and retry

## Purpose

Specify how commands (`SPEC-0008`) and entity diffs (`SPEC-0010`) survive offline operation and are retried. Recovery behavior is refined by `SPEC-0029` and `SPEC-0031`.

## State machine

```
execute() ──► [queued] ──send──► [sent] ──ack──► [acknowledged] (discarded)
                 ▲                  │
                 └──── retry ───────┘ (no ack received)
```

Entries are persisted under a reserved local table/file name (`REQ-0033`)
so a restart after an app kill recovers from the last known state rather
than losing a write silently. `acknowledged` is **durable receipt only**,
distinct from the authoritative outcome (`SPEC-0008` §4).

## Local durability

Where the engine supports transactions — true for the SQLite-family default
(`SPEC-0002`) — a write's state transition and the local data mutation it
produced **commit together**. There is no window in which the app can be
killed having created the order locally but not recorded the command.

This holds even though a handler is `async` and may await a read partway
through, because hoyadb serializes handler execution and commits its writes
as one transaction when it returns (`SPEC-0008` §3).

Where the engine does **not** support transactions: best-effort state
tracking, **no atomicity guarantee** beyond what that engine allows. An
explicit, accepted limitation (`NFR-0003`).

## Ordering across dependent writes

A locally-created id is swapped for the backend-assigned id once the
authoritative outcome (`applied`/`corrected`) is known — **not** on
acknowledgement, which does not carry the real id. hoyadb maintains a
mapping so dependent writes are corrected too:

```typescript
const order = await addOrderV1.execute({ customerId: "cust_123", items: [] });
// order.id is a local id, e.g. "local:a1b2c3"

await addOrderNoteV1.execute({ orderId: order.id, body: "Gift wrap" });
// Applies locally right away. Stays "queued" — never "sent" — until
// add-order-v1's outcome resolves the real id.
```

**A command is never transmitted while it still references another
command's unresolved local id.** That is what makes rejection simple: a
dependent that was never sent has nothing in flight to cancel and nothing
server-side to undo. If the parent is later `rejected`, its dependents are
rolled back through the same mechanism (`SPEC-0008` §5).

## Retry

A retry resends the **exact same envelope**, idempotency key included —
never a new key. Retries fire automatically when a new command is issued
(the app is evidently connected enough to be writing) and can be requested
explicitly:

```typescript
window.addEventListener("online", () => { db.sync.retryPending(); });
```

Retries use **exponential backoff with jitter**, so a recovering backend is
not hammered by every reconnecting client at once (`SPEC-0015`). Only
still-unacknowledged writes are resent, never a whole previous batch
unconditionally.

## Membership revocation

The queue does **not** inspect a queued write's data against current
membership before sending. Queued writes are sent and processed normally;
the backend's own authorization check rejects what the revoked identity may
no longer write, surfaced through the ordinary rejection path
(`SPEC-0012`).

## Open

The following interface-level questions must be read with the concrete recovery contracts in `SPEC-0029` and `SPEC-0031`:

- **The backlog threshold's default value, and whether it is per-table or
  global**, is not fixed — only that a configurable threshold exists.
- **Dependent-write handling covers exactly one parent→child hop.** A
  transitive chain (A→B→C) and a write referencing *two* unresolved local
  ids are both uncovered.
- **Whether the dependency edges are durably persisted or reconstructed in
  memory is unstated** — which decides whether an app kill mid-chain
  recovers correctly.
- **The detection mechanism** for recognizing an argument as a reference to
  an unresolved local id is not fixed.
- **The whole dependent-write section is scoped to commands only** —
  whether an entity-diff could safely do the same is unaddressed anywhere,
  which forces a logic-free table onto the command path purely for this
  guarantee.
- **A transaction/chain key** for grouping logically-related commands that
  do not reference each other's data remains genuinely open and undesigned.
- **What happens to queued entries when the local database is reset** — by
  a missing migration chain (`SPEC-0005`) or a membership reset
  (`SPEC-0012`) — is unaddressed.

## Traceability

- Parents: `ARC-0001`, `ADR-0016`.
- Satisfies: `REQ-0018`, `REQ-0019`, `REQ-0020`, `NFR-0002`, `NFR-0003`.
- Depends on: `DM-0004`, `SPEC-0001`.
- Verified by: `TC-0008`, `TC-0009`, `TC-0017`.
