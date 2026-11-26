# Walkthrough: one order, start to finish

> For the implemented JavaScript application API and executed examples, build the [public HTML documentation](../public/README.md) and start with its Getting started page. Historical illustrative syntax below is design context.

> Supporting guide. Current design authority: [Developer integration flow](../design/ux/flows/UX-0001-developer-integration-flow.md); [Command protocol](../design/spec/SPEC-0008-command-protocol.md); [Offline queue](../design/spec/SPEC-0009-offline-queue.md).
> Code and package names are illustrative design examples; no SDK implementation
> is supplied by this guide. Historical links preserve the original rationale.

Every other doc in this tree shows its own narrow slice of hoyadb, each
using variations of the same `orders`/`customers`/`order_items` example.
This doc mostly threads those existing pieces into one linear story, so a
reader can see how they actually fit together instead of assembling that
picture themselves across eight files — one exception is step 4, which
surfaces a real, previously-undisclosed gap rather than just reassembling
prior content. Follow the links; this doc is the map, not a replacement
for the docs it points into.

## Non-goals

Carried forward from [VISION.md](../design/vision/VIS-0001-hoyadb.md)'s "What we deliberately
avoid", applicable to this doc (it touches schema, commands, and the
[entity-diff](glossary.md#entity-diff-fallback-path) path, so both
apply here even though this doc designs neither):

- **A fully schemaless model.** Step 1's schema is declared, not inferred.
- **Sync that only covers reads, leaving writes entirely to the app.**
  Steps 3-7 are all about writes — a command, a dependent command, and a
  plain entity-diff edit.

## 1. Declare the schema

