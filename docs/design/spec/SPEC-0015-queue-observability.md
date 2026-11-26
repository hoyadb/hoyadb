---
id: SPEC-0015
title: Queue observability — per-command status, cancel, and retry backoff
type: spec
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-20
parents: [ARC-0001, ADR-0028]
satisfies: [REQ-0021, REQ-0022]
depends_on: [SPEC-0009, DM-0004]
tags: [observability, offline-queue]
---

# SPEC-0015: Queue observability — per-command status, cancel, and retry backoff

## Purpose

Specify what hoyadb tells the developer about its own offline queue, and how
that state stays bounded rather than growing forever. Written at lighter
depth than most of this tree, matching its source (`ADR-0028`, ADR-0028).

## Per-command status

Aggregate counts alone cannot distinguish "a note blocked on its parent
order" from an ordinary queued write, nor show a command's authoritative
outcome once it has left the queue. Both are resolved as **one extension of
the same call** — one place to look, not a second separately-named method to
reconcile against the first:

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

- `state` mirrors the `CommandOutcome` progression (`SPEC-0008` §4).
- `blockedOn` is present **only** for a command held `queued` pending
  another's unresolved local id, and carries the `local:`-prefixed id **the
  app's own code already holds** — not the blocking command's idempotency
  key, since that is the identifier the developer actually has on hand to
  correlate against.

Also observable live:
`db.sync.observeQueueStatus((s) => renderSyncIndicator(s));`

## Two retention lifecycles in one array

The `commands` array is **not** the queue itself, and cannot be: a command
is discarded from the queue the moment it is `acknowledged`, before or at
the same instant as its authoritative outcome becomes known. The reference
HTTP transport delivers receipt and outcome together (`API-0001`); other
transports may separate them. The array therefore spans:

1. entries still in the retry queue (`queued` / `sent`), and
2. a **bounded tail of recently-resolved** commands
   (`applied` / `corrected` / `rejected`), kept so a debugging workflow can
   still see them immediately after they leave the queue.

## Stuck and repeatedly-rejected commands

**No automatic cancellation.** Deciding a business-logic rejection is "done
trying" is a judgment hoyadb cannot make on the developer's behalf — the
same library-not-application posture as `SPEC-0012`'s revocation answer.
Instead, the status above makes a permanently-rejected command visible, and
an explicit cancel is offered:

```typescript
await db.sync.cancel("01J...Q4");
```

For a still-`queued` command it leaves the retry queue; for an
already-`rejected` one it clears the recently-resolved entry. Either way it
**rolls back the local optimistic mutation** — the same rollback path a
rejection uses (`SPEC-0008` §5) — and **cascades**: anything still
`blockedOn` the cancelled command is rolled back too.

**Cancelling a `sent`-but-unacknowledged command is a real, unclosed race.**
The backend may already have applied it, or may apply it after the client
has rolled back and stopped tracking it; hoyadb cannot retract a request in
flight. Clean on `queued` or permanently-`rejected`; **best-effort on
`sent`**.

## Retry backoff

Retries use **exponential backoff with jitter** — the policy being: do not
hammer a backend that is still recovering, especially when many clients
reconnect at once (`SPEC-0009`).

## Idempotency-key retention

The idempotency-key store is the **backend's own database**, so its
retention policy is the backend implementer's responsibility, not hoyadb's.
hoyadb requires only that a key stay valid at least as long as a client can
plausibly stay offline before reconnecting.

## Open

- **The concrete backoff curve** (initial delay, multiplier, cap) is not
  fixed — the policy is decided, the curve is configurable
  implementation-time detail.
- **Neither retention number is fixed**: not the recently-resolved window
  (how long, how many), nor the idempotency-key validity duration.
- **No attempt count or timing on an entry.** A command shows its `state`
  and optional `blockedOn`, but nothing says how long it has been there or
  how many retries it survived — **there is no field to threshold "stuck"
  on for a real support UI** beyond eyeballing `state`.
- **Diagnostic export** — a structured dump of local queue/sync state a
  developer could attach to a support ticket — is named as genuinely useful
  (the alternative today is asking a user to screen-share) but is **not
  designed**: no shape, no trigger, and **no redaction policy**
  (`THR-0007`). Genuinely deferred.
- **Whether entity-diff writes appear in this array, correlated by
  `recordId`, is not addressed** (`SPEC-0010`).

## Traceability

- Parents: `ARC-0001`, `ADR-0028`.
- Satisfies: `REQ-0021`, `REQ-0022`.
- Depends on: `SPEC-0009`, `DM-0004`.
- Verified by: `TC-0010`.
