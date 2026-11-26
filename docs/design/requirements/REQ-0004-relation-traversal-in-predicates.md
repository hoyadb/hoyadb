---
id: REQ-0004
title: A query filters on a to-one relation's field by dot-path
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0005]
tags: [data-model, query, relations]
---

# REQ-0004: A query filters on a to-one relation's field by dot-path

## Pattern

Ubiquitous, with an Unwanted-behaviour clause for unsupported traversal.

## Statement

THE system SHALL accept a dot-path field name in a query predicate to
filter on a field of a declared to-one relation, without the application
issuing a separate join call.

IF a predicate's dot-path traverses a has-many or many-to-many relation,
THEN THE system SHALL reject the query rather than silently evaluating a
different predicate — the predicate model is scoped to a query's own table
plus its direct to-one relations (`ADR-0027`).

## Examples

Example 1 — filtering orders by their customer's country
(`legacy/docs/03-data-model/query-model.md`, and `legacy/docs/10-walkthrough.md` step 2):

```typescript
const frenchCustomerOrders = await db
  .query("orders")
  .where("customer.country", "=", "FR")
  .fetch();
```

Result: orders whose related `customers` record has `country` equal to
`"FR"`. `customer` is the to-one relation declared in `REQ-0002`'s example;
no explicit join is written.

Example 2 — unsupported traversal rejected:

- Input: `db.query("orders").where("items.quantity", ">", 5)`, where
  `items` is a has-many relation.
- Output: the query is rejected. Filtering a table by a field on the
  *many* side is not covered in this version.

## Traceability

- Parent: `OBJ-0005`.
- Under: `ADR-0027`.
- Satisfied by: `SPEC-0004`.
- Verified by: `TC-0002`.
