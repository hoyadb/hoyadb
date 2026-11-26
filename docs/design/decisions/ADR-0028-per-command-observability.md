---
id: ADR-0028
title: Observability — per-command status, manual cancel, backoff with jitter, backend-owned key retention
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0001]
tags: [observability, offline-queue, retry]
---

# ADR-0028: Observability — per-command status, manual cancel, backoff with jitter, backend-owned key retention

Source: `legacy/PLAN_DECISIONS.md` D20, folded into
`legacy/docs/08-non-functional/observability.md` and
`legacy/docs/06-sync/offline-queue.md`. Raised by the same gap-analysis review as
`ADR-0027`; written at lighter depth than D0–D18.

## Context and problem statement

Two independent reviews converged on "you can't tell what's happening to a
specific command in production." Three adjacent gaps surfaced in the same
round: `offline-queue.md`'s retry section never actually specified a
backoff, the worked backend example inserted into an `idempotency_keys`
table with no pruning story, and nothing said what happens to a command
that keeps getting rejected forever.

## Considered options

- **Extend the existing `queueStatus()` with a per-command array; no
  auto-cancellation, with an explicit `cancel()` instead; exponential
  backoff with jitter; idempotency-key retention left to the backend.**
- **A second, separately-named introspection method** alongside the
  aggregate one.
- **Auto-cancel** a command after N rejections.
- **A fixed retry interval** rather than backoff.

## Decision outcome

In the context of telling a developer what is happening to a specific
queued command, facing four related gaps in one area, we decided for
**extending `queueStatus()` with a per-command array, an explicit manual
`cancel()`, exponential backoff with jitter, and backend-owned key
retention** and against a second introspection method, auto-cancellation,
and a fixed retry interval, to achieve one place to look for queue state
and a policy that does not hammer a recovering backend, accepting that
several concrete values are left as implementation-time detail.

### Consequences

**Good**
- One call, not two to reconcile. Each entry carries the command's
  idempotency key, current state, and — for a `queued` entry blocked on a
  dependency — which local id it is waiting on, so "blocked on its parent"
  and "ordinary queued" are finally distinguishable. That was the concrete
  complaint.
- `blockedOn` reports the `local:`-prefixed id the app's own code already
  holds, not the blocking command's idempotency key — the identifier the
  developer actually has on hand.
- No auto-cancellation: deciding a business-logic rejection is "done
  trying" is a judgment hoyadb cannot make for the developer — the same
  library-not-application posture as `ADR-0015`. `cancel()` rolls back the
  local optimistic mutation and cascades to anything blocked on it.
- Backoff with jitter specifically so a recovering backend is not hammered
  by every reconnecting client at once.

**Bad**
- **`cancel()` on a `sent`-but-unacknowledged command is a real, unclosed
  race**: the backend may already have applied it, or may apply it after
  the client has rolled back and stopped tracking it. hoyadb cannot retract
  a request in flight. Clean on `queued` or permanently-`rejected`,
  best-effort on `sent`.
- The `commands` array spans two different retention lifecycles in one
  call — entries still in the retry queue, plus a bounded tail of
  recently-resolved commands kept for debugging. **The retention window —
  how long, how many — is not fixed.**
- **The concrete backoff curve (initial delay, multiplier, cap) is not
  fixed** — the *policy* is the decision, not the curve.
- **Idempotency-key retention is the backend implementer's
  responsibility**; hoyadb only requires a key stay valid as long as a
  client can plausibly stay offline. **Neither number is fixed.** A
  narrower answer than a full TTL scheme, flagged deliberately.
- **No attempt count or timing on an entry** — nothing says how long a
  command has been in its state or how many retries it survived, so there
  is no field to threshold "stuck" on for a real support UI.
- **Diagnostic export** (a structured dump attachable to a support ticket)
  is named as genuinely useful but **not designed**: no shape, no trigger,
  no redaction policy. Genuinely deferred, unlike the items above.

### Confirmation

Confirmed by `SPEC-0015` (queue observability) and the introspection and
cancel test cases in `TP-0001`.

## Traceability

- Parent: `OBJ-0001`.
- Referenced by: `SPEC-0015`, `SPEC-0009`.
- Depends on: `ADR-0016`, `ADR-0010`, `ADR-0012`.
