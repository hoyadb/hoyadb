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

The acceptance target is defined by this objective and its linked requirements.

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

Shared core semantics must run across mobile, web and desktop bindings. Keeping protocol behavior in one core reduces drift between independently implemented platform wrappers.

## Traceability

- Parent: `VIS-0001`.
- Realized by: `REQ-0001`, `REQ-0002`, `REQ-0006`, `NFR-0005`.
- Risk against this objective: `RISK-0001` (the chosen core shape has no
  a platform-independent proof of lifecycle correctness).
