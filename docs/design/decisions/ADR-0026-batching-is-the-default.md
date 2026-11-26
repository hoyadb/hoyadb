---
id: ADR-0026
title: The reference transport always sends an array; each write resolves independently
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, transport, batching]
---

# ADR-0026: The reference transport always sends an array; each write resolves independently

Source: `legacy/PLAN_DECISIONS.md` D18, folded into
`legacy/docs/06-sync/transport-http-example.md`,
`legacy/docs/06-sync/entity-diff-fallback.md` and
`legacy/docs/06-sync/implementation-guide.md`.

## Context and problem statement

A client reconnecting after being offline with many queued writes sent them
as N separate requests, with nothing stated beyond "that's probably what
happens." Three docs had explicitly punted on this.

## Considered options

- **One endpoint that always takes an array, with each entry independently
  resolved and outcomes correlated by idempotency key; batching is the
  default, not an opt-in.**
- **All-or-nothing batch semantics** — the whole array succeeds or rolls
  back together.
- **Batching as an opt-in optimization**, with a separate single-write wire
  shape kept alongside it.

## Decision outcome

In the context of sending several queued writes after reconnecting, facing
writes that can have already-committed side effects, we decided for **one
array-taking endpoint with independent per-write resolution as the default
and only shape** and against all-or-nothing batch semantics and against
batching as an opt-in with a separate single-write shape, to achieve one
contract that stays correct rather than two paths that both must, accepting
that hoyadb makes no cross-write atomicity promise whatsoever.

### Consequences

**Good**
- The deciding constraint is stated plainly: all-or-nothing would mean
  promising cross-command atomicity hoyadb cannot back, since a write can
  have arbitrary already-committed side effects — a charged payment, a
  decremented stock count — by the time a later write in the same batch
  fails. Undoing that needs compensating-transaction logic hoyadb neither
  owns nor can guarantee.
- **There is no separate single-write wire shape**: a lone write is an
  array of length one. hoyadb's core has nothing to decide and a
  `Transport` implementation has nothing to switch between.
- Outcomes are correlated **by idempotency key** (or `recordId` for an
  entity-diff), not array position — a retry can resend only part of a
  previous array, so position is not a safe key.
- Ordering needs nothing elaborate: `ADR-0025` already guarantees a
  serialized local execution order, and a write blocked on an unresolved
  local id is never eligible to send, so a batch only ever contains writes
  sendable right now.
- Retry resends only what is still `sent`-but-unacknowledged, not the whole
  array blindly.
- A developer wanting one-request-per-write builds it in their own
  transport, calling the same endpoint once per envelope with a length-one
  array — **sequentially, not concurrently**, or dependent ordering breaks
  silently.

**Bad**
- **A known, unclosed correlation gap**: `recordId` alone cannot
  disambiguate two queued entity-diff writes to the *same* record landing
  in one array (two offline edits to different fields of one note). Both
  outcomes carry an identical `recordId` with nothing to tell them apart.
  Closing this needs a per-write correlation id for entity-diffs, which is
  **not designed**.
- This is a **reference-transport-level decision**, not a hoyadb-core
  guarantee — the same "example implementation, not an exclusive
  requirement" status the rest of the HTTP transport doc carries.
- An entry the backend could not resolve is indistinguishable, from the
  client's side, from never having been acknowledged: still `sent`, still
  eligible for retry.

### Confirmation

Confirmed by `API-0001` and `contracts/openapi.yaml` (the array-in/array-out
shape), `SPEC-0013` (the `Transport` interface), and the batching test
cases in `TP-0001`.

## Traceability

- Parent: `OBJ-0003`.
- Referenced by: `API-0001`, `SPEC-0013`, `SPEC-0009`, `SPEC-0010`.
- Depends on: `ADR-0025` (the local execution order), `ADR-0010`
  (correlation keys).
