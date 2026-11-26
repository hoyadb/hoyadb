---
id: REQ-0005
title: A query orders its results and caps their count
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0005]
tags: [data-model, query]
---

# REQ-0005: A query orders its results and caps their count

## Pattern

Ubiquitous.

## Statement

THE system SHALL support ordering a query's results by a named field in
ascending or descending direction, and SHALL support capping the result
count at a given number.

Both SHALL compose with a one-shot read and with a live subscription
(`REQ-0007`) in the same way a predicate does.

THE system SHALL NOT provide offset or cursor pagination beyond a single
count cap in this version.

## Examples

Example 1 — the 20 most recent orders for one customer
(`legacy/docs/03-data-model/query-model.md`):

```typescript
const recentOrders = await db
  .query("orders")
  .where("customer_id", "=", "cust_123")
  .orderBy("placed_at", "desc")
  .limit(20)
  .fetch();
```

Result: at most 20 orders for `cust_123`, newest `placed_at` first.

Example 2 — the same ordering on a live subscription
(`legacy/docs/10-walkthrough.md` step 2):

```typescript
db.query("orders")
  .where("customer.country", "=", "FR")
  .orderBy("placed_at", "desc")
  .observe((orders) => renderOrdersList(orders));
```

Result: the callback receives results in the same order a `.fetch()` would
have produced, and re-runs on set-membership change (`REQ-0008`).

## Open

**"Load the next page" is not covered and not decided.**
`legacy/docs/05-reactivity/subscriptions.md` separately flags sort/pagination
options as unspecified, and `legacy/docs/03-data-model/query-model.md` explicitly
declines to resolve it — naming a single count cap as the one piece that
exists today. A real, disclosed gap.

## Traceability

- Parent: `OBJ-0005`.
- Under: `ADR-0027`.
- Satisfied by: `SPEC-0004`.
- Verified by: `TC-0002`.
