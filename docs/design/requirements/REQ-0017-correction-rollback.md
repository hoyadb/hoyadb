---
id: REQ-0017
title: Corrections and rejections reach the app through its subscriptions, with a rollback available
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, commands, error-handling, rollback]
---

# REQ-0017: Corrections and rejections reach the app through its subscriptions, with a rollback available

## Pattern

Event-driven, with an Unwanted-behaviour clause against a separate error
channel.

## Statement

WHEN a command's authoritative outcome is `corrected` or `rejected`, THE
system SHALL notify the application through the same
subscription/event/callback mechanism it uses for ordinary local data
changes, and SHALL expose a rollback operation that reverts the local
optimistic mutation that command produced.

THE system SHALL NOT route corrections down a separate, error-only channel,
and SHALL NOT decide the application's recovery behaviour — retrying,
surfacing an error, or reconciling some other way is the integrating
developer's responsibility.

WHEN a command is rolled back, THE system SHALL also roll back any command
still blocked on that command's unresolved local id (`REQ-0019`).

## Examples

Example 1 — handling both outcomes:

```typescript
useManageRollback("orders", order.id, (rollback, correction) => {
  if (correction.status === "corrected") {
    showToast(`Order total adjusted to ${correction.result.total_cents}`);
  } else if (correction.status === "rejected") {
    rollback();
    showToast(`Order could not be placed: ${correction.reason}`);
  }
});
```

Result: on `corrected`, the backend's differing total is accepted and
surfaced. On `rejected`, the optimistic order and its items are reverted.

Example 2 — cascade: `add-order-v1` is
rejected for insufficient stock. The `add-order-note-v1` command still
queued against that order's unresolved local id is rolled back too, rather
than left pointing at an id that will never resolve.

## Open

hoyadb provides the correction and rollback *mechanism*, not the recovery
*policy*. No cross-binding callback shape is fixed by this requirement —
the hook above is illustrative for one binding only (`ADR-0012`).

## Traceability

- Parent: `OBJ-0003`.
- Under: `ADR-0012`.
- Satisfied by: `SPEC-0008`, `SPEC-0009`, `UX-0001`.
- Verified by: `TC-0007`.
