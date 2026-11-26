---
id: TC-0001
title: Schema declaration and first-class relations
type: test-case
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [TP-0001]
verifies: [REQ-0001, REQ-0002]
tags: [test, data-model, schema]
---

# TC-0001: Schema declaration and first-class relations

## Purpose

Verify that a declared schema produces typed tables with an implicit `id`,
that an undeclared table is rejected rather than created implicitly
(`REQ-0001`'s unwanted-behaviour clause), and that all three relation kinds
are navigable from either side without the application writing a join
(`REQ-0002`).

## Scenarios

See `TC-0001-schema-and-relations.feature`.

## Traceability

- Parent: `TP-0001`.
- Verifies: `REQ-0001`, `REQ-0002`.
