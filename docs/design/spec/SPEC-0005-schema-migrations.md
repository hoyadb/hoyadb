---
id: SPEC-0005
title: Schema migrations — chain-or-reset
type: spec
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-22
parents: [ARC-0001, ADR-0009]
satisfies: [REQ-0006]
depends_on: [SPEC-0003]
tags: [data-model, migrations]
---

# SPEC-0005: Schema migrations — chain-or-reset

## Purpose

Specify how an application's schema evolves after it ships. This concerns an
**application's own schema**, not hoyadb's library versioning (`NFR-0005`),
even though both use the word "version."

## Algorithm

On database open:

1. Read the client's current local schema version.
2. Compare it to the declared schema's version (`SPEC-0003`).
3. If equal — nothing to do.
4. If a chain of authored migration steps exists from the local version up
   to the declared version, apply each step **in order**.
5. If no such chain exists, **drop and recreate** the local database at the
   declared schema.

No migration step is ever inferred. No partial-upgrade state is ever left
behind. There is no softer fallback than "chain exists" versus "reset."

Migration runs as a **purely local operation on app start, before sync
resumes** — sequenced independently of sync, whose own schema-version check
(`REQ-0014`) handles a client out of date relative to the backend.

## Authoring a migration

```typescript
export const migrations = defineMigrations({
  migrations: [
    { fromVersion: 1, toVersion: 2,
      steps: [{ type: "addColumn", table: "orders", column: "cancelled_at" }] },
    { fromVersion: 2, toVersion: 3,
      steps: [{ type: "addColumn", table: "order_items", column: "discount_cents" }] },
  ],
});
```

A client on version 1 opening an app declaring version 3 applies 1→2 then
2→3. A client on version 1 opening an app that jumped to version 4, with no
chain reaching 4, is reset to version 4.

## What a reset means

Drops and recreates the local database at the declared schema, **discarding
locally-held data for that client**. It does **not** on its own resync data
back down from the backend — whether resyncing happens depends on how
scoping (`SPEC-0012`) and the offline queue (`SPEC-0009`) are configured for
that deployment.

An application that cannot tolerate this for a given schema jump must author
the missing chain explicitly.

## Open

- **The exhaustive set of migration step primitives** — add/drop/rename
  column, add/drop table, backfill an added column's value — is
  implementation-time detail, not fixed here. This spec fixes the
  chain-or-reset policy, not the step catalogue.
- **A real, disclosed gap: a reset and the offline queue do not
  cross-reference each other.** Dropping the local store presumably drops
  commands still sitting unacknowledged in the queue (`SPEC-0009`,
  `DM-0004`), silently discarding a user's pending write. Neither
  `SPEC-0005` nor `SPEC-0009`
  names this interaction. **Not resolved**, and deliberately not invented
  here — no test case covers it because there is no decided behaviour to
  verify.
- **Incremental adoption from an existing database-backed
  app is unaddressed** — every example assumes hoyadb owned the schema from
  v1 (`ADR-0023`).

## Traceability

- Parents: `ARC-0001`, `ADR-0009`.
- Satisfies: `REQ-0006`.
- Depends on: `SPEC-0003`.
- Verified by: `TC-0003`.

## Implemented data model Rust boundary

SPEC-0026/API-0004 define the Rust schema, one-shot queries, lazy relations and
atomic lifecycle boundary. ADR-0031 fixes `__hoya_`, explicit queryable through
tables, seven authored migration primitives and strict queue-free reset/adoption
guards. TC-0026/0027/0028 define conformance and regressions. Use the
[runnable model example](../../../crates/hoyadb-storage-memory/examples/data_model.rs)
for the Rust surface; packaged SDK and sync acceptance remain separate.
