---
id: SPEC-0007
title: Subscription performance guards — the coarser and finer opt-ins
type: spec
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [ARC-0001, ADR-0004]
satisfies: [REQ-0009]
depends_on: [SPEC-0006]
tags: [reactivity, performance]
---

# SPEC-0007: Subscription performance guards — the coarser and finer opt-ins

## Purpose

Specify the two directions an application can move away from `SPEC-0006`'s
default granularity — each an explicit, named opt-in on the same query
object, never a silent behaviour change.

## Opt-in: coarser (count-only)

For an application that needs only a number — a badge count — subscribing
to the count avoids materializing and diffing full records on every
set-membership change, and is **throttled** so a burst of writes does not
flood the consumer:

```typescript
const unsubscribe = db
  .query("orders").where("status", "=", "pending")
  .observeCount((count) => renderPendingOrdersBadge(count));
```

## Opt-in: finer (per-field)

For an application that needs to know when a specific field changes on an
already-matched record — not just when the record enters or leaves the set —
per-field observation is available, **scoped to the columns the application
names**:

```typescript
const unsubscribe = db
  .query("orders").where("status", "=", "pending")
  .observeWithColumns(["total_cents"], (pendingOrders) =>
    renderPendingOrdersTotal(pendingOrders));
```

This is more expensive than the default: it re-checks on every write to a
matched record's tracked columns, not only on set-membership changes. A
change to a matched order's `notes` still does not re-emit here, because
`notes` is not in the named set.

## Why these stay opt-in

Neither is universally cheaper than the default — which direction costs less
depends on the query. There is no single tier that is cheapest for
everything, which is exactly why `ADR-0004` picked one default with named
opt-ins. An
application opts into whichever direction its specific UI needs, rather than
hoyadb guessing.

## Open

- **Whether further tiers beyond count-only and per-field get added later is
  not decided.** This spec fixes the two that exist today, not a closed
  final set (`ADR-0004`).
- **Indexing — which these opt-ins' actual performance depends on — is
  undecided** (`RISK-0004`). The relative cost claims above cannot be made
  concrete without it.
- The throttle interval for count-only observation is not fixed here.

## Traceability

- Parents: `ARC-0001`, `ADR-0004`.
- Satisfies: `REQ-0009`.
- Depends on: `SPEC-0006`.
- Verified by: `TC-0004`.

## reactivity contract

API-0005 implements explicit Rust observe_count and observe_with_columns over
one shared projected key cache. Count materializes no full records and uses a
16 ms host-clock deadline, including after recovery; hosts schedule drains.
Named columns compare stored values, including missing versus Null. Explicitly
requested cells remain strict, while projected ordering keys follow API-0003
Null normalization. TC-0029/TC-0030 cover work, timing and error partitions.
TypeScript examples remain illustrative. No additional reactive tier or new
numerical performance budget is established.
