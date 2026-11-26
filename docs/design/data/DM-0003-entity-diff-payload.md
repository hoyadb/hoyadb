---
id: DM-0003
title: Entity-diff payload — what travels for a plain create/update/delete
type: data-model
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-24
parents: [ARC-0001, ADR-0005]
satisfies: [REQ-0023, REQ-0025]
tags: [data, sync, entity-diff, wire-format]
---

# DM-0003: Entity-diff payload — what travels for a plain create/update/delete

## Phase 5 extension — 2026-10-24

ADR-0033/API-0009 resolve the original gaps retained below as design history.
The current exact payload additionally requires `writeId` (immutable per-write
correlation, never an idempotency key) and `removedFields` (sorted explicit
optional-field removals, represented by Null in `changes`). Outcomes correlate
by the complete frozen `table`/`recordId`/`writeId` tuple. Queue status identifies
both write kinds without payloads. Declared Reference values and record IDs
participate in the same durable multi-parent dependency/mapping machinery as
commands. Configured ordinary writes capture one trusted context per batch;
command scopes and inbound/inverse writes never generate secondary diffs.
See API-0009 for namespaced identities, guarded comparison state and exact codec.

The payload hoyadb generates from an ordinary table mutation on a table that
does not use a command (`REQ-0023`). Source:
`legacy/docs/06-sync/entity-diff-fallback.md`.

## Fields

| Field | Type | Constraints | Classification (DATA-1) | Populated by |
| --- | --- | --- | --- | --- |
| `table` | string | required; a declared table (`REQ-0001`) | Internal | hoyadb core |
| `op` | enum | required; one of `create`, `update`, `delete` | Internal | hoyadb core |
| `recordId` | string | required; also the response correlation key (`REQ-0029`) | Internal | hoyadb core |
| `changes` | object | **only the fields actually written** — a partial patch, never the whole record | **Confidential** — application domain data | hoyadb core, from the mutation |
| `clientTimestamp` | timestamp | required; LWW comparison input, **never the tie-break authority** (`REQ-0025`) | Internal | hoyadb core |
| `schemaVersion` | integer | required (`REQ-0014`) | Internal | hoyadb core |
| `context.identity` | opaque | required; same meaning as `DM-0002`'s (`REQ-0015`) | **Confidential** | hoyadb core |

An application never hand-builds this payload — it writes an ordinary table
mutation and hoyadb generates it.

## Worked instance

`legacy/docs/06-sync/entity-diff-fallback.md`, from
`note.update({ body: "Customer requested gift wrap" })`:

```json
{
  "table": "order_notes",
  "op": "update",
  "recordId": "note_789",
  "changes": { "body": "Customer requested gift wrap" },
  "clientTimestamp": "2026-09-15T14:03:00.000Z",
  "schemaVersion": 3,
  "context": { "identity": "opaque-server-verified-identity-value" }
}
```

`changes` carrying only `body` — not the record's other fields — is what
lets per-field last-write-wins operate at field granularity rather than
whole-record granularity (`REQ-0024`).

## Difference from a command envelope

| | `DM-0002` (command) | `DM-0003` (entity-diff) |
| --- | --- | --- |
| Idempotency key | Yes | **No** — create/update/delete against a specific `recordId` is naturally idempotent to resend (`REQ-0013`) |
| Response correlation | `idempotencyKey` | `recordId` |
| Backend resolution | Re-runs the handler | Authorization check + per-field LWW (`REQ-0024`) |
| Authorization bar | Same | **Same** — "lower-ceremony" means skipping domain logic, never authorization (`REQ-0023`) |

## Mapping

| This field | Maps to | Direction |
| --- | --- | --- |
| whole payload | `API-0001` `POST /hoyadb/commands` request array element | out (shares the array with commands; distinguished by the presence of `table`+`op` rather than `command`) |
| `recordId` | `API-0001` response array element's `recordId` | round-trip (correlation key) |
| `changes` | `DM-0001`'s declared fields | local + out |
| `clientTimestamp` | the backend's per-field LWW comparison | out |

## Known gap in correlation

**`recordId` alone cannot disambiguate two queued entity-diff writes to the
*same* record landing in one array** — two offline edits to different fields
of one note produce two outcomes carrying an identical `recordId`, with
nothing in the contract to tell a client which is which. Closing this needs
a per-write correlation id for entity-diffs, which is **not designed**
(`REQ-0029`, `ADR-0026`).

Relatedly, whether an entity-diff write gets equivalent per-write
visibility in `REQ-0021`'s introspection array, correlated by `recordId`, is
**not addressed** either.

## Lifecycle and retention

Queued, tracked and retried through the same offline queue a command uses
(`DM-0004`, `REQ-0018`). Whether an entity-diff can reference another
write's unresolved local id, the way a command can, is **unaddressed
anywhere in the doc tree** (`REQ-0019`).

## Traceability

- Parents: `ARC-0001`, `ADR-0005`.
- Satisfies: `REQ-0023`, `REQ-0025`.
