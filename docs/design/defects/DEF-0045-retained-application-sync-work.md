---
id: DEF-0045
title: Application sync repeats mapping after admission contention
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-30
updated: 2026-10-30
parents: [SPEC-0046]
affects: [API-0011, API-0012]
regression_for: [TC-0050]
tags: [review, regression]
---

# DEF-0045: Application sync repeats mapping after admission contention

## Symptom

The 2026-10-30 review recorded SDK-2: A valid concurrent command makes reconcile Busy after the application mapper returns; retrying the pass invokes the mapper again for the same receipt.

## Acceptance

Retain the exact response and mapped proposal across Busy, use release-aware retries and cancel retained work on lifecycle invalidation.

## Root cause and fix

Application push previously retried the entire pass after core admission `Busy`,
losing the received response and re-invoking application mapping. It now reuses
`Database.resumeOwned` for prepare, receipt, context listing and reconciliation.
The receipt and encoded proposal survive admission retries; native release events
wake them without polling. Key and persisted creation context are captured before
calling the mapper, so mutating its argument cannot alter identity mapping facts.
The shared retry helper accepts application cancellation and unregisters its
release listener even while a native reply remains suspended.

## Verification

Real-WASM regressions in
[`application-review.test.mjs`](../../../packages/client/test/application-review.test.mjs)
first failed with `Busy` instead of completing/cancelling. They now cover receipt
and reconcile contention from another owner, same-owner background contention,
exact retained arguments despite later plan mutation, one mapper/transport call,
release during a suspended Busy reply, cancellation and immediate listener
retirement. A further failing regression caught mutable mapper context during
implementation and now passes. `npm test` passed 187 client and 11 testing-package
tests on the corrected source.
