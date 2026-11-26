---
id: UX-0001
title: Developer integration flow — schema, subscribe, mutate, handle correction
type: ux
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [ARC-0001]
satisfies: [REQ-0010, REQ-0017]
depends_on: [SPEC-0008, SPEC-0009]
tags: [ux, developer-experience, sdk]
---

# UX-0001: Developer integration flow — schema, subscribe, mutate, handle correction

## Note on applying the UX-flow template to a library

hoyadb is a library/SDK with no end-user-facing surface of its own, so the
"user" walked through here is the **integrating developer**, and the states
are the states of their integration — not screens.

There is precedent for this reading inside the framework itself:
`ai-governance/example/link-shortener/ux/flows/UX-0001-cli.md` applies the
same template to a command-line tool, translating WCAG into
terminal-appropriate equivalents. This document does the same for an SDK.

**Where the template fits poorly, this document says so rather than forcing
it** — see "Where this template does not transfer" at the end. The parts
that do transfer well are the state walkthrough (including the
error/partial/success states that are easy to leave undesigned) and the
error-copy table; those are the substance here.

### 1. Declare the schema

The developer declares tables, typed columns and relations
(`REQ-0001`, `REQ-0002`, `SPEC-0003`). Using the walkthrough's schema:
`customers`, `orders`, `order_items`, `products`, `order_notes`, with a
customer having many orders, an order belonging to one customer and having
many items.

**State:** nothing is live yet; this is a pure declaration.

### 2. Read and watch

Before any data exists, the developer already knows how to read it
(`REQ-0007`, `SPEC-0006`):

```typescript
const unsubscribe = db
  .query("orders")
  .where("customer.country", "=", "FR")
  .orderBy("placed_at", "desc")
  .observe((orders) => renderOrdersList(orders));
```

**States the developer must handle here:**

| State | What they see | Designed? |
| --- | --- | --- |
| Empty | The callback fires with an empty array | Yes |
| Populated | The callback fires with results, re-firing on set-membership change (`REQ-0008`) | Yes |
| **Loading / first sync in flight** | **Undefined** — a subscription cannot distinguish "no matching orders" from "orders haven't synced yet" | **No — `RISK-0005`** |

### 3. Write a command, applied optimistically

`orders` has real domain logic (a first-order discount), so it warrants a
command rather than a plain write (`REQ-0010`):

```typescript
const order = await addOrderV1.execute({
  customerId: "cust_123",
  items: [{ productId: "p1", quantity: 2 }],
});
// order.id is available immediately — a LOCAL id until the backend confirms
```

**State:** the subscription from step 2 re-emits right away. The command is
`queued`.

### 4. Reference it before it is confirmed

The developer adds a note to the order just created, referencing its
still-local id (`REQ-0019`):

```typescript
await addOrderNoteV1.execute({ orderId: order.id, body: "Gift wrap" });
```

**State:** applies locally immediately, but stays `queued` — never `sent` —
until the parent's outcome resolves the real id.

**A rough edge this flow surfaces, worth naming rather than hiding:**
`add-order-note-v1` has no real business logic of its own. It is a command
here *purely* to get the ordering guarantee, because that protection is
specified only for commands. Whether an entity-diff could safely do the same
is unaddressed anywhere (`SPEC-0010`). The developer pays ceremony they
would not otherwise need.

### 5. Over the wire

Only the order is eligible to send. It goes out as an array of one — the
ordinary case, not a special one (`REQ-0029`, `API-0001`).

### 6. Handle the outcome — the state that matters most

The backend independently checks stock, which the client handler could not:

| Outcome | What the developer must do |
| --- | --- |
| `applied` | Accept it. The local id is swapped for the real one; the note command unblocks and sends. |
| `corrected` | Reconcile — e.g. the backend applied a discount rule the client did not know about. |
| `rejected` | Roll back. The optimistic order **and its now-orphaned note** roll back together. |

```typescript
useManageRollback("orders", order.id, (rollback, correction) => {
  if (correction.status === "corrected") {
    showToast(`Order total adjusted to ${correction.result.total_cents}`);
  } else if (correction.status === "rejected") {
    rollback();
    showToast(`Order could not be placed: ${correction.reason}`);
  }
});
```

This hook is **an example for one binding**, not a shape every binding must
copy (`REQ-0017`). **hoyadb provides the mechanism, never the recovery
policy** — what the app actually does is the developer's decision, and this
is the single step where that division of responsibility bites hardest.

### 7. A plain write

Editing the note's text later has no business logic worth a command, so the
entity-diff path applies (`REQ-0023`) — same queue, same authorization bar,
resolved by per-field last-write-wins instead of a handler re-run.

### 8. Debug it

When a support ticket says "my gift-wrapped order never showed up", the
developer checks per-command status (`REQ-0021`): was the order ever
acknowledged, is it `rejected`, or is the note still `queued` waiting on a
parent that never resolved?

**This works only if the ticket arrives soon after the fact.** A resolved
entry is retained for a bounded window (**whose length is not fixed**)
before ageing out, after which an empty result cannot be told apart from
"this command never ran" (`SPEC-0015`).

## Error copy the developer surfaces

Derived from the outcomes hoyadb actually produces; the wording is
illustrative, the conditions are not:

| Condition | Source | Developer-facing handling |
| --- | --- | --- |
| Command `rejected` | `REQ-0017` | Roll back, show `correction.reason` |
| Command `corrected` | `REQ-0017` | Accept the backend's result, surface the difference |
| Write blocked on an unresolved parent | `REQ-0021` | Show as pending, not failed — `blockedOn` distinguishes it |
| Backlog threshold exceeded | `REQ-0021` | Local writes rejected; surface a sync-blocked state |
| Entity-diff rejected by authorization | `REQ-0023` | Roll back; the identity is no longer entitled to that record |

## Where this template does not transfer

Stated plainly rather than filled in with invented content:

- **WCAG 2.2 AA criteria do not apply.** hoyadb renders nothing. The
  accessibility of anything built on it is entirely the consuming
  application's responsibility, and hoyadb imposes no constraint that would
  help or hinder it. The link-shortener example could translate WCAG into
  terminal equivalents (colour, `NO_COLOR`, exit codes) because a CLI has an
  output surface; an SDK does not.
- **`CMP` (UI component contract) and `TOK` (design tokens) have no
  instances in this project**, for the same reason — there are no UI
  components and no visual constants to define. Their absence is correct,
  not an omission.
- **The "loading" state is genuinely undesigned**, not merely unspecified by
  this document — see step 2 and `RISK-0005`.

## Traceability

- Parent: `ARC-0001`.
- Satisfies: `REQ-0010`, `REQ-0017`.
- Depends on: `SPEC-0008`, `SPEC-0009`.
- Verified by: `TC-0007`.
