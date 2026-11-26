---
id: SPEC-0003
title: Schema declaration and relations
type: spec
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-22
parents: [ARC-0001, ADR-0002]
satisfies: [REQ-0001, REQ-0002, REQ-0033, NFR-0005]
depends_on: [DM-0001]
tags: [data-model, schema, relations]
---

# SPEC-0003: Schema declaration and relations

Source: `legacy/docs/03-data-model/schema.md` and `legacy/docs/03-data-model/relations.md`.

## Purpose

Specify how an application declares its data model to hoyadb — tables, typed
columns, and the three first-class relation kinds — and how hoyadb's own
reserved names stay out of that namespace.

## Declaring tables and columns

A schema is a set of named tables, each with named columns typed from
hoyadb's own primitive set, plus a schema version number:

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

Every table implicitly gains an `id` primary key and hoyadb's reserved
bookkeeping columns, neither declared by the application (`DM-0001`).

`version` is the *schema's* version, distinct from hoyadb's library version
(`NFR-0005`). It is the value carried in every write envelope (`REQ-0014`)
and the value migrations move a client between (`SPEC-0005`).

## Column types are hoyadb's own

The primitive set is string, integer, float, boolean, timestamp, and a
relation reference — **not** a passthrough of the underlying engine's types.
The concrete engine behind `SPEC-0001` represents each in its own native
types, which is what keeps a schema portable across engines.

## Declaring relations

All three kinds are first-class schema constructs with their own
declaration form, so an application never hand-builds a relation from a
filtered query or a manually-maintained pivot table (`REQ-0002`):

```typescript
customers: table({
  columns: { name: column.string(), email: column.string() },
  relations: { orders: relation.hasMany("orders", { foreignKey: "customer_id" }) },
}),
orders: table({
  columns: { status: column.string(), total_cents: column.integer() },
  relations: {
    customer: relation.toOne("customers", { foreignKey: "customer_id" }),
    items: relation.hasMany("order_items", { foreignKey: "order_id" }),
  },
}),
```

Many-to-many is declared through an explicit join table and exposed on both
sides, e.g.
`relation.manyToMany("products", { through: "order_products" })`.

## Load behaviour

A relation is **not** eagerly loaded. Navigating `order.customer` returns a
query object the application resolves — once, or as a live subscription
(`SPEC-0006`) — rather than a materialized record. This keeps the cost of
declaring a relation proportional to whether the application actually reads
through it.

## Reserved namespace enforcement

hoyadb's own tables, columns and fields live within a clearly namespaced
reserved convention — for example a fixed `__hoyadb_` prefix. A schema
declaring a name inside that namespace is **rejected**, rather than allowed
to shadow or corrupt hoyadb's own state (`REQ-0033`). **The exact prefix
string is not fixed** (`ADR-0023`).

## Error handling

| Condition | Behaviour |
| --- | --- |
| Read/write against an undeclared table | Rejected; the table is not created implicitly (`REQ-0001`) |
| Schema declares a name in the reserved namespace | Schema declaration rejected (`REQ-0033`) |
| Relation declared against an undeclared table | Schema declaration rejected |

## Open

- **The exhaustive built-in column-type catalogue, and whether app-defined
  custom types/validators are supported, is not specified** —
  implementation-time detail (`ADR-0002`).
- **Whether a many-to-many join table is always app-visible and
  app-queryable, or can be an implicit hoyadb-managed table, is not
  decided** (`ADR-0002`).
- **Eager/batched loading across a relation** — avoiding N+1 patterns when
  iterating a large has-many result — is not specified.
- **Indexing is not decided here or anywhere** (`RISK-0004`).
- **Large-schema authoring patterns are unaddressed** — splitting a 50+
  table schema across files, avoiding command-name collisions. Every
  example in the tree is a single inline 2–3 table declaration
  (`ADR-0024`).

## Traceability

- Parents: `ARC-0001`, `ADR-0002`.
- Satisfies: `REQ-0001`, `REQ-0002`, `REQ-0033`, `NFR-0005`.
- Depends on: `DM-0001`.
- Verified by: `TC-0001`, `TC-0003`.

## Storage prerequisite boundary — 2026-10-22

SPEC-0025/API-0003 define an explicit internal Schema input, exact storage
primitive/null semantics and direct to-one field resolution. ADR-0030 selects
engine-owned physical indexes without application index declarations. This
closes the index-ownership question above for the initial storage contract; it
does not deliver the public schema builder, all three relation navigation APIs,
version lifecycle or migrations described here.

## Implemented Phase 2 Rust boundary — 2026-10-22

SPEC-0026/API-0004 implement the public Rust schema, one-shot query, lazy
relation and atomic lifecycle boundary. ADR-0031 fixes the `__hoya_` prefix,
explicit queryable through tables, seven authored migration primitives and
strict queue-free reset/adoption guards. TC-0026 maps the original acceptance
behaviors to memory/native/browser assertions; TC-0027/0028 guard review defects.
See [execution evidence](../../implementation/phase-two-data-model.md) and
[the runnable example](../../../crates/hoyadb-storage-memory/examples/data_model.rs).
Earlier open questions above retain their historical framing; API-0004 is the
concrete implemented contract. Observation, sync/queue behavior and packaged
platform SDKs remain later phases.
