---
id: REQ-0007
title: One query object supports both a one-shot read and a live subscription
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0005]
tags: [reactivity, subscriptions]
---

# REQ-0007: One query object supports both a one-shot read and a live subscription

## Pattern

Ubiquitous.

## Statement

THE system SHALL expose, on the same query object, both a one-shot read
that resolves the query's current result once and a live subscription that
re-emits as matching local data changes, and SHALL return a handle from the
subscription that cancels it when invoked.

Both modes SHALL be available on every query, including one reached by
navigating a relation (`REQ-0002`), and both SHALL honour the same
predicates, ordering and limit (`REQ-0003`, `REQ-0004`, `REQ-0005`).

## Examples

Example 1 — both modes on the same query:

```typescript
// One-shot read
const pendingOrders = await db
  .query("orders").where("status", "=", "pending").fetch();

// Live subscription on the same query shape
const unsubscribe = db
  .query("orders").where("status", "=", "pending")
  .observe((pendingOrders) => {
    renderPendingOrdersBadge(pendingOrders.length);
  });

unsubscribe(); // later — cancels the subscription
```

Result: the one-shot call resolves once with the currently-pending orders.
The subscription invokes its callback with the same result, then again on
each subsequent set-membership change (`REQ-0008`), until `unsubscribe()`
is called.

Example 2 — both modes work with no network: both resolve entirely against
the local store, with no connectivity required (`NFR-0002`).

## Open

Propagating these notifications from Rust-core state across the FFI/WASM
boundary into a platform's own UI-reactivity system is named in
`ARC-0002` as the hardest open design
question the core/platform split raises. **Unresolved** (`ADR-0008`,
`ARC-0002`). This requirement fixes the API-level contract, not the binding
mechanism that implements it.

## Traceability

- Parent: `OBJ-0005`.
- Under: `ADR-0004`.
- Satisfied by: `SPEC-0006`.
- Verified by: `TC-0004`.
