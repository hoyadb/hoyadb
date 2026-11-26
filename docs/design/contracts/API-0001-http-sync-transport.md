---
id: API-0001
title: Reference HTTP sync transport contract
type: api
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-26
parents: [ARC-0001, ADR-0026]
satisfies: [REQ-0012, REQ-0014, REQ-0016, REQ-0026, REQ-0029]
depends_on: [SPEC-0013, DM-0002, DM-0003]
tags: [api, http, contract, reference]
---

# API-0001: Reference HTTP sync transport contract

## entity sync compatibility note

The original reference examples below are retained as historical version 1.
The unpublished entity sync extension is specified by API-0009 and ADR-0033:
entity requests add `writeId` and `removedFields`, responses use the complete
`table`/`recordId`/`writeId` tuple, and bounded pull pages have `complete` plus
explicit per-field clocks. The original recordId-only and incremental-only gaps
are resolved for this extension. Integrators adopting entity sync must update both
ends; old readers must reject unknown ledger data rather than strip it.
The exact experimental HTTP reference schema is
[entity-sync-openapi.json](entity-sync-openapi.json); `openapi.yaml` describes the
original historical reference and is not the entity sync worker wire schema.

## Status: an example, not an exclusive requirement

This contract gives hoyadb's wire shape a concrete, runnable form. **It is a
reference implementation, not a requirement that rules out other carriers**
(WebSocket, gRPC, anything else). hoyadb itself never performs these calls —
the developer's own `Transport` implementation does (`SPEC-0013`,
`REQ-0028`).

This document provides one HTTP transport example. It does not prove that a second transport works against the same contract; a transport-neutral interface alone is not evidence of interoperability.

## Required retry deduplication

SPEC-0038 makes explicit the obligation existing at-least-once retries require:
for a repeated `idempotencyKey` or entity `writeId`, the backend MUST NOT
re-execute side effects. It MUST return the stored final outcome if one exists,
otherwise a receipt. This also enables an explicit outcome-redelivery request to
reuse the original envelope safely. Authentication and authorization remain the
backend's responsibility. This is a transport contract, not an implemented
reference server or a network operation performed by core.

## Endpoints

| Method | Path | Purpose | Requirements |
| --- | --- | --- | --- |
| POST | `/hoyadb/commands` | Push queued writes (commands and entity-diffs alike) | `REQ-0012`, `REQ-0029` |
| GET | `/hoyadb/changes` | Pull changes since a checkpoint | `REQ-0026` |

## POST /hoyadb/commands

**Always an array. There is no separate single-write wire shape** — a lone
queued write is an array of length one, which is the common case, not a
special one (`REQ-0029`).

Request body: an array of envelopes in the order they were locally applied
(`SPEC-0008` §3), each either a command envelope (`DM-0002`) or an
entity-diff payload (`DM-0003`). The two are distinguished by the presence
of `command` versus `table` + `op`.

```http
POST /hoyadb/commands HTTP/1.1
Content-Type: application/json
Authorization: Bearer <the developer's own auth token>

[
  {
    "command": "add-order-v1",
    "idempotencyKey": "01J8Z3K9F5G6H7J8K9M0N1P2Q3",
    "schemaVersion": 1,
    "args": { "customerId": "cust_123", "items": [{ "productId": "p1", "quantity": 2 }] },
    "context": { "identity": "opaque-server-verified-identity-value", "capturedAt": "2026-09-15T14:03:00.000Z" }
  },
  {
    "command": "add-order-note-v1",
    "idempotencyKey": "01J8Z3K9F5G6H7J8K9M0N1P2Q4",
    "schemaVersion": 1,
    "args": { "orderId": "order_123", "body": "Gift wrap" },
    "context": { "identity": "opaque-server-verified-identity-value", "capturedAt": "2026-09-15T14:03:01.000Z" }
  }
]
```

The two commands above are **independent** — the note is on a different,
already-synced order. A dependent pair cannot appear together, because a
command referencing another's unresolved local id is never transmitted at
all (`REQ-0019`).

Response: an array of per-entry outcomes, **correlated by `idempotencyKey`**
(or `recordId` for an entity-diff) — **never by array position**, since a
retry can resend only part of a previous array:

```http
HTTP/1.1 200 OK
Content-Type: application/json

[
  { "idempotencyKey": "01J8Z3K9F5G6H7J8K9M0N1P2Q3", "status": "applied", "result": { "id": "order_456", "total_cents": 4200 } },
  { "idempotencyKey": "01J8Z3K9F5G6H7J8K9M0N1P2Q4", "status": "applied", "result": { "id": "note_790" } }
]
```

### Independence, not atomicity

**Each entry is independently resolved as its own authoritative run.** An
array is a transport-level grouping, not a transaction. One entry failing
does not roll back or block the others — hoyadb does not promise cross-write
atomicity it cannot back, since an earlier entry can have committed real
side effects (a charged payment, a decremented stock count) before a later
one fails.

### Acknowledgement

**Receiving a response at all — regardless of any entry's `status` — is what
this reference transport treats as that entry's `acknowledged` state.** A
network failure or timeout with no response leaves every entry `sent` and
eligible for retry.

The same applies **per entry** to a `200` whose array is shorter than the
request: an entry the backend could not resolve is indistinguishable, from
the client's side, from never having been acknowledged — still `sent`, still
retryable. Correlating by key rather than position is exactly what makes
this safe.

### Status codes

The specific HTTP status code used per outcome is **this reference
transport's own convention**. hoyadb's client API surfaces each outcome's
`status` field as a `CommandOutcome`, never an HTTP status — application code
never branches on a transport-specific code (`REQ-0016`).

## GET /hoyadb/changes

```http
GET /hoyadb/changes?since=eyJvcmRlcnMiOiJjaGtwdF8xMjMifQ HTTP/1.1
Authorization: Bearer <the developer's own auth token>
```

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "checkpoint": "eyJvcmRlcnMiOiJjaGtwdF8xMjQifQ",
  "changes": [
    { "table": "orders", "op": "update", "recordId": "order_456", "changes": { "status": "shipped" } }
  ]
}
```

`since` and `checkpoint` are **opaque tokens** from the client's point of
view — their internal structure (per-table checkpoints, a global sequence
number, or anything else) is a backend implementation detail this contract
does not fix.

## Out of scope for this contract

- **What a developer builds server-side** — authenticating a request,
  dispatching command versus entity-diff, running the right logic, looping
  a batch — is covered by the supporting
  [sync implementation guide](../../guides/sync-implementation.md), outside this
  wire contract.
- **How membership/bucket computation is exposed over HTTP** — whether
  `/hoyadb/changes` takes bucket parameters or the backend derives them
  entirely from the auth token — is left to a given backend
  (`SPEC-0012`).
- **Bootstrap / initial sync**: only incremental catch-up from a checkpoint
  is specified. A fresh install with no checkpoint is unaddressed
  (`RISK-0005`).

## Open

- **Proving this contract works unmodified over a second transport is
  unstarted** — see "Status" above.
- **`recordId` cannot disambiguate two entity-diff writes to the same record
  in one array** — a known correlation gap, undesigned (`REQ-0029`).

## Traceability

- Parents: `ARC-0001`, `ADR-0026`.
- Satisfies: `REQ-0012`, `REQ-0014`, `REQ-0016`, `REQ-0026`, `REQ-0029`.
- Depends on: `SPEC-0013`, `DM-0002`, `DM-0003`.
- Verified by: `TC-0014`.
