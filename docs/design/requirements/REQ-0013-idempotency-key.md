---
id: REQ-0013
title: Every command carries a client-generated, time-sortable idempotency key, reused on retry
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
mitigates: [THR-0003, THR-0008]
tags: [sync, commands, idempotency]
---

# REQ-0013: Every command carries a client-generated, time-sortable idempotency key, reused on retry

## Pattern

Ubiquitous, with an Unwanted-behaviour clause against minting a new key on
retry.

## Statement

THE system SHALL generate, entirely on the client and without network
access, a time-sortable identifier for every command and attach it to that
command's envelope.

IF a command already in the queue is retried, THEN THE system SHALL resend
the exact same envelope with the same idempotency key, and SHALL NOT mint a
new one.

THE system SHALL use that key to correlate a per-write outcome back to the
pending local command (`REQ-0029`).

THE system SHALL NOT require any backend to deduplicate against the key —
whether a given backend does so is that backend's own implementation
choice.

## Examples

Example 1 — the key on the wire
(`legacy/docs/06-sync/command-protocol.md`): `"idempotencyKey":
"01J8Z3K9F5G6H7J8K9M0N1P2Q3"` — a ULID-shaped, time-sortable value, present
on every command envelope.

Example 2 — retry after a lost response
(`legacy/docs/06-sync/offline-queue.md`):

- Given `add-order-v1` with key `01J...Q3` was sent and the response was
  lost to a network failure, leaving it `sent` and eligible for retry.
- When the retry fires.
- Then the identical envelope, still carrying key `01J...Q3`, is resent —
  which is what lets a backend that does deduplicate recognize it as the
  same command rather than a second order.

Example 3 — what an entity-diff does instead: an entity-diff write carries
**no** idempotency key, because create/update/delete against a specific
`recordId` is naturally idempotent to resend (`REQ-0023`). Its outcome is
correlated by `recordId`.

## Open

**Deduplication is the backend's choice, not hoyadb's guarantee**, so
double-apply is reduced rather than eliminated (`THR-0003`). The
idempotency-key store is the backend's own database and its retention
policy is the backend implementer's responsibility; hoyadb only requires a
key stay valid as long as a client can plausibly stay offline — **neither
duration is fixed** (`ADR-0028`).

## Traceability

- Parent: `OBJ-0003`.
- Under: `ADR-0010`.
- Satisfied by: `DM-0002`, `SPEC-0008`, `SPEC-0009`.
- Verified by: `TC-0006`, `TC-0009`.
