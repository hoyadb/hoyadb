---
id: DEF-0069
title: Durable alias resolution ignores the reference table and the retained mode
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-10
updated: 2026-11-10
parents: [SPEC-0049]
affects: [SPEC-0049, API-0011]
regression_for: [TC-0053]
tags: [entities, aliases, review, regression]
---

# DEF-0069: Durable alias resolution ignores the reference table and the retained mode

## Symptom

Reference cells were resolved through aliases without their target table, so a canonical identity in one table could be rewritten through another table's alias source with the same literal. Separately, ordinary `configure_entities` on a store opted into durable aliases succeeded silently, although SPEC-0049 requires a reopened store to verify its retained mode.

## Acceptance

A Reference cell with one declared to-one relation resolves through that relation's target table; ordinary configuration of a durable store returns `InvalidCommand`, on memory and SQLite.

## Root cause and fix

`entity_fields` takes the written table and resolves to-one cells through the target table; the mode check compares the durable flag in both directions. Command reference paths remain table-independent because `CommandDefinition.reference_paths` carries no table. Conformance cases `reference_cells_resolve_through_their_target_table` and `ordinary_configuration_rejects_retained_durable_mode` failed on memory before the change and pass on both adapters. A batch now computes its live closure once (`c17_incremental_live_closure_matches_full_recomputation`), a structural change with no behavior difference.
