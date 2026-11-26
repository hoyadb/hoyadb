---
id: DEF-0044
title: Explicit application sync survives lifecycle cancellation
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-30
updated: 2026-10-30
parents: [SPEC-0046]
affects: [API-0012]
regression_for: [TC-0050]
tags: [review, regression]
---

# DEF-0044: Explicit application sync survives lifecycle cancellation

## Symptom

The 2026-10-30 review recorded SDK-1: An explicit sync awaiting a never-settling transport or mapper survives reset, leaving explicitSync true and subsequent sync Busy.

## Acceptance

Cancel the SDK-owned wait on stop/reset/close, await owned work where required, suppress late results and let a successful reset synchronize again.

## Root cause and fix

The explicit application pass previously used a cancellation promise that never
settled and was absent from the reset/close wait list. `ApplicationDatabase.sync`
now retains its run and cancellation handle, checks the captured epoch before
pull, and clears only its own run. Stop cancels the SDK-owned wait; reset/close
await the explicit run as well as background push/pull before touching the owner.
Transport cancellation remains the application's responsibility.

## Verification

Real-WASM regression cases in
[`application-review.test.mjs`](../../../packages/client/test/application-review.test.mjs)
failed before the implementation: all six transport/mapper × stop/reset/close
cases timed out instead of rejecting `Closed`; disposal also ran before a held
explicit host call completed. They now pass, including late-result suppression,
subsequent explicit/background sync, and cancellation after push reaches pull.
`npm test` passed 187 client and 11 testing-package tests on the corrected source.
