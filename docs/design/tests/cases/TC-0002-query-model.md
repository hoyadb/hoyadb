---
id: TC-0002
title: Query predicates, relation traversal, ordering and limit
type: test-case
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [TP-0001]
verifies: [REQ-0003, REQ-0004, REQ-0005]
tags: [test, data-model, query]
---

# TC-0002: Query predicates, relation traversal, ordering and limit

## Purpose

Verify each operator in the catalogue `REQ-0003` fixes, that multiple
predicates combine as implicit AND, that a dot-path filters on a to-one
relation while a has-many traversal is rejected (`REQ-0004`), and that
ordering and limit behave as specified (`REQ-0005`).

## Scenarios

See `TC-0002-query-model.feature`.

## Traceability

- Parent: `TP-0001`.
- Verifies: `REQ-0003`, `REQ-0004`, `REQ-0005`.
