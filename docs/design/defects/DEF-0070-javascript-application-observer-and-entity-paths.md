---
id: DEF-0070
title: JavaScript application API mishandles entity links, observer errors and oversized receipts
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-10
updated: 2026-11-10
parents: [API-0011]
affects: [API-0012, SPEC-0033]
regression_for: [TC-0036, TC-0046]
tags: [javascript, bindings, review, regression]
---

# DEF-0070: JavaScript application API mishandles entity links, observer errors and oversized receipts

## Symptom

The application API sent ordinary `link`/`unlink` for entity-configured through tables, which core rejects without a captured context. Status, progress and outcome subscriptions delivered the terminal error event to the data callback. Disposal after close rejected; a recoverable data error without an error callback retired the listener; a subscription registered across a reset leaked in core; a close racing a finish reported a spurious background error; `execute` on a closed database rejected `InvalidHandle`. In the bindings crate, a server result that fits raw but not boxed made every later `pending_reconciliations` reply `LimitExceeded`, and batch preflight charged one allocated insert to every later read while re-serializing the accumulated report per write.

## Acceptance

Entity-configured through tables use the entity link path; terminal observer errors reach the background error sink, never a data callback; disposal after close resolves; recoverable errors keep their listener; no core subscription outlives a failed registration; a receipt that cannot be delivered alone is refused before the durable receipt; batch preflight stays an upper bound in one pass.

## Root cause and fix

Corrections are in `packages/client/src/application.js`, `client.js` and `crates/hoyadb-bindings/src/{reconciliation,commands,budget}.rs`, with regression cases in `packages/client/test/review-fixes.test.mjs` and `crates/hoyadb-bindings/tests/receipt_budget.rs`. `execute` on a closed database now rejects `Closed`, an error-kind change aligned with the API-0011 lifecycle. After rebuilding the WASM bridge, `npm test` reported 221 client and 11 testing cases passing and the React Native host suite 10.

Open: many individually valid pending outcomes can still exceed one unpaged reply; paging is a wire shape change that needs a decision.

## Follow-up — 2026-11-10

Aggregate overflow is closed by two additive read-only operations, `pending_reconciliations_page` and `queue_status_page` (request `{after}`, reply prefix plus `has_more`); the unpaged operations are unchanged. JavaScript, Swift and Kotlin call the unpaged operation first and walk pages only on `LimitExceeded`. The Swift and Kotlin loops were exercised with scripted native calls only; a multi-page queue status against the real core was not built. API-0011 §7 now states that `reconcileGuarded` sends the resolving operation, which core implements as the same path with the report dropped. The host schema pre-check is recorded in API-0012 as non-authoritative and pinned by a 34-case conformance suite against the real bridge: it never refuses a schema core accepts, and eight rules remain core-only.
