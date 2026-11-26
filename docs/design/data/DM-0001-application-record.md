---
id: DM-0001
title: Application record — a row in an app-declared table
type: data-model
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-22
parents: [ARC-0001, ADR-0002]
satisfies: [REQ-0001, REQ-0002]
tags: [data, schema, relations]
---

# DM-0001: Application record — a row in an app-declared table

The shape of one record in a table an application declares (`REQ-0001`),
using the walkthrough's `orders` table as the worked instance
(`legacy/docs/10-walkthrough.md` step 1).

## Persisted fields

Declared by the application:

| Field | Type | Constraints | Classification (DATA-1) | Retention |
| --- | --- | --- | --- | --- |
| `status` | string | required | Internal | until deleted, or until a local reset (`REQ-0006`, `REQ-0027`) |
| `total_cents` | integer | required | Internal | same |
| `placed_at` | timestamp | required | Internal | same |
| `notes` | string | optional | Internal | same |
| `customer_id` | relation reference | to-one to `customers` (`REQ-0002`) | Internal | same |

Added implicitly by hoyadb, never declared by the application
(`REQ-0001`):

| Field | Type | Notes |
| --- | --- | --- |
| `id` | string | Primary key. Locally generated for an optimistically-applied create, carrying a `local:` prefix until the backend-assigned id replaces it (`REQ-0019`). |
| hoyadb bookkeeping columns | — | Named within the reserved namespace (`REQ-0033`), so they cannot collide with an application's own column names. **The exact prefix string is not fixed** (`ADR-0023`). |

Column types are hoyadb's own primitive set — string, integer, float,
boolean, timestamp, relation reference — **not** a passthrough of the
underlying engine's types. The concrete engine represents each in its own
native types (`SPEC-0001`), which is what keeps a record portable across
engines.

## Computed / derived values

| Value | Derived from | Computed when | Notes |
| --- | --- | --- | --- |
| `order.customer` | `customer_id` + the declared to-one relation | On navigation | Returns a **query object**, not a materialized record — relations are not eagerly loaded (`REQ-0002`). |
| `order.items` | the declared has-many relation | On navigation | Same: a query object, resolvable once or as a subscription. |
| set membership | the record's fields vs. a live query's predicates | On every local write | What `REQ-0008` re-notifies on. Never persisted. |

## Mapping to interface payloads

| This field | Maps to | Direction | Notes |
| --- | --- | --- | --- |
| `id` | `DM-0002` command outcome's `result.id` | in | The backend-assigned id replaces the local one on `applied`/`corrected` (`REQ-0019`). |
| any declared field | `DM-0003`'s `changes` object | out | Only fields actually written, as a partial patch (`REQ-0023`). |
| any declared field | `API-0001`'s `GET /hoyadb/changes` `changes[].changes` | in | Applied on pull (`REQ-0026`). |
| `customer_id` | a command's `args`, e.g. `customerId` | out | Carried as an argument, not as a row mutation (`REQ-0012`). |

## Relationships

To-one, has-many and many-to-many are all first-class declared constructs
(`REQ-0002`). In the walkthrough's schema: an order belongs to one
`customers` record, has many `order_items`, and has many `order_notes`;
`order_items` additionally has a to-one to `products`.

**Not decided**: whether a many-to-many join table is always app-visible and
app-queryable or can be an implicit hoyadb-managed table (`ADR-0002`).

## Lifecycle and migrations

A record's shape changes only through an authored migration chain; where no
chain reaches the declared version, the local database — and therefore every
record in it — is dropped and recreated (`REQ-0006`).

**Disclosed gap**: what happens to queued writes referencing these records at
the moment of such a reset is unaddressed anywhere in the doc tree
(`REQ-0006`, `REQ-0018`).

**Not fixed**: the exhaustive built-in column-type catalogue, whether
app-defined custom types are supported, and indexing (`RISK-0004`).

## Traceability

- Parents: `ARC-0001`, `ADR-0002`.
- Satisfies: `REQ-0001`, `REQ-0002`.

## Concrete Phase 2 representation — 2026-10-22

ADR-0031/API-0004 resolve the earlier open namespace and join-table choices:
`__hoya_` is reserved, implicit `Row.id` is separate from application fields,
and persisted rows carry Integer `__hoya_schema_version`, hidden from public
rows/changes. Explicit application-queryable through tables have managed
link/unlink operations. The six exact primitive/null kinds use API-0003.
Reserved logical `__hoya_model` stores canonical schema/version and the positive
persisted default-ID counter; it does not change SQLite private format 1.
Known queue-free forward opens migrate atomically or reset logical tables while
retaining the counter. Unknown bookkeeping/queues reject unchanged; this does
not define pending-command recovery. Backend ID replacement remains sync work.
See [Phase 2 evidence](../../implementation/phase-two-data-model.md).
