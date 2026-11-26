---
id: REQ-0010
title: A command applies optimistically against the local store and is queued for an authoritative re-run
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0001, OBJ-0003]
tags: [sync, commands, optimistic-apply]
---

# REQ-0010: A command applies optimistically against the local store and is queued for an authoritative re-run

## Pattern

Event-driven.

## Statement

WHEN an application executes a named command, THE system SHALL run the
application's own client-side handler for that command against the local
store immediately, make the resulting local changes visible to reads and
subscriptions as soon as the handler completes, and queue the command's
envelope for transmission (`REQ-0018`).

WHEN the backend's authoritative outcome for that command becomes known,
THE system SHALL treat that outcome — not the local result — as
authoritative, and SHALL surface any difference to the application
(`REQ-0017`).

THE system SHALL execute a command's local apply with no network
connection available.

## Examples

Example 1 — placing an order (`legacy/docs/10-walkthrough.md` step 3,
`legacy/docs/06-sync/command-protocol.md`):

```typescript
const order = await addOrderV1.execute({
  customerId: "cust_123",
  items: [{ productId: "p1", quantity: 2 }],
});
```

Result: the client handler inserts an `orders` row and its `order_items`
rows locally; `order.id` is available immediately as a **local** id (e.g.
`local:a1b2c3`) until the backend confirms it (`REQ-0019`). A subscription
on pending orders re-emits right away. The command is queued.

Example 2 — the backend disagrees (`legacy/docs/10-walkthrough.md` step 6): the
backend's independently-authored handler checks stock, which the client
handler could not.

- Enough stock → outcome `applied`, carrying the real order id; the local
  id is swapped for it.
- Not enough stock → outcome `rejected`; the optimistic order and its
  now-orphaned note both roll back (`REQ-0017`).

## Open

Whether the two handler runs actually agree is **explicitly not something
hoyadb guarantees or verifies** (`ADR-0014`, `RISK-0002`). The handler is
the developer's own code on both sides.

## Traceability

- Parents: `OBJ-0001`, `OBJ-0003`.
- Under: `ADR-0005`.
- Satisfied by: `SPEC-0008`, `DM-0002`.
- Verified by: `TC-0005`.
