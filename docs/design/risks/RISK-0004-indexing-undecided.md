---
id: RISK-0004
title: Indexing is undecided, and both query performance and reactive diffing depend on it
type: risk
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-22
parents: [OBJ-0005]
tags: [data-model, performance, disclosed-gap]
---

# RISK-0004: Indexing is undecided, and both query performance and reactive diffing depend on it

## Description

Nothing in the doc tree decides whether indexes are declared in the schema,
inferred automatically, or left to the concrete storage engine.
`legacy/docs/03-data-model/query-model.md` names this as "a real, disclosed gap,
not an oversight," and `legacy/VISION.md` calls it out in the same terms:
"Indexing is explicitly not decided — a real, disclosed gap, not an
oversight."

Three separate docs independently flag it — `schema.md`,
`query-model.md`, and `performance-guards.md` — which is itself a signal
of how load-bearing it is.

## Category

Technical risk (non-security), data-model and performance scope.

## Impact if it materializes

Two things depend on efficient lookups against the predicates `REQ-0003`,
`REQ-0004` and `REQ-0005` define:

- **Query performance** directly.
- **`REQ-0008`'s incremental result-set diffing**, which is how a
  subscription decides whether a record entered or left a matched set
  without re-running the whole query. `ADR-0004` already records that this
  diffing is "real, non-trivial engineering, not a small helper function";
  without an indexing story it may not be implementable at acceptable cost
  at all.
- **`REQ-0009`'s opt-in granularities**, whose relative cost
  `legacy/docs/05-reactivity/performance-guards.md` notes "depends on the query" —
  a comparison that cannot be made concrete without knowing what lookups
  cost.

This also blocks `OBJ-0004` and `NFR-0001` from becoming measurable in a
meaningful way: a benchmark baseline taken against an unindexed
implementation would measure the wrong thing.

## Response

**Disclosed and deferred — needs its own decision before implementation.**
`ADR-0027` states the reasoning for not deciding it now: it should be
"informed by which predicates actually get used once there's a real schema
to measure against."

That is a defensible sequencing argument, but it means the decision is
genuinely outstanding rather than settled, and it is a prerequisite for the
performance work `ADR-0022` sequences.

## Traceability

- Parent: `OBJ-0005`.
- Disclosed in: `ADR-0002`, `ADR-0004`, `ADR-0027`.
- Affected: `REQ-0003`, `REQ-0008`, `SPEC-0004`, `SPEC-0006`, `NFR-0001`.

## Update — 2026-10-22

RFD-0001 is committed and ADR-0030 accepts engine-owned physical indexes after
operator approval. Index ownership is now decided; this risk remains active
because SPEC-0025/TC-0025 still require measured construction/update/lookup
costs and Phase 3 still requires genuine incremental live-query maintenance.
The original description records the motivating gap and is retained as history.
