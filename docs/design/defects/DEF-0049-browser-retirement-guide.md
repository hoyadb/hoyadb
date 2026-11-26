---
id: DEF-0049
title: Browser guide contradicts fresh-worker recovery
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-30
updated: 2026-10-30
parents: [SPEC-0046]
affects: [API-0011]
regression_for: [TC-0050]
tags: [review, regression]
---

# DEF-0049: Browser guide contradicts fresh-worker recovery

## Symptom

The 2026-10-30 review recorded SDK-6: The active integration guide says the default runtime cannot replace a retired worker, although the implementation and contract allow later fresh opens.

## Acceptance

Align active guidance with fresh-worker recovery, shared concurrent attempts and permanently closed old handles; preserve historical evidence.

## Root cause and fix

The active SDK integration guide and public platforms page now describe fresh
replacement attempts, shared concurrent opens, closed old handles and no replay.
`node --test packages/client/test/browser-retirement.test.mjs` passed its existing
QUALITY10 behavioral regression. Historical review evidence was not changed.

