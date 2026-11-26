---
id: REQ-0024
title: Entity-diff conflicts resolve per field, with deletes winning and a per-table override
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, conflict-resolution]
---

# REQ-0024: Entity-diff conflicts resolve per field, with deletes winning and a per-table override

## Pattern

Event-driven, with an Unwanted-behaviour clause against whole-record
resolution, and an Optional-feature clause for the override.

## Statement

WHEN two entity-diff writes disagree on the same record, THE system SHALL
resolve the conflict independently for each individual field, taking the
most recently written value per field.

WHEN a delete and a later update target the same record, THE system SHALL
let the delete win.

THE system SHALL NOT resolve a conflict at whole-record granularity, where
the losing write's untouched fields would be silently discarded.

WHERE a table declares its own resolution logic, THE system SHALL apply
that logic instead of the default for that table.

THE system SHALL NOT apply conflict resolution to the command path, whose
backend re-run is itself the resolution (`REQ-0010`).

## Examples

Example 1 — field-granular resolution: two offline clients edit one
`order_notes` record — client A changes `body`, client B changes a
different field. Both changes survive, because each field resolves
independently. Under whole-record LWW, one client's edit would vanish.

Example 2 — delete beats a later update: client A deletes `note_789`;
client B, offline, updates `note_789.body` afterwards. The delete wins; the
record stays deleted.

Example 3 — where the default is the wrong tool: two users increment the
same counter field in the same window. Per-field LWW makes both see their
own value overwritten rather than summed. **This is a real loss the default
accepts.** A table with that merge need should use a command
(`REQ-0010`), not a per-table override — this is the counter gap that
motivates the command path existing at all.

## Open

The exact shape of the per-table override hook — its signature, and what
state it receives (both candidate writes? the record's full history?) — is
**not fixed** (`ADR-0007`). Identical-timestamp ties are resolved
separately (`REQ-0025`).

## Traceability

- Parent: `OBJ-0003`.
- Under: `ADR-0007`.
- Satisfied by: `SPEC-0011`.
- Verified by: `TC-0011`.
