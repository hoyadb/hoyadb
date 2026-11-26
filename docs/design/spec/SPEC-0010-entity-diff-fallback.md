---
id: SPEC-0010
title: Entity-diff fallback — the plain create/update/delete write path
type: spec
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-24
parents: [ARC-0001, ADR-0005]
satisfies: [REQ-0023]
depends_on: [DM-0003, SPEC-0009]
tags: [sync, entity-diff]
---

# SPEC-0010: Entity-diff fallback — the plain create/update/delete write path

## Current Phase 5 contract — 2026-10-24

ADR-0033 and API-0009 resolve the original open items recorded below. Entity
sync is opt-in per declared table. The envelope has `writeId` correlation plus
explicit `removedFields`, and uses the shared queue/status/dependency/inverse
machinery. A shared admission permit excludes ordinary entity writes while a
command handler is active. TC-0034 E01–E06 verifies these additive contracts;
`recordId` alone is no longer the correlation rule. The historical source and
original gaps below explain the extension rather than define current behavior.

Source: `legacy/docs/06-sync/entity-diff-fallback.md`.

## Purpose

Specify the lower-ceremony write path for tables with no real domain logic
to express as a command (`SPEC-0008`), so not every write requires designing
a command. The two paths coexist per table, not globally exclusively.

## Lower-ceremony means skipping domain logic, never authorization

This is stated explicitly rather than left implicit, because an unaudited
weaker path here would undermine `SPEC-0014` regardless of how careful that
design is about the command path:

> An entity-diff write carries and is checked against the **exact same**
> trusted context and membership rules a command would be, enforced on the
> backend.

## Generating a payload

An application never hand-builds the payload. It writes an ordinary table
mutation and hoyadb generates it:

```typescript
const note = db.table("order_notes").findById("note_789");
await note.update({ body: "Customer requested gift wrap" });
```

The resulting payload shape is specified in `DM-0003`. Key properties:

- `op` is one of `create`, `update`, `delete`.
- **`changes` carries only the fields actually written** — a partial patch,
  never the whole record. This is what lets `SPEC-0011`'s per-field
  last-write-wins operate at field granularity rather than record
  granularity.
- `clientTimestamp` is LWW comparison input and is **not** trusted as the
  tie-break authority (`SPEC-0011`).
- `schemaVersion` and `context` carry the same meaning as in a command
  envelope (`SPEC-0008`), reused as-is rather than re-explained.

## Reliability

An entity-diff goes through the same offline queue as a command
(`SPEC-0009`) — persisted, tracked through the same states, retried the
same way.

It carries **no idempotency key**, because create/update/delete against a
specific `recordId` is naturally idempotent to resend: re-applying the same
patch, or the same delete, produces the same end state. A command's handler,
by contrast, may have side effects beyond the record it returns. Its outcome
is therefore correlated by `recordId` (`SPEC-0013`).

## Batching

Every write goes out through the one array-based endpoint by default,
entity-diffs and commands alike, **in the same array** — a lone write is an
array of length one, not a different wire shape. The two payload shapes are
distinguished by the presence of `command` versus `table` + `op`. Each
entity-diff is still independently authorized and conflict-resolved, exactly
as if it were the only entry (`SPEC-0013`).

## Open

- **Known correlation gap, not fixed**: `recordId` alone cannot
  disambiguate two still-queued entity-diff writes to the *same* record
  landing in one array — two offline edits to different fields of one note
  produce two outcomes carrying an identical `recordId`, with nothing in the
  contract to tell a client which is which. Closing this needs a per-write
  correlation id for entity-diffs, **which is not designed**.
- **Whether an entity-diff gets equivalent per-write visibility in
  `SPEC-0015`'s introspection array, correlated by `recordId`, is not
  addressed** — documented and exemplified in command terms only. A real
  gap, not silently assumed to work the same way.
- **Whether an entity-diff can reference another write's unresolved local
  id, with the same "stays queued until resolved" protection, is
  unaddressed anywhere in the tree** (`SPEC-0009`). As written, only the
  command path can correctly depend on an unresolved result.
- Whether a concurrent entity-diff write is excluded while a command handler
  runs is **unstated** (`SPEC-0008` §3).

## Traceability

- Parents: `ARC-0001`, `ADR-0005`.
- Satisfies: `REQ-0023`.
- Depends on: `DM-0003`, `SPEC-0009`.
- Verified by: `TC-0011`.
