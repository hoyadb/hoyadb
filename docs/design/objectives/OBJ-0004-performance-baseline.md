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

Performance targets require measurements with a defined workload, dataset, engine and target. Do not infer a numeric guarantee from the qualitative objective; use the benchmark methodology and explicit release references.

Its status is therefore `draft`, not `active`: an objective with no
Scale/Meter/Target does not meet the Frame-phase exit criteria in
`ai-governance/framework/01-lifecycle.md` ("each OBJ has a
Scale/Meter/Target"), and marking it `active` would misrepresent that.

## Baseline

**Unknown — no implementation exists to measure.** This is the literal
reason no target can be set: a figure chosen now would be a guess against
nothing.

## Target metric

No universal numeric target is defined here. Sync, reactivity and command processing add work beyond raw storage. Measured platform/workload baselines govern regression checks.

What must happen before this objective can carry a metric, per `ADR-0022`:

1. An implementation exists far enough to run a benchmark (gated on
   `ADR-0008`'s roadmap).
2. A benchmark methodology is designed — which operations, which local
   dataset sizes, on which platform/engine combination. None of this is
   fixed; `NFR-0001`'s "What's still open"
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

Unknown performance figures must remain unknown until measured. A fabricated latency target is not an acceptance criterion.

## Traceability

- Parent: `VIS-0001`.
- Decided by: `ADR-0022` (performance approach — baseline, then hold).
- Realized by: `NFR-0001` (which carries the same unfilled target).
