---
id: OBJ-0001
title: An application built on hoyadb stays fully usable with no network
type: objective
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [VIS-0001]
tags: [offline-first, availability]
---

# OBJ-0001: An application built on hoyadb stays fully usable with no network

## Statement

Every read and every write an application performs through hoyadb succeeds
with no network connection available, and no documented local operation
requires connectivity to complete.

## Baseline

n/a — new capability; no implementation exists to measure against (see
`legacy/docs/00-overview.md`'s "What this is not"). The comparison point named in
`README.md` is Realm/RxDB/WatermelonDB, all of which already meet this bar
for reads and writes — hoyadb does not get credit for a lower bar than the
prior art it was designed against.

## Target metric

Count of documented local operations (schema declaration, query one-shot,
query subscription, command execute, entity-diff write, queue
introspection) that require network connectivity to complete locally.
Target: **0**. Measured by the offline-path test cases in `TP-0001`, run
against the in-memory engine and a fake transport whose every send fails —
see `legacy/docs/09-testing.md`.

This is measurable without benchmark numbers: it is a pass/fail property
per operation, not a latency figure. Contrast `OBJ-0004`, where no
measurable target exists yet.

## Target date

Not fixed. Gated on the implementation roadmap in `ADR-0008`, whose first
step is a validation spike, not a feature release.

## Rationale

This restates `legacy/NEEDS.md`'s "Goal" as a checkable property: "reads and
writes always go to a local database first, and the app stays fully usable
with no network connection." `legacy/VISION.md` rejects "sync that only covers
reads, leaving writes entirely to the app" precisely because it fails this
objective outright.

## Traceability

- Parent: `VIS-0001`.
- Realized by: `REQ-0010`, `REQ-0018`, `REQ-0019`, `REQ-0020`, `REQ-0023`,
  `NFR-0002`, `NFR-0003`.
