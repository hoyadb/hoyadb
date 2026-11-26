---
id: REQ-0002
title: Relations are first-class schema constructs, navigable from either side
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0002]
tags: [data-model, relations]
---

# REQ-0002: Relations are first-class schema constructs, navigable from either side

## Pattern

Ubiquitous.

## Statement

THE system SHALL support three relation kinds — to-one, has-many, and
many-to-many — as declared schema constructs, each with its own declaration
form and its own load behaviour, and SHALL make a declared relation
navigable from either side without the application writing its own join
query.

THE system SHALL NOT eagerly load a relation by default: navigating a
relation SHALL return a query object the application resolves, either once
or as a live subscription (`REQ-0007`), rather than a record already
materialized in memory.

"First-class" here means specifically that an application never hand-builds
a relation out of a plain filtered query or a manually-maintained pivot
table.

## Examples

Example 1 — declaring all three kinds:

```typescript
customers: table({
  columns: { name: column.string(), email: column.string() },
  relations: { orders: relation.hasMany("orders", { foreignKey: "customer_id" }) },
}),
orders: table({
  columns: { status: column.string(), total_cents: column.integer() },
  relations: {
    customer: relation.toOne("customers", { foreignKey: "customer_id" }),
    items: relation.hasMany("order_items", { foreignKey: "order_id" }),
  },
}),
```

Result: `order.customer` resolves the to-one side and `customer.orders` /
`order.items` resolve the has-many sides, each as a query object — not as
eagerly-loaded records.

Example 2 — many-to-many through an explicit join table: `orders` and `products` joined via
`order_products`, exposed on both sides as
`relation.manyToMany("products", { through: "order_products" })` and
`relation.manyToMany("orders", { through: "order_products" })`. Navigating
either side returns a query object; neither side requires the application
to query `order_products` itself.

## Open

Whether a many-to-many join table is always app-visible and app-queryable
or can instead be an implicit hoyadb-managed table is **not decided**
(`ADR-0002`). Eager/batched loading across a relation — avoiding N+1
patterns when iterating a large has-many result — is implementation-time
detail, not specified. Querying *through* a has-many or many-to-many
relation in a predicate is out of scope for this version (`REQ-0004`).

## Traceability

- Parent: `OBJ-0002`.
- Under: `ADR-0002`.
- Satisfied by: `SPEC-0003`, `DM-0001`.
- Verified by: `TC-0001`.
