---
id: ADR-0022
title: Performance — establish a baseline once there is an implementation, then hold or improve it
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0004]
tags: [performance, open-question]
---

# ADR-0022: Performance — establish a baseline once there is an implementation, then hold or improve it

Source: `legacy/PLAN_DECISIONS.md` D14, folded into `legacy/VISION.md`'s "Performance"
and `legacy/docs/08-non-functional/performance.md`.

## Context and problem statement

`legacy/NEEDS.md`'s performance bar is qualitative — "must not feel slow,"
explicitly not "fastest local database." D14 asked whether to set numeric
targets now (query latency on N local rows, cold-start time, sync
round-trip budget) or stay qualitative until there is something real to
benchmark.

## Considered options

- **Set no numeric targets now.** Establish a benchmark baseline once an
  implementation exists, then treat "stay at or improve on that baseline"
  as the working target, sequencing performance work as a secondary
  objective after the core surface is built.
- **Set numeric targets now**, chosen by analogy to comparable systems.
- **Stay purely qualitative indefinitely**, with no baseline commitment at
  all.

## Decision outcome

In the context of setting performance targets for hoyadb, facing the
absence of any implementation to measure, we decided for **no numeric
targets now plus a committed baseline-then-hold process** and against
setting targets by analogy and against staying purely qualitative
indefinitely, to achieve a target that is grounded in a real measurement
whenever one exists, accepting that until then hoyadb has **no measurable
performance commitment at all** — and says so rather than implying one.

### Consequences

**Good**
- Honest. A figure set against no implementation would be a guess, and
  `legacy/PLAN.md`'s ground rule 2 and `CLAUDE.md`'s working rules both forbid
  inventing what has not been decided.
- The process is genuinely decided even though the figures are not:
  baseline first, then hold or improve on future changes.
- Sequencing is explicit — tightening performance is a secondary objective,
  deliberately after the core schema/storage/reactivity/sync surface, given
  the size of the rest of the project.

**Bad**
- **`OBJ-0004` and `NFR-0001` both carry an empty target as their actual
  content.** There is no Must, no Target, and no Past value. That is the
  recorded state, not an incomplete artifact.
- **No benchmark methodology is fixed either** — which operations, which
  local dataset sizes, on which platform/engine combination.
  `legacy/docs/08-non-functional/performance.md`'s "What's still open" says so
  directly. So even the baseline cannot be produced without further design.
- hoyadb accepts up front that it is "not expected to match the state of
  the art for raw storage performance," because sync, reactivity and
  command transform are real added layers on top of the raw engine.
- Gated on `ADR-0008`'s roadmap: no baseline can exist before the first
  implementation step runs.

### Confirmation

Nothing to confirm by test today — that is the decision's content.
`NFR-0001` is the artifact that will carry the figures once a baseline
exists; `legacy/docs/08-non-functional/performance.md` says the doc "should be
revisited and updated with that baseline rather than continuing to read as
purely qualitative."

## Traceability

- Parent: `OBJ-0004`.
- Realized by: `NFR-0001`.
- Depends on: `ADR-0008` (no baseline before an implementation exists).
