---
id: REQ-0033
title: hoyadb's own tables and fields use a reserved namespace that cannot collide with an app schema
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0002]
tags: [versioning, compatibility, schema]
---

# REQ-0033: hoyadb's own tables and fields use a reserved namespace that cannot collide with an app schema

## Pattern

Ubiquitous, with an Unwanted-behaviour clause for a colliding declaration.

## Statement

THE system SHALL name every table, column and field it maintains for its
own bookkeeping — the offline queue's storage (`REQ-0018`) and the implicit
per-table columns (`REQ-0001`) among them — within a clearly namespaced,
reserved naming convention.

IF an application's declared schema uses a name within that reserved
namespace, THEN THE system SHALL reject the schema declaration rather than
allowing a collision.

## Examples

Example 1 — reserved prefix: hoyadb's bookkeeping uses a
fixed prefix — for example `__hoyadb_` — so the offline queue's reserved
table is `__hoyadb_queue`-shaped rather than something an application might
plausibly also declare.

Example 2 — collision rejected:

- Input: a schema declaring a table named `__hoyadb_queue`.
- Output: the schema declaration is rejected, rather than the application's
  table silently shadowing or corrupting hoyadb's own state.

## Open

**The exact reserved prefix string is not fixed** — implementation-time
detail. The *policy* (a clearly namespaced reserved convention, enforced)
is what is decided here (`ADR-0023`).

## Traceability

- Parent: `OBJ-0002`.
- Under: `ADR-0023`.
- Satisfied by: `SPEC-0003`, `SPEC-0009`.
- Verified by: `TC-0003`.
