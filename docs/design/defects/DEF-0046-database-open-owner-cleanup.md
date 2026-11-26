---
id: DEF-0046
title: Failed database open retains an inaccessible owner
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

# DEF-0046: Failed database open retains an inaccessible owner

## Symptom

The 2026-10-30 review recorded SDK-3: The tagged database factory opens a retained memory owner before invalid transport validation fails; a subsequent dropMemoryStore returns Busy.

## Acceptance

Validate static sync options before owner acquisition and dispose any owner acquired by a later failed open without losing retryable cleanup state.

## Root cause and fix

The native owner was published before optional transport validation, and failed
startup had no cleanup. Open now captures the schema and validates/binds the
transport before native acquisition. Once acquired, the owner leaves the opening
reservation before startup; any startup exception awaits normal owner disposal.
A disposal failure on an explicitly constructed `Database` preserves its owner
for an explicit close retry, and prevents a replacement open until disposal.

## Verification and limit

The real-WASM retained-store regression first failed because reopening returned
`Busy`; it now reopens and drops the store successfully after invalid factory
options. Three additional lifecycle tests first failed and now verify validation
before native open, disposal after injected startup failure, and close retry after
an injected disposal failure. See
[`application-review.test.mjs`](../../../packages/client/test/application-review.test.mjs)
and [`lifecycle.test.mjs`](../../../packages/client/test/lifecycle.test.mjs).
`npm test` passed 187 client and 11 testing-package tests on the corrected source.

A simultaneous unexpected post-acquisition host startup failure and native close
failure remains a factory API limitation: the rejected factory cannot expose a
retry handle under the current return/error contract. Static configuration errors
now occur before acquisition; the direct `Database` API retains retryability.
No new public cleanup/error API or unbounded disposal retry was introduced.
