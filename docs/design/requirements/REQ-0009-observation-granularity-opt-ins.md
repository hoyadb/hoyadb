---
id: REQ-0009
title: Coarser and finer observation granularity are available as explicit opt-ins
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0005]
tags: [reactivity, subscriptions, performance]
---

# REQ-0009: Coarser and finer observation granularity are available as explicit opt-ins

## Pattern

Optional feature (two instances), with an Unwanted-behaviour clause against
silent substitution.

## Statement

WHERE an application opts into count-only observation, THE system SHALL
emit only the matched result's count, without materializing or diffing full
records, and SHALL throttle emissions so a burst of writes does not flood
the consumer.

WHERE an application opts into per-field observation over a named set of
columns, THE system SHALL additionally re-emit when any of those named
columns changes on an already-matched record, beyond the set-membership
changes `REQ-0008` already covers.

IF an application requests neither opt-in, THEN THE system SHALL apply
`REQ-0008`'s default and SHALL NOT silently substitute either opt-in
behaviour.

## Examples

Example 1 — count-only, for a badge
(`legacy/docs/05-reactivity/performance-guards.md`):

```typescript
db.query("orders").where("status", "=", "pending")
  .observeCount((count) => renderPendingOrdersBadge(count));
```

Result: the callback receives a number, not records, and is throttled. A
burst of ten pending orders arriving together does not produce ten
separate UI updates.

Example 2 — per-field, for a running total:

```typescript
db.query("orders").where("status", "=", "pending")
  .observeWithColumns(["total_cents"], (pendingOrders) =>
    renderPendingOrdersTotal(pendingOrders));
```

Result: given an already-matched pending order whose `total_cents` changes
from 4200 to 3800, this re-emits — where `REQ-0008`'s default would not,
since the order neither entered nor left the set. A change to that same
order's `notes` still does not re-emit, because `notes` is not in the named
column set.

## Open

Neither opt-in is universally cheaper than the default — which direction
costs less depends on the query, and that is exactly why one default plus
named opt-ins was chosen over a multi-tier model (`ADR-0004`). Whether
further tiers (e.g. explicit add/remove/update delta events) get added is
**not decided**; this fixes the two that exist today, not a closed final
set.

## Traceability

- Parent: `OBJ-0005`.
- Under: `ADR-0004`.
- Satisfied by: `SPEC-0007`.
- Verified by: `TC-0004`.
