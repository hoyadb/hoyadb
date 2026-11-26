---
id: OBJ-0002
title: One core implementation serves every supported platform
type: objective
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [VIS-0001]
tags: [architecture, portability]
---

# OBJ-0002: One core implementation serves every supported platform

## Statement

hoyadb's data model, storage dispatch, reactivity, and sync state machine
exist in exactly one implementation, reused unchanged on every target, so
client behaviour cannot drift between platforms.

## Baseline

n/a — new capability. The reference research (`legacy/references/`, summarized in
`legacy/references/comparison.md`) found every studied system shipping a single
shared core rather than per-platform reimplementations; WatermelonDB ships
one framework-agnostic JS API specifically to prevent that drift. hoyadb
matches that position with a different core language (`ADR-0001`).

## Target metric

Number of independent implementations of the core behaviour set (data
model, storage-interface dispatch, reactive query diffing, sync/offline
queue state machine). Target: **1**, with every platform binding being a
translation layer only. Measured by inspection against `ARC-0001` and
`ARC-0002`: a binding that reimplements any listed behaviour fails this
objective.

## Target date

Not fixed — see `OBJ-0001`'s note on `ADR-0008`.

## Rationale

`legacy/NEEDS.md`'s "Scope" section asks for a "generic core ... portable across
at least mobile and web, independent of any specific platform runtime,"
with platform wrappers around it. `legacy/VISION.md` lists "a separate
implementation per platform" first among the things hoyadb deliberately
avoids, because a sync protocol requires client and backend to agree on
behaviour, and N implementations is N chances for that agreement to break
silently.

## Traceability

- Parent: `VIS-0001`.
- Realized by: `REQ-0001`, `REQ-0002`, `REQ-0006`, `NFR-0005`.
- Risk against this objective: `RISK-0001` (the chosen core shape has no
  precedent in this project's own research).
