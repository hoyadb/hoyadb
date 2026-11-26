---
id: TC-0013
title: Membership-scoped sync and the explicit reset operation
type: test-case
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-24
parents: [TP-0001]
verifies: [REQ-0026, REQ-0027]
tags: [test, sync, scoping, membership]
---

# TC-0013: Membership-scoped sync and the explicit reset operation

## entity sync coverage contract

ADR-0033/API-0009 resolve initial sync and reset recovery. TC-0034 E11/E13/
E16–E19 adds bounded bootstrap, partial progress, interrupted native/browser
recovery, total reset lifetimes and scope reconfiguration. The original lack
of bootstrap coverage stated below records the pre-entity sync boundary.

## Purpose

Verify that what a client holds comes from a server-computed membership and
an opaque checkpoint, never from UI state (`REQ-0026`), and that the reset
operation is total and **developer-invoked only** (`REQ-0027`).

## Coverage boundary, stated

This deliberately **does not** assert that revoked data is purged without a
reset — by decision, it is not (`THR-0004` at `accepted-risk`). One scenario
asserts exactly that non-behaviour, so the accepted risk is visible in the
suite rather than implied by absence.

**No bootstrap/initial-sync scenarios exist**, because that behaviour is
unspecified (`RISK-0005`).

## Scenarios

See `TC-0013-scoping-and-reset.feature`.

## Traceability

- Parent: `TP-0001`.
- Verifies: `REQ-0026`, `REQ-0027`.
