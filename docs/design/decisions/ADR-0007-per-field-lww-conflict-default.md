---
id: ADR-0007
title: Conflict resolution — per-field last-write-wins, deletes win, overridable per table
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, conflict-resolution, entity-diff]
---

# ADR-0007: Conflict resolution — per-field last-write-wins, deletes win, overridable per table

## Decision

Use per-field last-write-wins with deletes winning as the entity-diff default.
Allow a table-specific resolver to replace the default. Partial patches contain
the fields actually written rather than replacing an entire losing record.
ADR-0017 governs timestamp ties; SPEC-0011 defines conflict behavior.

## Consequences

A genuine concurrent write to one field can still lose one side. Last-write-wins
does not add counters or preserve arbitrary domain intent: use the command path
for those operations. Custom resolvers may preserve conflicting values for an
application decision. The override signature and available state belong to the
concrete protocol contract, not to this decision alone.
