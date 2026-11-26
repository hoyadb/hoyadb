---
id: TC-0028
title: All authored descriptors validate nested names before open
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [TP-0001]
verifies: [REQ-0033, REQ-0041]
depends_on: [SPEC-0026, API-0004]
regression_for: [DEF-0003]
tags: [migrations, validation, regression]
---

# TC-0028: Nested migration operands

```gherkin
Scenario: An unselected edge cannot hide reserved or malformed nested names
  Given a managed version 1 store and an authored migration from 1 to 2
  And an AddTable declaration with a reserved column or a SetRelations declaration with a reserved name or foreign key
  When version 1 opens with that unselected migration descriptor
  Then MigrationFailure is returned before any catalog or row mutation
  And the recoverable open returns the unchanged engine for retry
```

## Execution — 2026-10-22

The [Phase 2 report](../../../implementation/phase-two-data-model.md) records
actual RED/GREEN observations, shared memory/native/browser execution and
limitations. This is an explicit Rust assertion mapping, not a claim that a
Gherkin runner executed this Markdown file.
