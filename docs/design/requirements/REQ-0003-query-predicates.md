---
id: REQ-0003
title: A query filters with comparison operators combined as implicit AND
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0005]
tags: [data-model, query]
---

# REQ-0003: A query filters with comparison operators combined as implicit AND

## Pattern

Ubiquitous.

## Statement

THE system SHALL support the comparison operators `=`, `!=`, `<`, `<=`,
`>`, `>=` and `in` in a query predicate, and SHALL combine multiple
predicates on one query as a logical AND.

THE system SHALL NOT provide OR, nor any explicit combinator, in this
version — the predicate surface is deliberately minimal (`ADR-0027`).

## Examples

Example 1 — two predicates, implicit AND
(`legacy/docs/03-data-model/query-model.md`):

```typescript
const activeOrders = await db
  .query("orders")
  .where("status", "=", "pending")
  .where("total_cents", ">=", 1000)
  .fetch();
```

Result: orders whose `status` is `"pending"` **and** whose `total_cents`
is at least 1000. An order with `status: "pending"` and
`total_cents: 500` does not match.

Example 2 — `in` against an array of values:

- Input: `.where("status", "in", ["pending", "shipped"])`
- Output: orders whose status is either `"pending"` or `"shipped"`. This is
  the only way to express a disjunction in this version, and only over one
  field's values.

## Open

Indexing — which this predicate surface's performance depends on — is
**not decided** anywhere in the tree (`RISK-0004`). The exact method names
and signatures shown are a reference convention, not a literal API contract
(`ADR-0027`).

## Traceability

- Parent: `OBJ-0005`.
- Under: `ADR-0027`.
- Satisfied by: `SPEC-0004`.
- Verified by: `TC-0002`.
