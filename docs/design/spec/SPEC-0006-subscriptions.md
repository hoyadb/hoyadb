---
id: SPEC-0006
title: Subscriptions — one query object, two consumption modes
type: spec
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [ARC-0001, ADR-0004]
satisfies: [REQ-0007, REQ-0008]
depends_on: [SPEC-0004, SPEC-0001]
tags: [reactivity, subscriptions]
---

# SPEC-0006: Subscriptions — one query object, two consumption modes

Source: `legacy/docs/05-reactivity/subscriptions.md`.

## Purpose

Specify how an application reads a query once or subscribes to it live, and
the default granularity at which a live subscription re-emits.

## The two modes

One query object supports both. Predicates, relation traversal, ordering and
limit (`SPEC-0004`) apply identically to each:

```typescript
// One-shot
const pendingOrders = await db
  .query("orders").where("status", "=", "pending").fetch();

// Live — same query shape
const unsubscribe = db
  .query("orders").where("status", "=", "pending")
  .observe((pendingOrders) => {
    renderPendingOrdersBadge(pendingOrders.length);
  });

unsubscribe();
```

Both resolve entirely against the local store, with no connectivity required
(`NFR-0002`).

## Default granularity: set-membership change

A live subscription re-emits when a record **enters or leaves** the matched
result set, and **not** when a field changes on an already-matched record
without moving it in or out of the set.

| Event | Re-emits? |
| --- | --- |
| A new order is created with `status: "pending"` | Yes — entered the set |
| A matched order's `status` changes to `"shipped"` | Yes — left the set |
| A matched order's `notes` changes | **No** — still in the set, unrelated field |
| A matched order's `total_cents` changes | **No** by default; yes under `SPEC-0007`'s per-field opt-in |

An application never has to subscribe to a whole table or the whole database
and re-filter the stream itself — that work is solved once here rather than
in every app built on hoyadb.

## Engineering this default actually requires

This design names the cost rather than presenting the default as simple:

- **Incremental result-set diffing** — RxDB's own name for its equivalent is
  "EventReduce" — is real, non-trivial engineering, not a small helper
  function. Updating a result incrementally rather than re-running the full
  query on every change is what makes the default affordable at all.
- **Sharing one subscription across N identical observers** on the same
  query (WatermelonDB's per-query Observable caching) is a second
  non-trivial piece needed for this default to perform at scale — not an
  optional nicety.

## Open

- **Indexing is undecided**, and the incremental diffing above depends on
  efficient lookups against `SPEC-0004`'s predicates (`RISK-0004`).
- **Sort and pagination beyond a single `limit(n)` cap remain open**
  (`SPEC-0004`).
- **Propagating change notifications across the core/platform boundary is
  unresolved** — `ARC-0002` names this as the hardest open design question
  the Rust-core split raises, and it is exactly this mechanism that has to
  cross it (`RISK-0001`).
- **Cross-tab / cross-window propagation does not exist** — a second tab's
  writes are not observed here (`RISK-0003`).
- **Reactivity during an initial bootstrap sync is undefined** — what a
  subscription emits while the local store is still filling is unstated, so
  an application cannot distinguish "no matches" from "not arrived yet"
  (`RISK-0005`).

## Traceability

- Parents: `ARC-0001`, `ADR-0004`.
- Satisfies: `REQ-0007`, `REQ-0008`.
- Depends on: `SPEC-0004`, `SPEC-0001`.
- Verified by: `TC-0004`.

## Phase 3 implementation checkpoint — 2026-10-23

SPEC-0027/API-0005 implement this behavior on the public Rust model. A shared
projected candidate cache maintains affected IDs and relation dependencies;
owned row snapshots leave core through explicit host drains. TC-0029/TC-0030
cover fetch/observe parity, cancellation, shared maintenance and errors. The
TypeScript surface above remains illustrative; no packaged SDK is published.
The existing engine indexes from ADR-0030 support targeted lookups. Browser
worker/main-thread and Swift desktop delivery are experimental bounded bridges;
cross-client propagation, pagination and bootstrap semantics remain open.
