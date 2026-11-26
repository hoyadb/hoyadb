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

The acceptance target is absolute offline behavior under the linked requirements.

## Target metric

Count of documented local operations (schema declaration, query one-shot,
query subscription, command execute, entity-diff write, queue
introspection) that require network connectivity to complete locally.
Target: **0**. Measured by the offline-path test cases in `TP-0001`, run
against the in-memory engine and a fake transport whose every send fails —
see `TP-0001`.

This is measurable without benchmark numbers: it is a pass/fail property
per operation, not a latency figure. Contrast `OBJ-0004`, where no
measurable target exists yet.

## Target date

Not fixed. Gated on the implementation roadmap in `ADR-0008`, whose first
step is a validation spike, not a feature release.

## Rationale

Reads and writes go to the local store first. Local operation must remain usable without a network connection; both reads and writes are included.

## Traceability

- Parent: `VIS-0001`.
- Realized by: `REQ-0010`, `REQ-0018`, `REQ-0019`, `REQ-0020`, `REQ-0023`,
  `NFR-0002`, `NFR-0003`.
