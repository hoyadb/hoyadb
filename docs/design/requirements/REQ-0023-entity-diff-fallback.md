---
id: REQ-0023
title: Plain table writes sync as entity-diff payloads under the same authorization bar as a command
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0001, OBJ-0003]
tags: [sync, entity-diff]
---

# REQ-0023: Plain table writes sync as entity-diff payloads under the same authorization bar as a command

## Pattern

Event-driven, with an Unwanted-behaviour clause against a weaker
authorization path.

## Statement

WHEN an application performs an ordinary table mutation on a table that
does not use a command, THE system SHALL generate an entity-diff payload
carrying the table name, the operation (`create`, `update` or `delete`),
the record id, only the fields actually written, a client timestamp, the
schema version (`REQ-0014`) and the trusted context (`REQ-0015`), and SHALL
queue it through the same offline queue a command uses (`REQ-0018`).

THE system SHALL NOT require the application to construct that payload by
hand.

THE system SHALL carry the same trusted context and be subject to the same
membership rules a command would be — "lower-ceremony" means skipping
*domain logic*, never skipping authorization.

## Examples

Example 1 — an ordinary update generating a payload
(`legacy/docs/06-sync/entity-diff-fallback.md`, `legacy/docs/10-walkthrough.md` step 7):

```typescript
const note = db.table("order_notes").findById("note_789");
await note.update({ body: "Customer requested gift wrap" });
```

generates:

```json
{
  "table": "order_notes", "op": "update", "recordId": "note_789",
  "changes": { "body": "Customer requested gift wrap" },
  "clientTimestamp": "2026-09-15T14:03:00.000Z",
  "schemaVersion": 3,
  "context": { "identity": "opaque-server-verified-identity-value" }
}
```

Result: `changes` is a partial patch, not the whole record — which is
exactly what lets per-field last-write-wins (`REQ-0024`) operate at field
granularity rather than record granularity.

Example 2 — authorization parity: the backend runs the same authorization
check on this payload it would on a command
(`legacy/docs/06-sync/implementation-guide.md`'s `resolveEntityDiff`), rejecting
with `{ status: "rejected", reason: "not authorized" }` where the identity
is not entitled to write that record.

## Open

An entity-diff carries **no idempotency key**, since create/update/delete
against a specific `recordId` is naturally idempotent to resend
(`REQ-0013`). Its outcome is correlated by `recordId`, which **cannot
disambiguate two queued writes to the same record in one batch** — a known,
undesigned gap (`REQ-0029`). Whether an entity-diff can reference an
unresolved local id is **unaddressed** (`REQ-0019`).

## Traceability

- Parents: `OBJ-0001`, `OBJ-0003`.
- Under: `ADR-0005`.
- Satisfied by: `SPEC-0010`, `DM-0003`.
- Verified by: `TC-0011`.