[schema.md](../design/spec/SPEC-0003-schema-and-relations.md) declares `orders` and `order_items`
as plain tables; [relations.md](relations.md) adds a
`customers` table and wires the relations between all three — a customer
has many orders, an order belongs to one customer and has many items. A
`products` table (referenced by `order_items`, and by the stock check in
step 6 below) completes the picture — beyond what schema.md/relations.md
show individually, but implied by both, and by
[command-protocol.md](../design/spec/SPEC-0008-command-protocol.md#1-shape-of-a-command)'s
own `add-order-v1` example already taking a `productId` argument:

```typescript
import { defineSchema, table, column, relation } from "@hoyadb/client";

export const schema = defineSchema({
  version: 1,
  tables: {
    customers: table({
      columns: { name: column.string(), email: column.string(), country: column.string() },
      relations: { orders: relation.hasMany("orders", { foreignKey: "customer_id" }) },
    }),
    orders: table({
      columns: {
        status: column.string(),
        total_cents: column.integer(),
        placed_at: column.timestamp(),
      },
      relations: {
        customer: relation.toOne("customers", { foreignKey: "customer_id" }),
        items: relation.hasMany("order_items", { foreignKey: "order_id" }),
      },
    }),
    order_items: table({
      columns: { quantity: column.integer(), unit_price_cents: column.integer() },
      relations: {
        order: relation.toOne("orders", { foreignKey: "order_id" }),
        product: relation.toOne("products", { foreignKey: "product_id" }),
      },
    }),
    products: table({
      // `quantity` here is the product's stock level — matches
      // implementation-guide.md's own `stock.quantity` stock check below,
      // not order_items' own `quantity` (how many of this product one
      // line item ordered) — same field name, two different tables.
      columns: { name: column.string(), quantity: column.integer() },
    }),
    // order_notes is used through both the command and entity-diff paths
    // in this walkthrough — see step 4 for why.
    order_notes: table({
      columns: { body: column.string() },
      relations: { order: relation.toOne("orders", { foreignKey: "order_id" }) },
    }),
  },
});
```

`order_items.product` above is a plain to-one, matching how
[command-protocol.md](../design/spec/SPEC-0008-command-protocol.md#1-shape-of-a-command)'s
and [implementation-guide.md](sync-implementation.md#worked-example-add-order-v1-server-side)'s
`add-order-v1` handlers already use it (`order_items.insert({ product_id,
quantity, ... })`). [relations.md](relations.md)'s own
example models orders↔products as a many-to-many instead, to demonstrate
that relation kind in isolation — both are valid; this walkthrough
follows the simpler shape already established in the sync docs.

## 2. Read and watch orders

Before any order exists, the app already knows how it would read them —
[query-model.md](../design/spec/SPEC-0004-query-model.md)'s predicates, and
[subscriptions.md](../design/spec/SPEC-0006-subscriptions.md)'s live-updating
`.observe()`:

```typescript
const unsubscribe = db
  .query("orders")
  .where("customer.country", "=", "FR")
  .orderBy("placed_at", "desc")
  .observe((orders) => renderOrdersList(orders));
```

## 3. Place an order — a command, applied optimistically

`orders` has real domain logic (a first-order discount, per
[command-protocol.md](../design/spec/SPEC-0008-command-protocol.md#1-shape-of-a-command)'s
worked example) — enough to justify a
[command](glossary.md#command) rather than a plain write:

```typescript
const order = await addOrderV1.execute({
  customerId: "cust_123",
  items: [{ productId: "p1", quantity: 2 }],
});
// order.id is available immediately — a LOCAL id, per
// offline-queue.md, until the backend confirms it
```

This runs the client's own copy of the handler against the local store
right away — [command-protocol.md](../design/spec/SPEC-0008-command-protocol.md#3-handler-execution-model)'s
"Handler execution model" is what guarantees this applies atomically, in
order, even though the handler is `async` and read from the local store
partway through (checking the customer's prior-order count for the
discount).

## 4. Reference it before it's confirmed

The app immediately adds a note to the order it just created — a second
command, referencing the first one's still-local id. This *is* a command,
not the entity-diff path order_notes uses later in step 7, and
deliberately so: [offline-queue.md](../design/spec/SPEC-0009-offline-queue.md#ordering-across-dependent-writes)'s
"stays queued until resolved" protection for a reference to another
write's not-yet-confirmed local id is specified only for commands.
Whether an entity-diff write could ever safely reference an unresolved
local id the same way isn't addressed anywhere in this tree — a real gap
this specific choice surfaces, not one this walkthrough invents an answer
to. `add-order-note-v1` has no real business logic of its own (per
[entity-diff-fallback.md](../design/spec/SPEC-0010-entity-diff-fallback.md)'s usual bar for
what needs a command) — it's a command here purely to get that ordering
guarantee:

```typescript
await addOrderNoteV1.execute({ orderId: order.id, body: "Gift wrap" });
```

The note command applies locally right away, but stays `queued` — never
`sent` — until the order command's own outcome resolves the real id.

## 5. Over the wire — the order, alone

The note command is still `queued`, not `sent` — it references the
order's unresolved local id, and
[offline-queue.md](../design/spec/SPEC-0009-offline-queue.md#ordering-across-dependent-writes)'s
rule is that a command referencing an unresolved local id is never
transmitted. Only the order is actually eligible to send. D18's "batching
is the default" answer, [transport-http-example.md](../design/contracts/API-0001-http-sync-transport.md#post-hoyadbcommands)'s
one endpoint, still applies — a batch of one is the ordinary case, not a
special one:

```json
[
  { "command": "add-order-v1", "idempotencyKey": "01J...Q3", "schemaVersion": 1, "args": { "customerId": "cust_123", "items": [{ "productId": "p1", "quantity": 2 }] }, "context": { "identity": "...", "capturedAt": "..." } }
]
```

## 6. The backend, independently, decides

[implementation-guide.md](sync-implementation.md#3-the-command-path)'s
worked `addOrderV1Handler` runs the developer's own, separately-authored
server-side logic — including the one check the client couldn't make: is
there actually enough stock?

- **Enough stock** → `applied`, carrying the real order id. The client
  swaps the local id for it, and the note command (still queued, waiting
  on exactly this) gets its `orderId` corrected and finally sent.
- **Not enough stock** → `rejected`. The app finds out through the same
  subscription mechanism as any other data change — see
  [command-protocol.md](../design/spec/SPEC-0008-command-protocol.md#5-error-handling-and-rollback)'s
  `useManageRollback` example — and the optimistic order and its
  now-orphaned note both [roll back](glossary.md#rollback) together.

## 7. A plain write, no domain logic

Editing the note's text later has no business logic worth a command — the
[entity-diff fallback path](../design/spec/SPEC-0010-entity-diff-fallback.md) instead:

```typescript
const note = db.table("order_notes").findById("note_789");
await note.update({ body: "Customer requested gift wrap" });
```

Same [offline queue](../design/spec/SPEC-0009-offline-queue.md), same authorization bar as
a command (per [entity-diff-fallback.md](../design/spec/SPEC-0010-entity-diff-fallback.md#lower-ceremony-means-skipping-domain-logic-never-authorization)),
resolved with per-field [conflict-resolution.md](../design/spec/SPEC-0011-conflict-resolution.md)
last-write-wins instead of an authoritative handler re-run.

## 8. Debugging it

If a support ticket says "my gift-wrapped order never showed up":
[observability.md](../design/spec/SPEC-0015-queue-observability.md)'s per-command
status is where to look first — was `add-order-v1` ever acknowledged, is
it sitting `rejected`, or is the note command still `queued`, quietly
waiting on a parent that never resolved? This works well if the ticket
comes in soon after the fact; a `rejected`/`applied`/`corrected` entry is
only retained for a bounded window (see
[offline-queue.md](../design/spec/SPEC-0009-offline-queue.md#open)) before it
ages out, at which point an empty result can't be told apart from "this
command never ran." A different, related gap for the same scenario:
[observability.md](../design/spec/SPEC-0015-queue-observability.md#open)'s
diagnostic export — a support-ticket-ready state dump — doesn't exist yet
either.

## Where this walkthrough stops

It doesn't cover schema migrations, security/at-rest encryption, or
testing this flow — each already has its own doc
([migrations.md](../design/spec/SPEC-0005-schema-migrations.md),
[security-model.md](../design/spec/SPEC-0014-security-model.md),
[09-testing.md](testing.md)) written at the depth this one
deliberately isn't.
