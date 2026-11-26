---
id: SPEC-0004
title: Query model — predicates, relation traversal, ordering and limit
type: spec
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-22
parents: [ARC-0001, ADR-0027]
satisfies: [REQ-0003, REQ-0004, REQ-0005]
depends_on: [SPEC-0003]
tags: [data-model, query]
---

# SPEC-0004: Query model — predicates, relation traversal, ordering and limit

## Purpose

Specify exactly what a query chain supports, so the surface used throughout
the rest of the design is a real reference rather than an implied one.
Written at lighter depth than most of this tree, matching its source
(`ADR-0027`, ADR-0027).

## Predicates

Supported operators: `=`, `!=`, `<`, `<=`, `>`, `>=`, `in` (against an array
of values). Multiple predicates on one query combine as **implicit AND**;
there is no OR and no explicit combinator in this version.

```typescript
const activeOrders = await db
  .query("orders")
  .where("status", "=", "pending")
  .where("total_cents", ">=", 1000)
  .fetch();
```

## Relation traversal

A predicate may filter on a field of a declared **to-one** relation using a
dot-path field name; no separate join call is written:

```typescript
const frenchCustomerOrders = await db
  .query("orders")
  .where("customer.country", "=", "FR")
  .fetch();
```

Traversing a has-many or many-to-many relation in a predicate is **not
supported** — the predicate model is scoped to a query's own table plus its
direct to-one relations. Such a query is rejected rather than silently
evaluating something else (`REQ-0004`).

## Ordering and limit

```typescript
const recentOrders = await db
  .query("orders")
  .where("customer_id", "=", "cust_123")
  .orderBy("placed_at", "desc")
  .limit(20)
  .fetch();
```

`orderBy` takes a field name and `"asc"` / `"desc"`; `limit` caps the result
count. Both compose with the one-shot read and the live subscription
(`SPEC-0006`) exactly as predicates do.

## Deliberately out of scope for this version

Per `ADR-0027`, all of the following are **absent by decision**, not by
oversight — extending is easier than walking back an over-designed surface:

- A full query-builder DSL.
- OR, and any explicit combinator.
- Joins across more than one relation in a single call.
- Aggregate functions beyond what `.fetch().length` does implicitly.
- **Offset or cursor pagination.** A single `limit(n)` cap is the only
  piece that exists; "load the next page" is not covered and is **not
  decided** — `SPEC-0006` separately flags
  sort/pagination as unspecified and this spec does not resolve it.

## Open

- **Indexing is not decided.** Query performance here, and `SPEC-0006`'s
  incremental result-set diffing, both depend on efficient lookups against
  these predicates — but whether indexes are declared in the schema,
  inferred automatically, or left to the concrete engine is unspecified.
  A real, disclosed gap needing its own decision before implementation
  (`RISK-0004`).
- **The exact method names and signatures above are this design's reference
  convention, not a literal API contract** — consistent with `SPEC-0001`'s
  implementation-time framing for the underlying interface.

## Traceability

- Parents: `ARC-0001`, `ADR-0027`.
- Satisfies: `REQ-0003`, `REQ-0004`, `REQ-0005`.
- Depends on: `SPEC-0003`.
- Verified by: `TC-0002`.

## Concrete storage contract

ADR-0030 resolves the deferred physical index ownership in favor of engines.
SPEC-0025/API-0003 fix the internal Rust query signatures, validation, primitive
comparison, missing/null to-one behavior and deterministic ordering/limit ties.
TC-0025 covers the same semantics across memory/native/browser SQLite and
separate index-cost evidence. Public query builders/live subscriptions and
pagination beyond one limit remain outside that storage slice.

## Implemented data model Rust boundary

SPEC-0026/API-0004 define the Rust schema, one-shot queries, lazy relations and
atomic lifecycle boundary. ADR-0031 fixes `__hoya_`, explicit queryable through
tables, seven authored migration primitives and strict queue-free reset/adoption
guards. TC-0026/0027/0028 define conformance and regressions. Use the
[runnable model example](../../../crates/hoyadb-storage-memory/examples/data_model.rs)
for the Rust surface; packaged SDK and sync acceptance remain separate.
