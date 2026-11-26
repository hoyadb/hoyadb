---
id: DEF-0054
title: Application pending reconciliation listing races with settlement
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-30
updated: 2026-10-30
parents: [SPEC-0046]
affects: [API-0012]
regression_for: [TC-0050]
tags: [javascript, concurrency, regression]
---

# DEF-0054: Pending reconciliation context race

## Symptom and cause

While testing DEF-0045, polling application `pendingReconciliations()` during
background settlement returned `Protocol`: it first obtains a pending list and
then requests advisory context for each key. A key can settle between those
calls. The current `reconcile()` wrapper already treats context as advisory,
but listing does not handle this normal concurrent transition.

## Acceptance and regression gap

Add a deterministic list/context interleaving before implementation. A key that
has settled by the context read must not make the pending listing fail; preserve
other pending entries. Do not swallow genuine context errors for still-pending
keys or lifecycle invalidation. Exercise both the public listing and automatic
sync's use of it, with the real WASM facade.

## Fix and verification

`pendingReconciliations` now treats context as advisory only after confirming
that the key is absent from a fresh core pending list. Still-pending `Protocol`
and other errors propagate. Owner/generation guards before and after the reads
prevent reset or close from being mistaken for ordinary settlement. Core remains
the authority; no host recovery cache or new wire operation was added.

## Verification

Deterministic real-WASM tests settle one of two pending keys immediately before
its context read. Before the fix the public listing rejected `Protocol`, and
background sync reported `Protocol` and failed to map the remaining receipt.
Both now pass and preserve the second entry. Fault-injection cases preserve
`Protocol` for a still-pending key, `BackendFailure` and actual owner close.
A separate failing reset interleaving now rejects `Closed` rather than returning
an empty list. See
[`application-review.test.mjs`](../../../packages/client/test/application-review.test.mjs).
`npm test` passed 187 client and 11 testing-package tests on the corrected source.
