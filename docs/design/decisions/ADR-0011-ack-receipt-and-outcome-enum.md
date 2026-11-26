---
id: ADR-0011
title: Ack is durable receipt, separate from outcome, surfaced as one transport-agnostic enum
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, commands, reliability, transport]
---

# ADR-0011: Ack is durable receipt, separate from outcome, surfaced as one transport-agnostic enum

## Context and problem statement

The queue must distinguish a send attempt from durable acknowledgement and from the eventual authoritative outcome, so retries do not confuse receipt with acceptance (`ADR-0016`).

## Considered options

- **A durable queue of sent-but-unacknowledged commands, where an ack
  discards from the queue and its absence keeps the command queued, with
  the outcome surfaced to app code as one transport-agnostic status enum.**
- **Branch on transport-specific signals** — e.g. have app code read HTTP
  status codes directly.
- **Conflate ack and acceptance** — treat "the backend received it" and
  "the backend applied it" as one state.

## Decision outcome

In the context of knowing when a command may be discarded versus retried,
facing a transport hoyadb deliberately does not own, we decided for **a
durable queue keyed on acknowledgement, with the command's outcome exposed
as one transport-agnostic status enum** and against having app code branch
on transport-specific codes and against conflating receipt with acceptance,
to achieve a retry rule that works identically over any transport, accepting
that the wire-level signal for an ack is left entirely to each transport to
define.

### Consequences

**Good**
- Acknowledgement (durably received) and acceptance (the authoritative
  outcome) are tracked separately — a command can be acked and still be
  rejected once the backend actually runs it.
- Application code never branches on an HTTP status or any other
  transport-specific code: `CommandOutcome` is `queued` / `sent` /
  `acknowledged` / `applied` / `corrected` / `rejected`. ADR-0011's answer is
  explicit that "the developer map[s] the code to the enum / response
  object."
- The rule is simple and total: ack discards from the queue; no ack keeps
  it queued for retry.

**Bad**
- Two distinct shapes now exist and must be kept straight: the app-facing
  `CommandOutcome` carries no correlation field, while the transport-internal
  `WireOutcome` adds `idempotencyKey` (or `recordId`) so hoyadb's core can
  match an entry back to the right pending command. hoyadb's core strips
  the correlation field before the app sees an outcome.
- **A real, disclosed gap in the general contract**: for a transport where
  ack and outcome arrive *separately*, there is a window where a command is
  `acknowledged` — and therefore already discarded from the retry queue —
  but its outcome is not yet known, making it briefly invisible in the
  per-command status array entirely. The reference HTTP transport happens
  to collapse both into one reply, hiding this. **Not designed for**.
- Rollback is the necessary companion to a `rejected` outcome, decided
  separately in `ADR-0012`.

### Confirmation

Confirmed by `SPEC-0008` (the enum and the ack rule), `SPEC-0009` (the
queue acting on it), `API-0001` (how the reference HTTP transport signals
an ack), and the outcome test cases in `TP-0001`.

## Traceability

- Parent: `OBJ-0003`.
- Referenced by: `SPEC-0008`, `SPEC-0009`, `SPEC-0013`, `API-0001`.
