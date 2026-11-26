---
id: RISK-0002
title: Command determinism is a responsibility boundary, not a solved problem
type: risk
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, commands, riskiest-assumption]
---

# RISK-0002: Command determinism is a responsibility boundary, not a solved problem

## Description

`ADR-0005`'s whole optimistic-local/authoritative-server model depends on a
command's two handler runs — one on the client, one on the backend —
reaching compatible results. `ADR-0014` decides this is explicitly **not**
something hoyadb guarantees, verifies, or enforces: the handler is the
developer's own code on both sides.

The developer owns client/server handler compatibility. Moving that responsibility to the integration does not eliminate divergence risk.

## Category

Delivery and technical risk (non-security).

## Impact if it materializes

An integrating developer whose two handler implementations drift will see
the drift surface as an unexpected `corrected` or `rejected` outcome
(`REQ-0016`) — sometimes long after the fact, and with no diagnostic
pointing at the drift as the cause. In the worst case the client and
backend disagree persistently about what a command means, which is a
correctness failure in application data, not a hoyadb failure hoyadb can
detect.

The exposure is widest exactly where commands are most valuable: the
counter- and ordering-style merges that motivated `ADR-0005` in the first
place.

## What makes this worse than a documentation gap

**There is no client/server handler-drift testing story**, despite
determinism being the project's self-identified riskiest assumption.
`TP-0001` discloses this plainly: nothing addresses
contract/golden testing between a client handler and its independently
authored server counterpart — for example, replaying the optional debug
execution log against the server handler to catch drift. `REQ-0034`
provides hermetic testing of hoyadb's own behaviour, not of the developer's
two handlers agreeing.

## Response

**Accepted and transferred, not mitigated.** `ADR-0014` records the
transfer explicitly: hoyadb owns queueing, transport, optimistic apply and
triggering the re-run; it does not own handler logic, and it never inspects
handler internals.

What is offered instead of a guarantee:

- The `ctx.now()` convention demonstrates the recommended pattern — thread
  non-deterministic inputs through `ctx`, capture the client's value into
  the envelope as `context.capturedAt`, have the backend read it from there
  rather than its own wall clock (`REQ-0012`). Recommended, not enforced.
- Because handler logic is the developer's, the two sides may legitimately
  be written in different languages, kept compatible by the developer's own
  test suite or by translation.
- An embeddable scripting runtime that executes identically inside the Rust
  core and on the backend would remove the need for two implementations
  entirely. `ADR-0014` flags this as **worth researching, not adopted** —
  no such runtime is named or committed to.

## Traceability

- Parent: `OBJ-0003`.
- Arises from: `ADR-0005`.
- Decision to transfer: `ADR-0014`.
- Affected: `SPEC-0008`, `SPEC-0013`.
