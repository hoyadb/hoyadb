---
id: SPEC-0008
title: Command protocol — shape, execution model, reliability and error handling
type: spec
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [ARC-0001, ADR-0005, ADR-0025, ADR-0010, ADR-0011, ADR-0012, ADR-0013, ADR-0014]
satisfies: [REQ-0010, REQ-0011, REQ-0012, REQ-0013, REQ-0014, REQ-0015, REQ-0016, REQ-0017]
depends_on: [DM-0002, SPEC-0009]
tags: [sync, commands]
---

# SPEC-0008: Command protocol — shape, execution model, reliability and error handling

## Purpose

Specify hoyadb's primary write path: what a command is, how its handler runs
locally, what travels, and how a backend correction or rejection reaches the
application.

## 1. Shape of a command

```typescript
export const addOrderV1 = defineCommand("add-order-v1", {
  args: { customerId: "string", items: "array" },
  async handler(db, args, ctx) {
    const priorOrderCount = (await db.query("orders")
      .where("customer_id", "=", args.customerId).fetch()).length;

    const order = await db.table("orders").insert({
      customer_id: args.customerId,
      status: "pending",
      discounted: priorOrderCount === 0,
      placed_at: ctx.now(),
    });
    for (const item of args.items) {
      await db.table("order_items").insert({
        order_id: order.id, product_id: item.productId, quantity: item.quantity,
      });
    }
    return order;
  },
});
```

**The same handler is authored twice by the developer** — once bundled into
the client, once on the backend — not shared code hoyadb ships or executes
on their behalf. See §6.

The envelope that actually travels is specified in `DM-0002`.

## 2. Optimistic local apply, authoritative server re-run

`execute(args)` runs the handler against the local store immediately; the
application sees the result right away, with a `local:`-prefixed id until
the backend confirms it (`SPEC-0009`). The envelope is queued. Once sent,
the backend re-runs its own handler copy against real server-side state. The
backend's result is authoritative: it can confirm, correct (e.g. assign a
different id, apply a discount rule the client did not know), or reject.

## 3. Handler execution model

**Guaranteed:**

- A handler is `async`; its `db` supports awaited reads *and* writes.
- **Only one command handler runs against the local store at a time.**
- A handler's writes commit as **one atomic, ordered local transaction**
  when it returns.
- Handler `db` access goes through a **tracked object**: a later read in the
  same handler sees that handler's own earlier writes (read-your-own-writes
  — the example above relies on `order.id` two lines after the insert), but
  nothing outside it — a live subscription, other app code, another
  handler — observes any of it until the whole log commits. Partial
  mid-handler state is never visible.

This is a **client-side execution detail only**. The envelope is unchanged,
and the backend still independently re-runs its own handler copy, which is
what preserves "the backend re-run is itself the resolution."

**Rejected alternative**: sending the client's recorded action log for the
backend to replay. That removes the backend's independent authority check —
replaying writes cannot catch "this product is actually out of stock now."
Restoring the check means either database constraints (strictly weaker than
arbitrary handler code) or a second independent server-side pass, which is
the handler re-run again. One narrow piece survives: a client **may** attach
that log purely as a debugging aid, always ignorable, **never** a substitute
for the backend's own run.

**Known costs, accepted rather than solved** — all three open:

- **Global serialization**, not scoped to overlapping tables or rows —
  trades throughput for simplicity; no finer-grained scheme designed.
- **No liveness story**: no timeout, cancellation or watchdog for a handler
  that never resolves. One hung handler blocks every other command
  indefinitely.
- **Scope against other write paths is unstated**: whether a concurrent
  entity-diff write (`SPEC-0010`) or an in-progress pull-apply touching the
  same rows is excluded the same way another handler is, is unaddressed.

## 4. Reliability

- **Idempotency** (`REQ-0013`): every command carries a client-generated,
  time-sortable key; a retry resends the same key. Whether a backend
  deduplicates against it is **that backend's choice**, not hoyadb's
  mandate.
- **Ack/receipt** (`REQ-0016`): acknowledgement (durably received) is
  tracked separately from acceptance (the authoritative outcome). Ack
  discards from the queue; no ack keeps it queued for retry. The
  wire-level ack signal is transport-specific (`API-0001`), never surfaced
  to application code.
- **Outcome**: one transport-agnostic `CommandOutcome` — `queued`, `sent`,
  `acknowledged`, `applied`, `corrected`, `rejected`. The
  correlation-bearing `WireOutcome` is transport-internal; the core strips
  the correlation field before the application sees anything (`DM-0002`).

## 5. Error handling and rollback

On `corrected` or `rejected`, the application is notified through the **same
subscription/event/callback mechanism as ordinary data changes** — not a
separate error channel — and an explicit rollback reverts the optimistic
mutation:

```typescript
useManageRollback("orders", order.id, (rollback, correction) => {
  if (correction.status === "corrected") {
    showToast(`Order total adjusted to ${correction.result.total_cents}`);
  } else if (correction.status === "rejected") {
    rollback();
    showToast(`Order could not be placed: ${correction.reason}`);
  }
});
```

This is *an* example for one binding, **not** a shape every binding must
copy. hoyadb provides the mechanism, not the recovery policy — retrying,
surfacing an error or reconciling otherwise is the developer's decision.

## 6. Determinism — a non-goal

**hoyadb does not and cannot guarantee that a command produces compatible
results on both sides.** It owns the infrastructure — queueing, transport,
optimistic apply, triggering the re-run — not the handler's logic, which is
the developer's own code on both sides. Avoiding uncoordinated wall-clock
reads or random values inside a handler (threading them through `ctx`, as
`ctx.now()` does) is the developer's problem to solve.

The recommended threading convention: `ctx.now()`'s local value is captured
into the envelope as `context.capturedAt`, and the backend's own `ctx.now()`
reads it from there rather than its own clock.

Tracked as `RISK-0002`. **There is no handler-drift testing story**
(`REQ-0034`).

## 7. Identity boundary

The trusted context is opaque, supplied through the developer's extension
point, forwarded unmodified, and **client-unforgeable within hoyadb's
plumbing** — a handler reads `ctx.identity` and cannot assign it
(`REQ-0015`). That guarantee is narrow: the backend must independently
re-derive identity from the transport's own auth and use `context.identity`
only for logging (`THR-0001`, `THR-0008`). That backend is never hoyadb's
code.

## Open

The serialization format of `args`, the concrete shape of the optional
debug execution-log attachment, and the three execution-model risks in §3
are all **unresolved** — each needs its own decision before implementation.

## Traceability

- Parents: `ARC-0001`, `ADR-0005`, `ADR-0010` … `ADR-0014`, `ADR-0025`.
- Satisfies: `REQ-0010` … `REQ-0017`.
- Depends on: `DM-0002`, `SPEC-0009`.
- Verified by: `TC-0005`, `TC-0006`, `TC-0007`, `TC-0012`.
