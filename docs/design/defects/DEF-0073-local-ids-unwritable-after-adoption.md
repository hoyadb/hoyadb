---
id: DEF-0073
title: Default local IDs become unwritable after entity adoption
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-10
updated: 2026-11-10
parents: [API-0009]
affects: [API-0009, SPEC-0049]
regression_for: [TC-0034, TC-0053]
tags: [entities, review, regression]
---

# DEF-0073: Default local IDs become unwritable after entity adoption

## Symptom

The default ID generator issues `local:N` for ordinary inserts, while the entity
path rejects any `local:` identity without a producer record. A row inserted
before `configure_entities` could then no longer be replaced, patched or removed
through the public API (`UnknownLocalReference`).

## Acceptance

First adoption refuses, before any mutation, a configured table that already
holds a `local:` row. The default generator, the documented per-write rejection
and every persisted format are unchanged; seeded rows with other IDs still adopt.

## Root cause and fix

`refuse_reserved_local_rows` runs one limit-1 range query per configured table
after the live-recovery check and returns `Protocol`, the kind the neighbouring
store-state refusals use. Hosts must supply IDs or a generator before adoption
(API-0009, SDK integration guide).

`adoption_refuses_default_generated_local_ids_the_entity_path_rejects` and the
shared `adoption_refuses_reserved_local_rows` case (memory and SQLite) failed
with `Ok(())` before the change. Limit: adoption is synchronous, so an adapter
whose read does not complete on first poll skips the check and keeps only the
later per-write rejection; the browser SQLite path was not run.
