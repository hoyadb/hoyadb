---
id: ADR-0027
title: Query model — a deliberately minimal operator catalogue; indexing left undecided
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0005]
tags: [data-model, query, open-question]
---

# ADR-0027: Query model — a deliberately minimal operator catalogue; indexing left undecided

## Context and problem statement

`.where(field, op, value).fetch()`/`.observe()` is the single most-used API
surface in the whole doc tree — it appears in `schema.md`, `relations.md`,
`subscriptions.md`, `performance-guards.md`, `command-protocol.md` and
`platform-bindings.md` — but every sample used only `"="`, and no doc
enumerated what else exists.

## Considered options

- **A minimal catalogue**: comparison operators, implicit AND across
  multiple `.where()` calls, to-one relation traversal by dot-path,
  `.orderBy()`, `.limit()`.
- **A full query-builder DSL** with OR, explicit combinators, multi-relation
  joins and aggregates.
- **Leave it unspecified**, continuing to imply precision the docs did not
  have.

## Decision outcome

In the context of what a `.where(...)` chain actually supports, facing docs
that implied more precision than existed, we decided for **a minimal
catalogue** and against a full query-builder DSL and against leaving it
unspecified, to achieve a real reference a reader can use while keeping the
surface small enough to extend later, accepting that several genuinely
useful capabilities are absent from this version.

### Consequences

**Good**
- Concretely fixed: operators `=`, `!=`, `<`, `<=`, `>`, `>=`, `in`;
  multiple `.where()` calls combined as implicit AND; to-one relation
  traversal via dot-path (`.where("customer.country", "=", "FR")`);
  `.orderBy(field, "asc"|"desc")`; `.limit(n)`.
- Deliberately smaller than it could be — extending this is easier than
  walking back an over-designed surface. Matches
  `SPEC-0006`'s own "cheap by default" philosophy.

**Bad**
- **Indexing is explicitly not decided** — named as a real, disclosed gap
  rather than assumed away. Both query performance and the incremental
  result-set diffing `ADR-0004` depends on assume efficient lookups, and
  nothing says whether indexes are declared in the schema, inferred, or
  left to the concrete engine. It needs its own decision before
  implementation. Tracked as `RISK-0004`.
- No OR and no explicit combinator in this version.
- No traversal of has-many or many-to-many relations in a predicate — the
  predicate model is scoped to a query's own table plus direct to-one
  relations.
- **No offset/cursor pagination beyond a single `limit(n)` cap** — "load
  the next page" is not covered and is not decided here either.
- No aggregate functions beyond what `.fetch().length` already does
  implicitly.
- The exact method names and signatures are this doc's reference
  convention, **not a literal API contract** — consistent with
  `SPEC-0001`'s "implementation-time detail" framing.

### Confirmation

Confirmed by `SPEC-0004` (the query model) and the query test cases in
`TP-0001`, which exercise each listed operator, dot-path traversal,
ordering and limit.

## Traceability

- Parent: `OBJ-0005`.
- Referenced by: `SPEC-0004`, `SPEC-0006`.
- Risk: `RISK-0004`.
