---
id: TC-0011
title: Entity-diff payloads and conflict resolution
type: test-case
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-24
parents: [TP-0001]
verifies: [REQ-0023, REQ-0024, REQ-0025]
tags: [test, sync, entity-diff, conflict-resolution]
---

# TC-0011: Entity-diff payloads and conflict resolution

## entity sync coverage contract

TC-0034 E01/E03/E04/E07–E10 extends this original case list. It covers writeId
correlation for repeated record IDs and deterministic two-client sequences
through a backend first-arrival fixture. That fixture tests client convergence;
it does not establish authorization or transactional correctness for an
integrator's backend. The original absence statements below are historical.

## Purpose

Verify that an ordinary table mutation generates a partial-patch payload
under the same authorization bar as a command (`REQ-0023`), that per-field
last-write-wins resolves at field granularity with deletes winning
(`REQ-0024`), and that the client attaches a timestamp but attempts no local
tie-break (`REQ-0025`).

## Coverage boundary, stated

`REQ-0025`'s **server-side** tie-break — first received wins — is a backend
recommendation hoyadb documents but does not run, and has **no scenario
here**. Only the client-side half is testable within hoyadb.

The known `recordId` correlation collision for two queued writes to the same
record in one batch has **no scenario**, because closing it is undesigned
(`REQ-0029`, `SPEC-0010`).

## Scenarios

See `TC-0011-entity-diff-and-conflicts.feature`.

## Traceability

- Parent: `TP-0001`.
- Verifies: `REQ-0023`, `REQ-0024`, `REQ-0025`.
