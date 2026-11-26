---
id: DM-0002
title: Command envelope — what travels for a domain-level write
type: data-model
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-20
parents: [ARC-0001, ADR-0005]
satisfies: [REQ-0012, REQ-0013, REQ-0014, REQ-0015]
tags: [data, sync, commands, wire-format]
---

# DM-0002: Command envelope — what travels for a domain-level write

The in-memory and on-the-wire shape of one command (`REQ-0012`).

## Fields

| Field | Type | Constraints | Classification (DATA-1) | Populated by |
| --- | --- | --- | --- | --- |
| `command` | string | required; carries an explicit version suffix, e.g. `add-order-v1` (`REQ-0014`) | Internal | Application, at `defineCommand` |
| `idempotencyKey` | string | required; client-generated, time-sortable (ULID-leaning); **reused verbatim on retry** (`REQ-0013`) | Internal | hoyadb core |
| `schemaVersion` | integer | required (`REQ-0014`) | Internal | hoyadb core |
| `args` | object | required; the command's validated arguments | **Confidential** — application domain data; retained locally only, never sent as telemetry (`REQ-0032`) | Application, at `execute()` |
| `context.identity` | opaque | required; already-authenticated, never inspected or validated by hoyadb (`REQ-0015`) | **Confidential** | hoyadb core, from the developer's extension point |
| `context.capturedAt` | timestamp | what `ctx.now()` returned during the local run | Internal | hoyadb core, automatically |

## Worked instance

Example command envelope:

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

## What is deliberately absent

**The row mutations the local handler produced are not here.** The local
run inserted an `orders` row and several `order_items` rows
(`DM-0001`); none of them appear. The backend derives its own by re-running
its own handler copy — that is the property `OBJ-0003` measures and
`REQ-0012` requires.

A client **may** attach its recorded local execution log as an optional
debugging aid, which the backend is always free to ignore and which is never
a substitute for the backend's own run. **Its concrete shape is not fixed**
(`ADR-0025`).

## Outcome shapes

Two distinct shapes, deliberately not the same object (`REQ-0016`):

| Shape | Fields | Who sees it |
| --- | --- | --- |
| `WireOutcome` | `status` + `result`/`reason`, **plus** `idempotencyKey` (or `recordId` for an entity-diff) | Transport-internal; correlates an entry back to a pending local command |
| `CommandOutcome` | `status` + `result`/`reason`, **no correlation field** | The application; the core strips the correlation field first |

The app-facing `CommandOutcome.status` is one of `queued`, `sent`,
`acknowledged`, `applied`, `corrected`, `rejected`. The reference transport's
`WireOutcome.status` is limited to `applied`, `corrected`, or `rejected`;
local queue states never appear in its wire response (`API-0001`,
`contracts/openapi.yaml`). On `applied` or `corrected`, `result` carries whatever the
backend handler returned — including the real record id that replaces the
local one (`REQ-0019`, `DM-0001`).

## Mapping

| This field | Maps to | Direction |
| --- | --- | --- |
| whole envelope | `API-0001` `POST /hoyadb/commands` request array element | out |
| `idempotencyKey` | `API-0001` response array element's `idempotencyKey` | round-trip (correlation key, **not** array position — `REQ-0029`) |
| whole envelope | `DM-0004`'s persisted queue entry payload | local |
| `context.capturedAt` | the backend's own `ctx.now()` return value | out (`ADR-0014`'s determinism-threading convention) |

## Lifecycle and retention

Persisted in the offline queue (`DM-0004`) from `queued` until
`acknowledged`, then discarded from the queue — with a bounded
recently-resolved tail retained for debugging whose **window is not fixed**
(`REQ-0021`). Contents are held in local state only and are covered by
at-rest encryption (`REQ-0030`), with the limits `THR-0002` records.

**Not fixed**: the serialization format of `args` (structured JSON versus a
typed binary format) is implementation-time detail (`REQ-0012`).

## Traceability

- Parents: `ARC-0001`, `ADR-0005`.
- Satisfies: `REQ-0012`, `REQ-0013`, `REQ-0014`, `REQ-0015`.
