---
id: SPEC-0012
title: Scoping and topology — membership-computed sync and the reset operation
type: spec
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-24
parents: [ARC-0001, ADR-0006, ADR-0015, ADR-0018]
satisfies: [REQ-0026, REQ-0027]
depends_on: [SPEC-0013]
tags: [sync, scoping, topology, membership]
---

# SPEC-0012: Scoping and topology — membership-computed sync and the reset operation

## Current entity sync contract

ADR-0033/API-0009 resolve bootstrap and reset's queue/lifetime interaction.
Bootstrap is bounded paged pull with visible committed partial data and explicit
readiness; page data, comparison state and opaque checkpoint commit atomically.
The client binds its store to an opaque scope and rejects stale requests;
authorization and membership calculation remain external. Default reset refuses
live recovery. Explicit DiscardRecovery clears local recovery/data atomically,
retains schema and identity counters, and retires old host callbacks/replies only
after commit. Reconfiguration requires an empty post-reset store. These replace
the illustrative drop/recreate description and those original open items below;
TC-0034 E11–E20 maps the current acceptance. Multi-tenant backend design stays open.

## Purpose

Specify the client-server topology hoyadb assumes, how server-side
membership rules determine what a client holds, and what happens when that
membership narrows.

## One authoritative backend, membership-declared scoping

hoyadb assumes **one authoritative backend**, not a symmetric mesh of equal
peers. What a client is entitled to hold is declared once, server-side, as
membership rules (e.g. "this account's data") and computed for that client —
**not** derived from which screens happen to be open at that moment. The
client sees exactly what the backend decides to expose.

Server-side, the pattern is: re-derive identity from the transport's own
auth (`SPEC-0008` §7), compute membership from it, return changes within it
plus a new opaque checkpoint:

```typescript
const identity = await authenticate(req);
const buckets = await computeMembership(identity);
const changes = await db.queryChangesSince(buckets, req.query.since);
```

Checkpoints are **opaque tokens** from the client's point of view — whether
they are per-table checkpoints, a global sequence number, or something else
is a backend implementation detail (`SPEC-0013`, `API-0001`).

## A real scaling constraint, carried forward

Membership computation must have explicit resource bounds and measured scaling limits.

## Membership revocation

When a client's membership narrows, hoyadb does **not**:

- detect the change (a developer-owned process — hoyadb does not poll for
  or infer it),
- selectively purge the now-out-of-scope records, or
- invoke the reset itself.

It exposes an explicit reset the developer invokes:

```typescript
async function onMembershipRevoked() {
  await db.reset(); // drops and recreates the local store
  // or: await db.reopen({ filename: "workspace-b.hoyadb" });
}
```

**If the developer does not invoke it, previously-synced data simply
persists.** hoyadb does not police its own data lifecycle — it is a library,
not an application that manages this on the developer's behalf. This is a
deliberate, recorded decision, carried as `THR-0004` at `accepted-risk`.

## Interaction with the offline queue

Not special-cased. A client offline with writes queued against data it is
about to lose scope over sends them normally once connectivity returns, and
**the backend's own authorization check** — the same membership rules — then
rejects whatever the revoked identity may no longer write, surfaced through
the ordinary rejection path (`SPEC-0008` §5), not a distinct mechanism.

## Same identity, multiple devices

Both devices see and hold the same membership-scoped data, since membership
is computed from identity, not from which device asked. Where an integration
genuinely needs to distinguish devices, that is a second field on the
trusted-context extension point (`SPEC-0008` §7) — a documented pattern, not
a hoyadb feature. `SPEC-0011` carries the conflict-resolution half of this
same answer.

## Open

- **The exact shape of a membership rule declaration** — a query language, a
  config file format, something else — is implementation-time detail, **not
  fixed**.
- **Bootstrap / initial sync is unspecified.** Everything here is
  incremental catch-up from a checkpoint; a fresh install with no local
  data — pagination for a large first sync, reactivity behaviour while it
  is in flight, interaction with the small-dataset footprint framing — is
  unaddressed anywhere (`RISK-0005`). Every first run hits this path.
- **What happens to queued writes at the moment of a reset is
  unaddressed** — the same gap `SPEC-0005` records for a migration reset.
- **Multi-tenant / multi-app backend scoping is unaddressed**: whether one
  backend serving several hoyadb apps, or staging versus production, needs
  tenant-scoped idempotency keys, command names or membership rules
  (`ADR-0023`).

## Traceability

- Parents: `ARC-0001`, `ADR-0006`, `ADR-0015`, `ADR-0018`.
- Satisfies: `REQ-0026`, `REQ-0027`.
- Depends on: `SPEC-0013`.
- Verified by: `TC-0013`.
