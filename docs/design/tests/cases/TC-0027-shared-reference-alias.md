---
id: TC-0027
title: Shared foreign-key aliases must agree on target and optionality
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [TP-0001]
verifies: [REQ-0002, REQ-0041]
depends_on: [SPEC-0026, API-0004]
regression_for: [DEF-0002]
tags: [schema, relations, regression]
---

# TC-0027: Shared-key aliases

```gherkin
Scenario: Matching aliases are valid but ambiguous targets are rejected
  Given customers and suppliers tables and orders with two to-one aliases sharing buyer_id
  When both aliases target customers with the same optionality
  Then the schema is valid and both aliases share one Reference column
  When an alias instead targets suppliers or changes optionality
  Then InvalidSchema is returned before any table or metadata mutation
```

## Execution — 2026-10-22

The [Phase 2 report](../../../implementation/phase-two-data-model.md) records
actual RED/GREEN observations, shared memory/native/browser execution and
limitations. This is an explicit Rust assertion mapping, not a claim that a
Gherkin runner executed this Markdown file.
