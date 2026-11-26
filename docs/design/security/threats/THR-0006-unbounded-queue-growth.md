---
id: THR-0006
title: Unbounded offline-queue growth exhausts local device storage
type: threat
status: mitigated
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [ARC-0001]
tags: [security, stride, denial-of-service]
---

# THR-0006: Unbounded offline-queue growth exhausts local device storage

## Description

Every write made while offline is persisted to the local queue
(`REQ-0018`). A client that stays offline for a long period, or whose
writes are all blocked on an unresolvable dependency, accumulates queued
writes with no natural bound — consuming device storage until the app or
the device itself degrades.

## STRIDE grid

| Category | Applies? | Notes |
| --- | --- | --- |
| Spoofing | No | Not relevant. |
| Tampering | No | Not relevant. |
| Repudiation | No | Not relevant. |
| Information Disclosure | No | Not relevant, though a larger queue widens `THR-0002`'s blast radius. |
| Denial of Service | Yes | The primary impact — local storage exhaustion, self-inflicted or attacker-driven. |
| Elevation of Privilege | No | Not relevant. |

## Target ASVS level

L2 (OWASP ASVS 5.0).

## Relevant ASVS items (referenced loosely)

- V11.1.x — verify that the application enforces limits on resource
  consumption per user or session.
- V12.x — verify that resource exhaustion conditions are handled without
  loss of control.

## Attack scenario

1. A client goes offline, or its backend becomes unreachable.
2. The application keeps issuing writes; each is applied optimistically and
   persisted to the queue (`REQ-0010`, `REQ-0018`).
3. A blocked dependency chain (`REQ-0019`) means some writes can never
   become eligible to send, so they accumulate rather than draining.
4. Queue growth is unbounded relative to available device storage — a
   particular concern given `NFR-0004`'s explicitly mobile-appropriate
   footprint target.

## Mitigation

- `REQ-0021` — hoyadb may reject further local writes once the pending or
  blocking backlog exceeds a configurable threshold
  (`configure({ sync: { maxPendingCommands: 500 } })`), rather than letting
  the queue grow without limit.
- `REQ-0021` also exposes aggregate and per-command queue state, so an
  application can surface the condition and act before the threshold is
  reached rather than discovering it as a hard failure.
- `REQ-0022` lets a developer explicitly cancel a command that will never
  resolve, draining the queue deliberately — hoyadb does not auto-cancel.
- Implemented by: `SPEC-0009`, `SPEC-0015`.

## Residual risk

- **The threshold's default value, and whether it is per-table or global,
  is not fixed** (`ADR-0016`). Only the existence of a configurable
  threshold is decided.
- **No attempt count or timing is exposed** on a queue entry, so an
  application cannot threshold "stuck" on anything more precise than the
  state value itself (`ADR-0028`).
- Rejecting writes at the threshold trades one failure for another: the
  application stops being able to write offline, which is a partial defeat
  of `OBJ-0001`. That trade is deliberate — degrading predictably beats
  exhausting the device.

## Traceability

- Parent: `ARC-0001`.
- Mitigated by: `REQ-0021`, `REQ-0022`.
- Verified by: `TC-0010`.
