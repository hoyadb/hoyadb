---
id: REQ-0016
title: A command's outcome is surfaced as one transport-agnostic status, with ack distinct from acceptance
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, commands, reliability]
---

# REQ-0016: A command's outcome is surfaced as one transport-agnostic status, with ack distinct from acceptance

## Pattern

Event-driven, with an Unwanted-behaviour clause against transport leakage.

## Statement

WHEN a command's state changes, THE system SHALL surface that state to the
application as one transport-agnostic status value drawn from a fixed set:
`queued`, `sent`, `acknowledged`, `applied`, `corrected`, `rejected`.

THE system SHALL treat acknowledgement (the backend durably received the
command) as distinct from acceptance (the backend's authoritative outcome)
— a command MAY be acknowledged and still rejected once actually run.

WHEN a command is acknowledged, THE system SHALL discard it from the retry
queue; while it is unacknowledged, THE system SHALL keep it queued for
retry (`REQ-0020`).

IF a transport signals an outcome with a transport-specific code, THEN THE
system SHALL map it to the status set above before the application observes
it, and SHALL NOT require application code to branch on that code.

## Examples

Example 1 — the app-facing status set:

```typescript
type CommandOutcome =
  | { status: "queued" } | { status: "sent" } | { status: "acknowledged" }
  | { status: "applied", result: unknown }
  | { status: "corrected", result: unknown }
  | { status: "rejected", reason: string };
```

Result: an application checking `outcome.status === "rejected"` behaves
identically whether the transport underneath was HTTP, WebSocket, or a
test fake. `applied`'s `result` carries whatever the backend handler
returned — e.g. the real order id (`REQ-0019`).

Example 2 — ack without acceptance: `add-order-v1` is acknowledged (the
backend durably received it), then the backend's stock check fails and its
outcome is `rejected`. Both happen; the first did not predict the second.

Example 3 — the wire shape is separate: a batch response entry is a
`WireOutcome`, adding `idempotencyKey` (or `recordId`) so hoyadb's core can
match it to the right pending command. The core strips that correlation
field before the application sees a `CommandOutcome`.

## Open

**A real, disclosed gap**: for a transport where ack and outcome arrive
*separately*, a command can be `acknowledged` — and so already discarded
from the retry queue — before its outcome is known, making it briefly
invisible in the per-command status array (`REQ-0021`). The reference HTTP
transport happens to deliver both in one reply, hiding this. **Not designed
for**.

## Traceability

- Parent: `OBJ-0003`.
- Under: `ADR-0011`.
- Satisfied by: `SPEC-0008`, `SPEC-0009`, `API-0001`.
- Verified by: `TC-0007`.
