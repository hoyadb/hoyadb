---
id: TC-0016
title: In-memory engine, fake transport and deterministic injection
type: test-case
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-24
parents: [TP-0001]
verifies: [REQ-0034]
depends_on: [ADR-0035, TC-0036]
tags: [test, testing-support]
---

# TC-0016: In-memory engine, fake transport and deterministic injection

## Purpose

Verify the three affordances `REQ-0034` requires — the in-memory engine
behaves identically above the storage interface, the fake transport
implements the same contract as a real one, and injected clock and id
generator make results deterministic.

This case is somewhat self-referential: the other sixteen cases in
`TP-0001` all depend on these affordances, so this one verifies the
foundation they stand on.

## Coverage boundary, stated

`REQ-0034`'s two disclosed gaps — **no client/server handler-drift testing
story** and **no storage-engine conformance suite** — have no scenarios,
because neither is designed (`RISK-0002`, `ADR-0003`).

## Scenarios

See `TC-0016-testing-support.feature`.

ADR-0035 explicitly supersedes the historical command-ID example on 2026-10-24:
commands inject exactly `local:test-fixed-id-001`, preserving recovery guards;
ordinary CRUD still injects exactly `test-fixed-id-001`. The feature retains its
historical wording and labels the two current acceptance scenarios explicitly.

## Traceability

- Parent: `TP-0001`.
- Verifies: `REQ-0034`.
