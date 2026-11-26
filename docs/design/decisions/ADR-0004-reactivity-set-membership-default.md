---
id: ADR-0004
title: Reactivity — one query object, two modes, set-membership change as the default trigger
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0005]
tags: [reactivity, subscriptions, performance]
---

# ADR-0004: Reactivity — one query object, two modes, set-membership change as the default trigger

Source: `legacy/refined-vision/04-reactivity-subscriptions/alternatives.md`, forked
into `legacy/docs/05-reactivity/subscriptions.md` and
`legacy/docs/05-reactivity/performance-guards.md`.

## Context and problem statement

`legacy/NEEDS.md`'s "Reactivity" section requires that applications subscribe to
an entity or a query and be notified as local data changes. The open
question is the *granularity* of that notification: too coarse and every
app re-filters a raw change stream itself; too fine and every unrelated
field edit wakes the UI.

## Considered options

- **One query object supporting both a one-shot read and a live
  subscription, defaulting to re-notify only on set-membership change,
  with coarser and finer opt-ins.** Two independent systems converge on
  exactly this: RxDB's `RxQuery` (`.exec()` / `.$`, incremental
  "EventReduce" updates) and WatermelonDB's `Query` (`.fetch()` /
  `.observe()`, plus `observeCount()` throttled and `observeWithColumns()`
  per-field as explicit opt-ins).
- **A multi-tier configurable-granularity model** — PowerSync's
  basic/incremental/explicit-delta tiers.
- **A coarse whole-table or whole-database change feed** with no
  query-result-level reactivity — the fallback used by every reference
  system that does not own a schema/query layer.

## Decision outcome

In the context of defining how an application observes local data changes,
facing the trade-off between notification precision and cost, we decided
for **one query object with one-shot and live modes, defaulting to
re-notify only on set-membership change** and against PowerSync's
multi-tier model and against a coarse whole-table/whole-database feed, to
achieve a cheap default that still solves "did *my* data change" once
inside hoyadb rather than in every app, accepting that the default's
incremental result-set diffing is real, non-trivial engineering rather than
a small helper.

### Consequences

**Good**
- Two non-collaborating systems converging on the same one-object/two-modes
  shape *and* the same "cheap by default, precision by opt-in" philosophy
  is stronger evidence than either alone.
- An app that needs different granularity opts in explicitly
  (`observeCount()`, `observeWithColumns([...])`) — never a silent
  behaviour change.

**Bad**
- Incremental result-set diffing (RxDB's own name for it: "EventReduce") is
  named in `legacy/docs/05-reactivity/subscriptions.md` as non-trivial
  engineering, not a small helper function. Sharing one subscription across
  N identical `.observe()` calls on the same query is a second non-trivial
  piece needed for this default to perform at scale.
- One default tier instead of three means less flexibility than PowerSync's
  model. Set aside for now, explicitly worth revisiting once this tier's
  real limits are felt in practice.
- **Indexing, which this diffing depends on for efficient lookups, is not
  decided** — `RISK-0004`.
- Whether further opt-in tiers get added later is **not decided**; this
  fixes the two that exist today, not a closed final set.
- Propagating these notifications across the FFI/WASM boundary into a
  platform's UI-reactivity system is **unresolved** (`ADR-0001`,
  `ADR-0008`).

### Confirmation

Confirmed by `SPEC-0006` (subscriptions) and `SPEC-0007` (performance
guards), and by the reactivity test cases in `TP-0001`.

## Pros and cons of the options

### One object, two modes, set-membership default (chosen)
- Good: two independent precedents; cheap default; explicit opt-ins.
- Bad: incremental diffing is genuinely hard; depends on undecided indexing.

### Multi-tier configurable granularity (PowerSync)
- Good: more flexible.
- Bad: three behaviours to design, document and keep consistent instead of
  one default with an opt-in fast path — more surface area than a first
  version needs.

### Coarse whole-table / whole-database feed
- Bad: rejected outright — pushes re-filtering onto every app built on
  hoyadb, and `ADR-0002` already commits to owning the schema, which
  positions hoyadb to do better.

## Traceability

- Parent: `OBJ-0005`.
- Referenced by: `ARC-0001`, `SPEC-0006`, `SPEC-0007`.
- Risk: `RISK-0004`.
