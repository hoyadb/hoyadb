---
id: REQ-0012
title: A command travels as an envelope of name, arguments, keys and trusted context
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, commands, wire-format]
---

# REQ-0012: A command travels as an envelope of name, arguments, keys and trusted context

## Pattern

Ubiquitous, with an Unwanted-behaviour clause against sending local
mutations as the authoritative payload.

## Statement

THE system SHALL construct, for every command, an envelope carrying the
command's versioned name, its arguments, its idempotency key (`REQ-0013`),
the client's schema version (`REQ-0014`), and the trusted context
(`REQ-0015`), and SHALL attach the idempotency key, schema version and
trusted context itself rather than requiring the application's code to
populate them.

THE system SHALL NOT send the row mutations the local handler produced as
the command's authoritative payload. WHERE a client attaches its recorded
local execution log, THE system SHALL mark it as an optional debugging aid
the backend may ignore, never a substitute for the backend's own handler
run.

## Examples

Example 1 — the envelope for the walkthrough's order:

```json
{
  "command": "add-order-v1",
  "idempotencyKey": "01J8Z3K9F5G6H7J8K9M0N1P2Q3",
  "schemaVersion": 3,
  "args": { "customerId": "cust_123", "items": [{ "productId": "p1", "quantity": 2 }] },
  "context": {
    "identity": "opaque-server-verified-identity-value",
    "capturedAt": "2026-09-15T14:03:00.000Z"
  }
}
```

Result: the backend receives the *action* and its inputs. `capturedAt` is
whatever `ctx.now()` returned during the local run, captured into the
envelope by hoyadb's core without the developer's code doing anything —
which is what lets the backend's own `ctx.now()` reuse the client's value
rather than its own wall clock.

Example 2 — what is absent: the envelope above carries no `orders` row and
no `order_items` rows, even though the local handler inserted several. The
backend derives them by running its own handler.

## Open

The serialization format of `args` (structured JSON versus a typed binary
format) is implementation-time detail, not fixed. The concrete shape of the
optional debug execution-log attachment — a separate envelope field, a
debug-only endpoint, or something else — is **not fixed** either
(`ADR-0025`).

## Traceability

- Parent: `OBJ-0003`.
- Under: `ADR-0005`, `ADR-0025`.
- Satisfied by: `DM-0002`, `SPEC-0008`, `API-0001`.
- Verified by: `TC-0006`.
