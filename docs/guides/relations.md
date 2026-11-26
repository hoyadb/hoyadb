# Relations

> Supporting guide. Current design authority: [Schema and relations](../design/spec/SPEC-0003-schema-and-relations.md); [Query model](../design/spec/SPEC-0004-query-model.md).
> Code and package names are illustrative design examples; no SDK implementation
> is supplied by this guide. Current API contracts define the supported SDK surface.

How to-one, has-many, and many-to-many relationships are declared between
tables, and what "first-class" concretely means for each.

## Non-goals

Carried forward from [VISION.md](../design/vision/VIS-0001-hoyadb.md)'s "What we deliberately
avoid", applicable to this doc:

- **A fully schemaless model.** Relations are declared schema constructs,
  not inferred from ad-hoc document references.
- **A schema that's just a mirror of a specific external database.**
  Relation declarations are hoyadb's own construct, not foreign-key
  conventions borrowed from a specific external database.

## First-class means a schema construct, not a convention

"First-class" means each relation kind (to-one, has-many, many-to-many) has
its own declaration in [schema.md](../design/spec/SPEC-0003-schema-and-relations.md)'s schema language and its own
load behavior — an app never hand-builds a relation out of a plain filtered
query or a manually-maintained pivot table.

HoyaDB declares to-one, has-many and many-to-many relations as schema constructs. Applications do not need to emulate the relation kinds with conventions over unrelated queries.

## Declaring relations

Continuing the `orders` / `order_items` schema from
[schema.md](../design/spec/SPEC-0003-schema-and-relations.md):

```typescript
import { defineSchema, table, column, relation } from "@hoyadb/client";

export const schema = defineSchema({
  version: 1,
  tables: {
    customers: table({
      columns: {
        name: column.string(),
        email: column.string(),
      },
      relations: {
        // has-many: one customer has many orders
        orders: relation.hasMany("orders", { foreignKey: "customer_id" }),
      },
    }),

    orders: table({
      columns: {
        status: column.string(),
        total_cents: column.integer(),
      },
      relations: {
        // to-one: each order belongs to one customer
        customer: relation.toOne("customers", { foreignKey: "customer_id" }),
        // has-many: one order has many line items
        items: relation.hasMany("order_items", { foreignKey: "order_id" }),
      },
    }),

    order_items: table({
      columns: {
        quantity: column.integer(),
      },
      relations: {
        order: relation.toOne("orders", { foreignKey: "order_id" }),
      },
    }),

    products: table({ columns: { name: column.string() } }),

    // many-to-many: orders <-> products, via an explicit join table
    order_products: table({
      columns: {},
      relations: {
        order: relation.toOne("orders", { foreignKey: "order_id" }),
        product: relation.toOne("products", { foreignKey: "product_id" }),
      },
    }),
    // exposed on both sides as a first-class many-to-many:
    // orders.relations.products = relation.manyToMany("products", { through: "order_products" })
    // products.relations.orders = relation.manyToMany("orders", { through: "order_products" })
  },
});
```

A relation, once declared, is navigable from either side without the app
writing its own join query: `order.customer` resolves the to-one, and
`customer.orders` / `order.items` resolve the has-many sides as
[query objects](../design/spec/SPEC-0006-subscriptions.md) — subject to the same
one-shot-vs-subscribe choice as any other query.

## Load behavior

A relation reference is not eagerly loaded by default: navigating
`order.customer` returns a query object the app resolves (once, or as a
live [subscription](glossary.md#subscription)) rather than a customer record already materialized in
memory. This keeps the default cost of declaring a relation proportional to
whether the app actually reads through it, consistent with
[05-reactivity/performance-guards.md](../design/spec/SPEC-0007-performance-guards.md)'s
"cheap by default, precision by opt-in" principle. Eager/batched loading
across a relation (avoiding N+1 query patterns when iterating a large
has-many result) is implementation-time detail not yet specified here.

## What's still open

Whether a many-to-many relation's join table is always an app-visible,
app-queryable table (as in the example above) or can instead be an
implicit, hoyadb-managed table the app never sees directly is not yet
decided — this doc specifies that many-to-many is first-class and
navigable from both sides, not the exact visibility of its join
mechanism. How a declared relation is actually queried — to-one traversal
via dot-path field names, has-many/many-to-many not yet covered — is
specified separately in [query-model.md](../design/spec/SPEC-0004-query-model.md) (ADR-0027).
