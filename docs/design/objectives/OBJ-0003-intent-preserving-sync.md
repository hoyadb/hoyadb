---
id: OBJ-0003
title: Writes reach the backend as intent, resolved authoritatively server-side
type: objective
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [VIS-0001]
tags: [sync, commands]
---

# OBJ-0003: Writes reach the backend as intent, resolved authoritatively server-side

## Statement

A write with real domain logic travels to the backend as a named action
with its arguments and a trusted identity — not as the set of rows it
happened to produce locally — and the backend's independent re-run of that
action determines the final result.

## Baseline

n/a — new capability. Of the eight systems researched, six use a pure
entity-diff model (`legacy/docs/06-sync/overview.md`); Zero's mutators are the
only close precedent for the command shape, and the reference comparison
records them as "closer to typed RPC with local prediction than true
business intent." The baseline for *domain-level* commands specifically is
therefore: no precedent in the research set.

## Target metric

Two checkable, pass/fail properties:

1. A command's wire envelope contains its name, arguments, idempotency key,
   schema version, and trusted context — and does **not** carry the row
   mutations the local handler produced as its authoritative payload. An
   optional debug-only execution log is permitted and is never a
   substitute (`ADR-0025`). Measured against `DM-0002`.
2. Every command outcome an application observes originates from the
   backend's own handler run, not from the local optimistic apply.
   Measured by the command-outcome cases in `TP-0001`.

## Target date

Not fixed — see `OBJ-0001`'s note on `ADR-0008`.

## Rationale

`legacy/NEEDS.md`'s "Sync mechanism" section states the preference directly: "the
goal being to preserve *why* a change happened (context/intent), not just
the resulting state, so the backend can apply its own logic/validation
around that intent." `legacy/VISION.md` rejects "treating plain state-diffing as
the only way to sync a write" for the counter and ordered-list cases where
value diffing structurally fails.

## Traceability

- Parent: `VIS-0001`.
- Realized by: `REQ-0010`, `REQ-0011`, `REQ-0012`, `REQ-0013`, `REQ-0014`,
  `REQ-0016`, `REQ-0017`, `REQ-0024`, `REQ-0025`, `REQ-0026`.
- Risk against this objective: `RISK-0002` (command determinism is a
  responsibility boundary, not a solved problem).
