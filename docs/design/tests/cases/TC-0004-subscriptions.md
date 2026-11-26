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

## Phase 3 execution — 2026-10-23

The [reactive implementation report](../../../implementation/phase-three-reactivity.md)
records shared memory/native/browser assertions and actual Swift/main-thread
callback execution, including behavioral RED/GREEN, error recovery, normalized
ordering, retained count deadlines and cancellation. Implementation `58101a9`
preserves these as permanent regressions. These are explicit Rust assertion
mappings, not a claim that a Gherkin runner executed this Markdown file.
