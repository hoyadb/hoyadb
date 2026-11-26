---
id: REQ-0020
title: Unacknowledged writes are retried with exponential backoff and jitter
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0001]
tags: [sync, offline-queue, retry]
---

# REQ-0020: Unacknowledged writes are retried with exponential backoff and jitter

## Pattern

Event-driven, with an Unwanted-behaviour clause against a fixed interval.

## Statement

WHEN a new command is issued, THE system SHALL attempt to resend writes
still in the `sent`-but-unacknowledged or `queued`-and-eligible state.

WHEN the application explicitly requests a retry, THE system SHALL attempt
the same.

IF a retry attempt fails, THEN THE system SHALL schedule the next attempt
using exponential backoff with jitter, and SHALL NOT retry on a fixed
interval.

THE system SHALL resend only what is still unacknowledged, not an entire
previous batch unconditionally.

## Examples

Example 1 — developer-triggered retry on reconnect:

```typescript
window.addEventListener("online", () => {
  db.sync.retryPending();
});
```

Result: queued and unacknowledged writes are attempted, in local execution
order (`REQ-0011`).

Example 2 — automatic piggyback: issuing any new command triggers a retry
sweep, on the reasoning that an app issuing new writes is evidently
connected enough to try.

Example 3 — jitter's purpose: when a backend recovers from an outage, many
clients reconnect at once. Jitter spreads their retries rather than having
every client retry on the same schedule and re-overwhelm it.

## Open

**The concrete backoff curve — initial delay, multiplier, cap — is not
fixed.** The *policy* (back off, do not hammer a recovering backend) is
what is decided; the curve is a configurable, implementation-time value
(`ADR-0028`).

## Traceability

- Parent: `OBJ-0001`.
- Under: `ADR-0016`, `ADR-0028`.
- Satisfied by: `SPEC-0009`, `SPEC-0015`.
- Verified by: `TC-0009`.
