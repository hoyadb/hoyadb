---
id: DM-0004
title: Offline queue entry — the persisted record of a pending write
type: data-model
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [ARC-0001, ADR-0016]
satisfies: [REQ-0018, REQ-0019, REQ-0021]
tags: [data, sync, offline-queue]
---

# DM-0004: Offline queue entry — the persisted record of a pending write

The locally-persisted record hoyadb keeps for each write awaiting
acknowledgement, under a reserved table or file name (`REQ-0033`). Source:
`legacy/docs/06-sync/offline-queue.md`.

## Persisted fields

| Field | Type | Constraints | Classification (DATA-1) | Retention |
| --- | --- | --- | --- | --- |
| correlation key | string | the `idempotencyKey` for a command (`DM-0002`), or the `recordId` for an entity-diff (`DM-0003`) | Internal | until acknowledged |
| `state` | enum | `queued` → `sent` → `acknowledged`; a write is discarded from the queue on acknowledgement (`REQ-0018`) | Internal | until acknowledged |
| envelope / payload | object | the full `DM-0002` or `DM-0003` body, resent verbatim on retry (`REQ-0013`) | **Confidential** — contains `args`/`changes` and `context.identity` | until acknowledged |
| local execution order | integer | the serialized order `REQ-0011` establishes; what makes sending an array in order safe (`REQ-0029`) | Internal | until acknowledged |
| `blockedOn` | string | present only while the entry is `queued` pending another write's unresolved local id; carries the `local:`-prefixed id the app itself holds, **not** the blocking command's idempotency key (`REQ-0021`) | Internal | until the dependency resolves |

## Local id mapping

Alongside the entries, hoyadb maintains the mapping from a locally-generated
id to its backend-assigned replacement (`REQ-0019`), so that queued writes
referencing the local id are corrected when the parent's authoritative
outcome (`applied` or `corrected`) resolves it. The swap happens on that
outcome, **not** on acknowledgement — acknowledgement does not carry the
real id.

## Introspection projection

`REQ-0021`'s per-command array is a projection over two different retention
lifecycles, which this model has to represent distinctly:

| Bucket | Contents | Lifecycle |
| --- | --- | --- |
| Still queued | entries in `queued` or `sent` | The queue proper; retried until acknowledged |
| Recently resolved | `applied` / `corrected` / `rejected` entries | **Already discarded from the queue**, kept in a short bounded tail purely so a debugging workflow can still see them |

```typescript
// legacy/docs/06-sync/offline-queue.md
{
  pending: 3, sending: 1, blocked: false,
  commands: [
    { idempotencyKey: "01J...Q3", state: "sent" },                              // still queued
    { idempotencyKey: "01J...Q4", state: "queued", blockedOn: "local:a1b2c3" }, // still queued
    { idempotencyKey: "01J...Q2", state: "rejected", reason: "..." },           // recently resolved
  ],
}
```

## Durability

Where the engine supports transactions, an entry's state transition and the
local data mutation it produced commit in **one** transaction, so there is no
window in which one happened without the other (`NFR-0003`). Where it does
not, state is tracked best-effort with **no atomicity claim** — an explicit,
accepted limitation.

## Open

- **The recently-resolved retention window — how long, how many — is not
  fixed** (`REQ-0021`).
- **Whether the dependency edges (`blockedOn` and the local-id mapping) are
  themselves durably persisted or reconstructed in memory at runtime is
  unstated** — which decides whether an app kill mid-chain recovers
  correctly (`REQ-0019`).
- **Only one parent-to-child hop is designed.** A transitive chain (A→B→C),
  and an entry referencing two unresolved local ids at once, are not covered.
- **No attempt count or timing is stored**, so nothing can threshold "stuck"
  beyond the `state` value itself (`ADR-0028`).
- **What happens to entries when the local database is reset** — by a missing
  migration chain (`REQ-0006`) or a membership reset (`REQ-0027`) — is
  unaddressed anywhere.
- The exact reserved name for this table/file is implementation-time detail
  (`REQ-0033`).

## Traceability

- Parents: `ARC-0001`, `ADR-0016`.
- Satisfies: `REQ-0018`, `REQ-0019`, `REQ-0021`.
- Depends on: `DM-0002`, `DM-0003` (the payloads it persists).
