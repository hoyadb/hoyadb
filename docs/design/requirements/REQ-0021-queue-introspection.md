---
id: REQ-0021
title: Queue state is introspectable in aggregate and per command
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0001]
mitigates: [THR-0006]
tags: [observability, offline-queue]
---

# REQ-0021: Queue state is introspectable in aggregate and per command

## Pattern

Ubiquitous, with an Event-driven clause for a backlog threshold.

## Statement

THE system SHALL expose the offline queue's state to the application both
in aggregate (counts of pending and sending writes, and whether writes are
currently blocked) and as a per-command array, from one call rather than
two separately-named methods.

THE system SHALL include, for each entry, its idempotency key and current
state, and SHALL include the local id an entry is waiting on where that
entry is `queued` because of a dependency (`REQ-0019`).

THE system SHALL make queue state observable as a live subscription, not
only as a one-shot read.

WHEN the pending or blocking backlog exceeds a configurable threshold, THE
system MAY reject further local writes rather than letting the queue grow
unbounded.

## Examples

Example 1 — introspection (`legacy/docs/06-sync/offline-queue.md`):

```typescript
const status = await db.sync.queueStatus();
// {
//   pending: 3, sending: 1, blocked: false,
//   commands: [
//     { idempotencyKey: "01J...Q3", state: "sent" },
//     { idempotencyKey: "01J...Q4", state: "queued", blockedOn: "local:a1b2c3" },
//     { idempotencyKey: "01J...Q2", state: "rejected", reason: "..." },
//   ],
// }
```

Result: the second entry is distinguishable as "blocked on its parent"
rather than merely queued — which was the concrete gap this closes.
`blockedOn` reports the `local:`-prefixed id the app's own code already
holds, not the blocking command's idempotency key.

Example 2 — a sync-status indicator:
`db.sync.observeQueueStatus((status) => renderSyncIndicator(status));`

Example 3 — backlog cap:
`configure({ sync: { maxPendingCommands: 500 } });`

## Open

- The `commands` array spans **two different retention lifecycles**:
  entries still in the retry queue, plus a bounded tail of recently-resolved
  commands kept for debugging after they leave the queue. **The retention
  window — how long, how many — is not fixed.**
- The backlog threshold's **default value, and whether it is per-table or
  global, is not fixed.**
- **No attempt count or timing is exposed** on an entry, so there is no
  field to threshold "stuck" on for a real support UI.
- The array is documented in command terms only; **whether an entity-diff
  write gets equivalent visibility, correlated by `recordId`, is not
  addressed** (`legacy/docs/06-sync/entity-diff-fallback.md`).

## Traceability

- Parent: `OBJ-0001`.
- Under: `ADR-0028`.
- Satisfied by: `SPEC-0015`, `SPEC-0009`.
- Verified by: `TC-0010`.
