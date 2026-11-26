---
id: OBJ-0004
title: Establish a performance baseline — no numeric target is set yet
type: objective
status: draft
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [VIS-0001]
tags: [performance, open-question]
---

# OBJ-0004: Establish a performance baseline — no numeric target is set yet

## Statement

**This objective has no measurable target, and that is its actual content,
not an omission.** hoyadb has not committed to any numeric performance
figure. What is decided is the *process*: establish a benchmark baseline
once there is an implementation to measure, then hold or improve it.

This artifact deliberately records the absence of a target rather than
inventing one. `legacy/docs/08-non-functional/performance.md` is explicit: "This
is the *process* that's decided, not the figures: this doc is a
placeholder for real numbers until an implementation exists to produce a
baseline from." `legacy/NEEDS.md` lists "concrete performance targets/benchmarks"
under its own "Open questions (not yet decided)".

Its status is therefore `draft`, not `active`: an objective with no
Scale/Meter/Target does not meet the Frame-phase exit criteria in
`ai-governance/framework/01-lifecycle.md` ("each OBJ has a
Scale/Meter/Target"), and marking it `active` would misrepresent that.

## Baseline

**Unknown — no implementation exists to measure.** This is the literal
reason no target can be set: a figure chosen now would be a guess against
nothing.

## Target metric

**Not yet defined.** The qualitative bar carried forward from `legacy/NEEDS.md`'s
"Performance" section is: apps built on hoyadb "must not feel slow," with
the explicit caveat that hoyadb "is not expected to match the state of the
art for raw storage performance" — sync, reactivity, and command transform
are real added layers, not a free abstraction.

What must happen before this objective can carry a metric, per `ADR-0022`:

1. An implementation exists far enough to run a benchmark (gated on
   `ADR-0008`'s roadmap).
2. A benchmark methodology is designed — which operations, which local
   dataset sizes, on which platform/engine combination. None of this is
   fixed; `legacy/docs/08-non-functional/performance.md`'s "What's still open"
   says so directly.
3. The resulting figures become the baseline, and "stay at or improve on
   that baseline" becomes the working target.

`NFR-0001` is the quality requirement that will carry the numbers once
they exist; it currently records the same gap, with its Must and Target
lines explicitly unfilled.

## Target date

None. Tightening performance is sequenced *after* the core
schema/storage/reactivity/sync surface is built out — a secondary
objective by explicit decision (`ADR-0022`), given the size of the rest of
the project.

## Rationale

Recording this as an explicit gap is the point. `CLAUDE.md`'s working
rules require saying "not found"/"unknown" rather than offering a
plausible-sounding guess, and `legacy/PLAN.md`'s ground rule 2 forbids inventing
what has not been decided. A fabricated "p95 under 50ms" here would be
exactly that.

## Traceability

- Parent: `VIS-0001`.
- Decided by: `ADR-0022` (performance approach — baseline, then hold).
- Realized by: `NFR-0001` (which carries the same unfilled target).
