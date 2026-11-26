---
id: TC-0004
title: One-shot reads, live subscriptions, and observation granularity
type: test-case
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [TP-0001]
verifies: [REQ-0007, REQ-0008, REQ-0009]
tags: [test, reactivity, subscriptions]
---

# TC-0004: One-shot reads, live subscriptions, and observation granularity

## Purpose

Verify that both modes exist on one query object (`REQ-0007`), that the
default re-emits **only** on set-membership change — the negative case
being the load-bearing one (`REQ-0008`) — and that both opt-ins change
granularity only when explicitly requested (`REQ-0009`).

## Scenarios

See `TC-0004-subscriptions.feature`.

## Traceability

- Parent: `TP-0001`.
- Verifies: `REQ-0007`, `REQ-0008`, `REQ-0009`.

## reactivity execution
