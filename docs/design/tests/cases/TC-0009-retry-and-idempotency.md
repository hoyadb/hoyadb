---
id: TC-0009
title: Retry behaviour and idempotency-key reuse
type: test-case
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [TP-0001]
verifies: [REQ-0020, REQ-0013, THR-0003]
tags: [test, sync, retry, security]
---

# TC-0009: Retry behaviour and idempotency-key reuse

## Purpose

Verify that a retry resends the identical envelope with the **same**
idempotency key rather than minting a new one (`REQ-0013`), that retries
fire on both triggers with exponential backoff and jitter (`REQ-0020`), and
that only still-unacknowledged writes are resent.

## Coverage boundary, stated

This verifies the **client-side half** of `THR-0003`'s mitigation only.
Whether a backend actually deduplicates against the key is that backend's
own choice, which hoyadb neither mandates nor can test — which is exactly
why `THR-0003` sits at `mitigation-planned` rather than `mitigated`.

## Scenarios

See `TC-0009-retry-and-idempotency.feature`.

## Traceability

- Parent: `TP-0001`.
- Verifies: `REQ-0020`, `REQ-0013`, `THR-0003` (client-side half).
