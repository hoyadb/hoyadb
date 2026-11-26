---
id: DEF-0053
title: Active roadmap instructions require a private process guide
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-30
updated: 2026-10-30
parents: [SPEC-0046]
affects: [SPEC-0045, REQ-0048]
regression_for: [TC-0050]
tags: [review, regression]
---

# DEF-0053: Active roadmap instructions require a private process guide

## Symptom

The 2026-10-30 review recorded REL-04: Active roadmap workflow text still directs contributors to the private ai-governance TDD guide after public contribution checks were made self-contained.

## Acceptance

Point active workflow instructions to CONTRIBUTING.md and preserve historical references and accepted ADR bodies.

## Root cause and fix

Both active roadmap workflow links now target `CONTRIBUTING.md#change-workflow`.
Historical citations and accepted ADR bodies remain unchanged. The four public
checkout packaging tests passed, and the referenced contribution heading exists.

