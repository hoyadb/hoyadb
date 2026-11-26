---
id: NFR-0001
title: Local operation responsiveness — no numeric target is set yet
type: nfr
status: draft
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0004]
tags: [performance, open-question]
---

# NFR-0001: Local operation responsiveness — no numeric target is set yet

## ISO/IEC 25010:2023 characteristic

Performance efficiency (sub-characteristic: Time behaviour).

## Planguage

- **Tag**: local-operation-responsiveness
- **Ambition**: An application built on hoyadb does not feel slow — local
  reads, subscription updates and optimistic command applies happen fast
  enough that a user does not wait on them.
- **Scale**: **Not yet defined.** No operation set, no measurement point
  and no unit have been fixed. See "Why this is empty" below.
- **Meter**: **Not yet defined.** No benchmark methodology exists — which
  operations, which local dataset sizes, on which platform/engine
  combination are all open.
- **Past**: **Unknown.** No implementation exists to measure.
- **Must**: **Not set.**
- **Target**: **Not set.**
- **Wish**: Once a baseline exists, "stay at or improve on that baseline"
  becomes the working target (`ADR-0022`).

## Why this is empty

This artifact defines a qualitative objective. Numeric targets require measured data and an explicit methodology; missing figures must not be invented.

Applications should remain responsive under their supported local workloads. Sync, reactivity and command processing add real work beyond the underlying storage engine.

Its status is `draft`, not `accepted`: an NFR without Scale/Meter/Target
does not meet the Specify-phase exit criteria in
`ai-governance/framework/01-lifecycle.md`, and `accepted` would
misrepresent it. This also keeps it correctly outside the no-orphan rule's
coverage requirement, since there is no measurable bar for a test case to
verify yet.

## What must happen before this can carry numbers

1. An implementation exists far enough to benchmark (gated on `ADR-0008`).
2. A benchmark methodology is designed (`ADR-0022`).
3. The resulting figures become Past, and Must/Target are set from them.

## Traceability

- Parent: `OBJ-0004`.
- Under: `ADR-0022`.
- Satisfied by: nothing yet — deliberately. There is no measurable bar for
  a design artifact to satisfy.
- Verified by: nothing yet — deliberately, for the same reason.
