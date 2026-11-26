---
id: REQ-0011
title: Command handlers run one at a time and commit their writes as one atomic transaction
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-26
parents: [OBJ-0003]
tags: [sync, commands, concurrency, transactions]
---

# REQ-0011: Command handlers run one at a time and commit their writes as one atomic transaction

## Pattern

State-driven, with an Unwanted-behaviour clause against observing partial
state.

## Statement

THE system SHALL permit a command handler to be asynchronous and to both
read from and write to the local store, with both awaited.

WHILE one command handler is running against the local store, THE system
SHALL NOT begin running another command handler against that same database.
An active or queued handler on an independent database SHALL NOT prevent local
writes or handler admission here (ADR-0037). Applications SHALL coordinate their
own invariants spanning databases; no cross-database transaction is supplied.

WHEN a command handler returns, THE system SHALL commit every write it made
as a single, atomic, ordered local transaction, in the order the handler
issued them.

WHILE a command handler is running, THE system SHALL make that handler's
own earlier writes visible to its own later reads (read-your-own-writes),
and SHALL NOT make any of its writes visible to a live subscription, to
other application code, or to another command's handler, until that
transaction commits.

## Examples

Example 1 — read-your-own-writes inside a handler:

```typescript
async handler(db, args, ctx) {
  const priorOrderCount = (await db.query("orders")
    .where("customer_id", "=", args.customerId).fetch()).length;

  const order = await db.table("orders").insert({
    customer_id: args.customerId,
    status: "pending",
    discounted: priorOrderCount === 0,
    placed_at: ctx.now(),
  });
  for (const item of args.items) {
    await db.table("order_items").insert({ order_id: order.id, ... });
  }
  return order;
}
```

Result: the awaited read partway through resolves against current local
state; `order.id` is available two lines after the `insert` that created
it; and the `orders` row plus every `order_items` row commit together when
the handler returns. A subscription observes all of them at once, never the
order without its items.

Example 2 — no interleaving: two commands executed back-to-back run their
handlers strictly one after the other, even though both are `async`. The
second handler's first read sees the first handler's committed writes, or
none of them — never a partial subset.

## Approved scope and remaining limits

ADR-0037 supersedes ADR-0025's application-wide serialization boundary with
per-database FIFO. Async handlers, read-your-writes and atomic commit remain.
Same-database ordinary writes, entity changes and pull apply cannot bypass its
active handler. SDK host queues must use the same boundary. Duplicate ownership
must reject before distinct owners can gain independent mutation admission.

A stuck handler can still block its own database. Explicit cancellation, abort
and owner disposal remain required; the library cannot preempt arbitrary
non-yielding host code. Browser store-scoped admission is enabled only after
its duplicate-owner rejection is demonstrated, retaining the conservative global
fallback otherwise. No cross-tab ownership guarantee is inferred. These are
requirements and limits, not claims that the implementation has passed tests.

## Traceability

- Parent: `OBJ-0003`.
- Under: `ADR-0037` (supersedes the boundary in `ADR-0025`).
- Satisfied by: `SPEC-0008`.
- Verified by: `TC-0005`.
