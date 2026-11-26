---
id: REQ-0019
title: A write referencing an unresolved local id stays queued until the backend id is known
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0001]
tags: [sync, offline-queue, ordering]
---

# REQ-0019: A write referencing an unresolved local id stays queued until the backend id is known

## Pattern

State-driven, with an Event-driven clause for the id swap.

## Statement

WHILE a queued command references another command's not-yet-resolved local
id, THE system SHALL keep it in the `queued` state and SHALL NOT transmit
it.

WHEN a command's authoritative outcome (`applied` or `corrected`) becomes
known, THE system SHALL swap the locally-generated id it produced for the
backend-assigned id, and SHALL maintain a mapping so that queued writes
referencing the local id are corrected to the real one.

THE system SHALL NOT perform the swap on acknowledgement alone, since
acknowledgement does not carry the backend-assigned id (`REQ-0016`).

## Examples

Example 1 — the dependent pair:

```typescript
const order = await addOrderV1.execute({ customerId: "cust_123", items: [] });
// order.id is a local id, e.g. "local:a1b2c3"

await addOrderNoteV1.execute({ orderId: order.id, body: "Gift wrap" });
// Applies locally right away. Stays "queued" — never "sent" — until
// add-order-v1's outcome resolves the real id.
```

Result: only the order is eligible to send. When its outcome is `applied`
with `{ id: "order_456" }`, the note's `orderId` argument is corrected to
`order_456` and the note becomes eligible to send.

Example 2 — rejected parent: if `add-order-v1` is instead `rejected`, the
note is rolled back rather than left pointing at an id that will never
resolve (`REQ-0017`). Because it was never sent, there is nothing in flight
to cancel and nothing server-side to undo.

## Open

Several real gaps, **none resolved** (`ADR-0016`):

- **Only one parent-to-child hop is designed.** A chain A→B→C, where C is
  blocked transitively through B, is not covered; nor is a command
  referencing *two* different unresolved local ids at once.
- Whether the dependency edges are **durably persisted or reconstructed in
  memory** is unstated — which decides whether an app-kill mid-chain
  recovers correctly.
- The **detection mechanism** for recognizing that an argument value is a
  reference to an unresolved local id (deep-scanning args versus an explicit
  declaration) is not fixed.
- **This is scoped to commands only.** Whether an entity-diff write could
  safely reference an unresolved local id is unaddressed anywhere — which
  forces a table with no business logic onto the command path purely for
  this ordering guarantee.

## Traceability

- Parent: `OBJ-0001`.
- Under: `ADR-0016`.
- Satisfied by: `SPEC-0009`, `DM-0004`.
- Verified by: `TC-0008`.
