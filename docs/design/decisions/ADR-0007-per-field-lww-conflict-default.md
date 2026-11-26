---
id: ADR-0007
title: Conflict resolution — per-field last-write-wins, deletes win, overridable per table
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, conflict-resolution, entity-diff]
---

# ADR-0007: Conflict resolution — per-field last-write-wins, deletes win, overridable per table

Source: `legacy/refined-vision/07-conflict-resolution/alternatives.md` (Options
A/B/C/D, Option A chosen), forked into `legacy/docs/06-sync/conflict-resolution.md`.

## Context and problem statement

The command path needs no separate merge step — the backend's authoritative
re-run *is* the resolution (`ADR-0005`). The entity-diff fallback path does
need one: two writes to the same record can genuinely disagree with no
re-run to arbitrate. `legacy/NEEDS.md` lists conflict resolution as an open
question; `legacy/VISION.md` rules out "last write to arrive wins the whole
record, loser silently discarded" as a default.

## Considered options

- **Option A — per-field/column LWW default, deletes win, pluggable
  per-table hook.** Two independent, unrelated systems converge on exactly
  this without coordinating: PowerSync's default backend behaviour ("the
  last update, as received by the server, to each individual field
  wins... deletes always win") and cr-sqlite's default per-column CRDT type
  (also LWW). The per-table override hook mirrors RxDB's pluggable
  conflict-handler object.
- **Option B — server-wins as a starting point, client-wins per changed
  field on top.** WatermelonDB's algorithm; per the reference comparison,
  probably the single most reusable concrete algorithm in the whole
  research set.
- **Option C — fully automatic per-column CRDT dispatch, no manual hook.**
  cr-sqlite's model, assigning each column a CRDT type (LWW,
  fractional-index for ordering, counters) at the schema level.
- **Option D — deterministic winner, but the loser preserved for the app to
  reconcile.** PouchDB/CouchDB's `_conflicts` approach.

## Decision outcome

In the context of resolving disagreeing writes on the entity-diff fallback
path, facing `legacy/VISION.md`'s refusal to silently discard a whole losing
record, we decided for **per-field last-write-wins with deletes always
winning, overridable per table** (Option A) and against WatermelonDB's
client-wins-on-top algorithm (B), fully automatic per-column CRDT dispatch
(C), and loser-preserving resolution (D), to achieve field-granular
resolution matching the closest thing the research found to an industry
default, accepting that per-field LWW still silently loses one side of a
genuine field-level double-write.

### Consequences

**Good**
- Two unrelated systems reaching the same default is the reference
  research's own basis for calling this "the closest thing to an industry
  default."
- Field granularity (not record granularity) is what avoids the silent
  whole-record loss `legacy/VISION.md` rejects. This works because an entity-diff
  payload carries only the fields actually written, as a partial patch
  (`REQ-0023`).
- Any table can replace the default with its own resolver — `legacy/VISION.md`
  rejects "a merge strategy with no override."

**Bad**
- **This default is the floor, not the ceiling, and loses data in a real
  case**: two users incrementing the same counter in the same window both
  see their own value overwritten, not summed. This is exactly the
  counter-CRDT gap that motivates the command path existing as the primary
  mechanism. A table with that kind of merge need should use a command, not
  a per-table override of this default.
- Options B, C and D remain available as per-table overrides rather than
  being ruled out. Option D in particular is strictly safer against silent
  data loss and is worth offering for safety-sensitive fields (financial,
  or anything where "flag it for a human" beats any automatic rule).
- The exact shape of the override hook — its signature, what state it
  receives (both candidate writes? the record's full history?) — is **not
  fixed**; implementation-time detail.
- Identical-timestamp ties are not resolved by this ADR; see `ADR-0017`.

### Confirmation

Confirmed by `SPEC-0011` (conflict resolution) and the conflict test cases
in `TP-0001`.

## Pros and cons of the options

### Option A — per-field LWW, deletes win, overridable (chosen)
- Good: two independent precedents; field granularity; overridable.
- Bad: silently loses one side of a genuine field-level double-write.

### Option B — WatermelonDB's server-wins-then-client-fields
- Good: more nuanced than plain LWW — a local edit is not lost just because
  the server's clock or write order happened to be later. Most reusable
  concrete algorithm in the research set.
- Bad: needs local `_status`/`_changed` per-row bookkeeping, a real ongoing
  storage cost, and still does not solve the counter problem.

### Option C — automatic per-column CRDT dispatch
- Good: handles ordering and, if counters are correctly supported,
  counters, entirely automatically.
- Bad: "no manual hook at all" is the opposite pole from hoyadb's pluggable
  design; and cr-sqlite's own docs and README disagreeing on whether its
  counter CRDT actually works is itself evidence this is harder to get
  fully right than it looks, even for the project that specializes in it.

### Option D — loser preserved
- Good: strictly safer against silent data loss than any of the above.
- Bad: defers the actual reconciliation work to the app, which must
  actively look at the preserved losing revision or it accumulates unseen.
  Not adopted as the default for that reason; kept as a per-table option.

## Traceability

- Parent: `OBJ-0003`.
- Referenced by: `SPEC-0011`.
- Refined by: `ADR-0017` (identical-timestamp tie-break).
