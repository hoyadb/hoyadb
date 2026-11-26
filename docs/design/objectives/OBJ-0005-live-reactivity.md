---
id: OBJ-0005
title: Applications observe query results live, without re-filtering a raw change feed
type: objective
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [VIS-0001]
tags: [reactivity, subscriptions]
---

# OBJ-0005: Applications observe query results live, without re-filtering a raw change feed

## Statement

An application subscribes to a query and is notified when *that query's
own result* changes, so it never has to re-filter a whole-table or
whole-database change stream itself to work out whether its data moved.

## Baseline

n/a — new capability. The reference research found this shape in two
independent systems: RxDB's `RxQuery` (`.exec()` / `.$`, incremental
"EventReduce" updates) and WatermelonDB's `Query` (`.fetch()` /
`.observe()`, re-emitting on set-membership change). The research also
found the opposite pattern — a coarse whole-table change feed — in every
studied system that does *not* own a schema/query layer. hoyadb owns one
(`ADR-0002`), so the coarse feed is the baseline it is explicitly
choosing to beat.

## Target metric

Two pass/fail properties, measured by the reactivity cases in `TP-0001`:

1. A subscription on a query re-notifies when a record enters or leaves
   that query's matched result set, and does **not** re-notify on an
   unrelated field edit to an already-matched record. (This is the default
   granularity; `REQ-0009`'s opt-ins deliberately change it.)
2. No API surface exists that requires an application to subscribe to a
   whole table or the whole database and filter it itself.

## Target date

Not fixed — see `OBJ-0001`'s note on `ADR-0008`.

## Rationale

`legacy/NEEDS.md`'s "Reactivity" section requires that applications "subscribe or
observe an entity or a query, and be notified as the underlying local data
changes — not just read it once." `legacy/VISION.md` rejects "reactivity as a
bare whole-database or whole-table change feed" because "that pattern
pushes the work of filtering 'did *my* data change' onto every single app
built on hoyadb, instead of solving it once."

## Traceability

- Parent: `VIS-0001`.
- Realized by: `REQ-0003`, `REQ-0004`, `REQ-0005`, `REQ-0007`, `REQ-0008`,
  `REQ-0009`.
- Risk against this objective: `RISK-0004` (indexing is undecided, and
  incremental result-set diffing depends on efficient lookups).
