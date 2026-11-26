---
id: REQ-0001
title: An application declares a schema of typed tables and columns
type: requirement
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0002]
tags: [data-model, schema]
---

# REQ-0001: An application declares a schema of typed tables and columns

## Pattern

Ubiquitous, with an Unwanted-behaviour clause for an undeclared table.

## Statement

THE system SHALL accept a schema declaration consisting of named tables,
each with named columns typed from hoyadb's own primitive type set, plus a
schema version number, and SHALL implicitly add to every table a primary
key column `id` and hoyadb's reserved bookkeeping columns, which the
application does not declare.

IF an application reads or writes a table that its declared schema does not
contain, THEN THE system SHALL reject the operation rather than creating
the table implicitly.

Column types are hoyadb's own primitive set (string, integer, float,
boolean, timestamp, and a relation reference — `REQ-0002`), **not** a
passthrough of whatever the underlying storage engine supports. The
concrete engine behind the storage interface (`SPEC-0001`) is responsible
for representing each hoyadb column type in its own native types, which is
what keeps a schema portable across engines.

## Examples

Example 1 — declaring the walkthrough's `orders` table:

```typescript
const schema = defineSchema({
  version: 1,
  tables: {
    orders: table({
      columns: {
        status: column.string(),
        total_cents: column.integer(),
        placed_at: column.timestamp(),
        notes: column.string({ optional: true }),
      },
    }),
  },
});
```

Result: an `orders` table with the four declared columns, plus an implicit
`id` primary key and hoyadb's reserved columns. `schema.version` is `1`,
and that value is what travels in every write envelope (`REQ-0014`).

Example 2 — undeclared table rejected:

- Input: `db.query("invoices")` against the schema above, which declares no
  `invoices` table.
- Output: the operation is rejected; no `invoices` table is created.

## Open

The exhaustive built-in column-type catalogue, and whether app-defined
custom types/validators are supported, is implementation-time detail — not
fixed by this requirement (`ADR-0002`). Indexing is a separate, undecided
gap (`RISK-0004`).

## Traceability

- Parent: `OBJ-0002`.
- Under: `ADR-0002`.
- Satisfied by: `SPEC-0003`, `DM-0001`.
- Verified by: `TC-0001`.
