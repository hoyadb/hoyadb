---
id: REQ-0022
title: A developer can explicitly cancel a queued or rejected command; hoyadb never auto-cancels
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0001]
mitigates: [THR-0006]
tags: [observability, offline-queue, rollback]
---

# REQ-0022: A developer can explicitly cancel a queued or rejected command; hoyadb never auto-cancels

## Pattern

Event-driven, with an Unwanted-behaviour clause against automatic
cancellation.

## Statement

WHEN the application cancels a command by its idempotency key, THE system
SHALL remove it from the retry queue if it is still `queued`, SHALL roll
back the local optimistic mutation it produced, and SHALL cascade that
rollback to any command still blocked on it (`REQ-0019`).

IF a command is repeatedly rejected by the backend, THEN THE system SHALL
keep it visible through per-command status (`REQ-0021`) and SHALL NOT
cancel or discard it automatically.

## Examples

Example 1 — explicit cancel:

```typescript
await db.sync.cancel("01J...Q4");
```

Result: for a still-`queued` command, it leaves the retry queue and its
local optimistic mutation is reverted — the same rollback path a rejection
uses (`REQ-0017`). Anything `blockedOn` it is rolled back too.

Example 2 — an already-rejected command: it has already left the queue
(discarded on acknowledgement), so `cancel()` clears its entry from the
recently-resolved tail and rolls back its local mutation.

Example 3 — no auto-cancel: a command rejected ten times in a row stays
visible as `rejected` in `queueStatus()`. Deciding it is "done trying" is a
business judgment hoyadb has no basis to make on the developer's behalf —
the same library-not-application posture as `REQ-0027`.

## Open

**Cancelling a `sent`-but-unacknowledged command is a real, unclosed
race.** The backend may already have applied it, or may apply it after the
client has rolled back and stopped tracking it; hoyadb cannot retract a
request already in flight. `cancel()` is clean on a `queued` or
permanently-`rejected` command and **best-effort on a `sent` one**
(`ADR-0028`).

## Traceability

- Parent: `OBJ-0001`.
- Under: `ADR-0028`.
- Satisfied by: `SPEC-0015`.
- Verified by: `TC-0010`.
