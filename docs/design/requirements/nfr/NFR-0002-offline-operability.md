---
id: NFR-0002
title: Every documented local operation works with no network connection
type: nfr
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0001]
tags: [offline-first, availability]
---

# NFR-0002: Every documented local operation works with no network connection

## ISO/IEC 25010:2023 characteristic

Reliability (sub-characteristic: Availability), with Functional suitability
(Functional completeness) as a secondary characteristic — the bar is that
the *full* local operation set remains available offline, not a degraded
subset.

## Planguage

- **Tag**: offline-operability
- **Ambition**: An application keeps working normally with no connectivity
  — not in a reduced read-only mode, and not by queueing failures the user
  can see.
- **Scale**: Number of documented local operations that fail, or require
  degraded behaviour, when no network is available.
- **Meter**: The offline-path cases in `TP-0001`, run against the in-memory
  engine (`REQ-0034`) with a fake transport whose every `push` and `pull`
  rejects. The operation set measured is: schema declaration and migration
  (`REQ-0001`, `REQ-0006`), one-shot query (`REQ-0003`, `REQ-0004`,
  `REQ-0005`), live subscription (`REQ-0007`, `REQ-0008`, `REQ-0009`),
  command execute with optimistic apply (`REQ-0010`), entity-diff write
  (`REQ-0023`), and queue introspection (`REQ-0021`).
- **Past**: No comparison baseline is required; the offline acceptance target is absolute.
- **Must**: **0** operations fail.
- **Target**: **0** — identical to Must. This is a pass/fail property, not
  a figure to optimize; there is no meaningful "better than zero."

## Rationale

This is `VIS-0001`'s "Goal" made measurable: "reads and writes always go to
a local database first, and the app stays fully usable with no network
connection." `VIS-0001` rejects "sync that only covers reads, leaving
writes entirely to the app" precisely because it fails this bar.

Measurable without benchmark numbers — unlike `NFR-0001`, this is a
pass/fail property per operation, which is why it carries a real Must and
Target while `NFR-0001` does not.

## Traceability

- Parent: `OBJ-0001`.
- Satisfied by: `ARC-0001`, `SPEC-0009`.
- Verified by: `TC-0017`.
