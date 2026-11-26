---
id: REQ-0008
title: A subscription re-emits only on set-membership change by default
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0005]
tags: [reactivity, subscriptions, performance]
---

# REQ-0008: A subscription re-emits only on set-membership change by default

## Pattern

Event-driven, with an Unwanted-behaviour clause for an unrelated field edit.

## Statement

WHEN a record enters or leaves a live subscription's matched result set,
THE system SHALL re-emit that subscription's result.

IF a field changes on a record that is already in the matched set, and that
change does not move the record into or out of the set, THEN THE system
SHALL NOT re-emit — unless the application has explicitly opted into finer
granularity (`REQ-0009`).

THE system SHALL NOT require an application to subscribe to a whole table
or the whole database and filter the stream itself.

## Examples

Example 1 — entering the set:

- Given a subscription on `orders` where `status = "pending"`, currently
  matching 3 orders.
- When a new order is created with `status: "pending"`.
- Then the subscription re-emits with 4 orders.

Example 2 — unrelated edit does **not** re-emit:

- Given the same subscription, with order `order_456` already matched.
- When `order_456.notes` is changed to `"Gift wrap"` (its `status` stays
  `"pending"`, so it neither enters nor leaves the set).
- Then the subscription does **not** re-emit. This is the default
  granularity; `observeWithColumns(["notes"], ...)` would be the explicit
  opt-in that does (`REQ-0009`).

Example 3 — leaving the set:

- When `order_456.status` changes from `"pending"` to `"shipped"`.
- Then the subscription re-emits with that order absent.

## Open

Implementing this default well requires incremental result-set diffing —
named in `SPEC-0006` as real, non-trivial
engineering, not a helper function — and sharing one subscription across N
identical observers on the same query. Both are needed for the default to
perform at scale. Indexing, which the diffing depends on, is **undecided**
(`RISK-0004`).

## Traceability

- Parent: `OBJ-0005`.
- Under: `ADR-0004`.
- Satisfied by: `SPEC-0006`.
- Verified by: `TC-0004`.
