# Sync implementation guide

The Rust owner is specified by [API-0006](../design/contracts/API-0006-command-owner.md)
and [API-0009](../design/contracts/API-0009-entity-pull-reset.md). The TypeScript
examples below illustrate design rather than defining an installed SDK API.
Use [SDK integration practices](sdk-integration.md#sync-integration-practices)
and the installed declarations for actual transport, completion and reconciliation.

Configure outgoing entity tables and a trusted context provider explicitly.
Ordinary writes then generate their own patches in the common queue; an inbound
page may contain any schema-declared table selected by the backend. Table opt-in
does not authorize access. Preserve canonical envelope bytes on retry and match
entity replies by the full table/recordId/writeId tuple. Use typed Entity
corrections when the backend selects different field values or clocks.

For pull, obtain an owned request and its exact checkpoint before awaiting the
transport. Busy retains that request and response; StalePull retires the request
and requires discarding the old response and fetching again, never rebinding
old bytes to a fresh request. PullConflict requires resolving overlapping
outbound recovery before refetching from the unchanged checkpoint. Committed partial bootstrap data is
visible; readiness comes from progress. Default reset preserves recovery by
refusing to discard it. Only an explicit application choice permits discard;
successful reset invalidates prior host generations and subscriptions.

The Rust owner supports the explicit [Stop/Discard policy](../design/spec/SPEC-0040-simple-command-conflict-policy.md).
Call `set_conflict_policy(ConflictPolicy { default, commands })` on an idle owner;
the default is `ConflictReaction::Stop`, exact command-name overrides win, and
reopening resets this ephemeral policy. `rollback_resolving`,
`reconcile_resolving` and manual `discard` return `Resolution::Completed` or
`Resolution::KeptCurrent`. The latter keeps current data: rejected or unsent
eligible work becomes `Discarded` with `rolled_back=false`; a conflicting
accepted correction instead applies valid mappings and skips its writes.
Later recorded work, entity recovery and in-flight work remain protected.
The existing unit-returning methods use the same planner and report size checks.

Read the returned report directly: ordinary history trimming can remove a
reported record in the same commit. Discarded history retains its original
arguments, undo and frozen bytes, but never dispatches or acts as recoverable
work. It also prevents older binaries from opening the store until that history
is removed by normal retention; switching back to Stop does not remove it, and
there is no time guarantee for a downgrade. No format conversion runs on open.
Implementation: [owner policy and reports](../../crates/hoyadb-core/src/command/policy.rs),
[closure planning](../../crates/hoyadb-core/src/command/reconcile/discard.rs),
[binding operations](../../crates/hoyadb-bindings/src/reconciliation.rs), and
[shared behavioral cases](../../tests/conformance/conflict_policy.rs).


Validated Exclusive owners now stage ledger changes through a synchronous
[first-image transaction](../../crates/hoyadb-core/src/command/ledger_txn.rs).
Planner and storage errors restore the prior ledger; successful storage apply
installs the packet inventory, tombstone cache and session epoch before
notification delivery can suspend. Cancelling that later delivery retains the
committed write, subscription registrations and previously queued snapshots.
Unfinished groups recover current rows and counts through refresh or a later
write, preserving count deadlines; this is not a general guarantee that a
cancelled mutation rolls back. See [API-0005](../design/contracts/API-0005-reactive-query-subscriptions.md)
and [DEF-0061](../design/defects/DEF-0061-cancelled-publication-loses-subscriptions.md).
Reconciliation
reads closure, correction, guard and mapping identities plus actual referencing
rows; pull and merge retain only their touched row and metadata images.

The [opaque delta boundary](../../crates/hoyadb-core/src/storage/sync.rs) keeps
model, packet and row comparisons in one adapter transaction. Unproven owners
and the provided adapter fallback retain full validation and full inventory CAS.
Reset and entity reconfiguration also retain full plans. Existing format-2
metadata packets preserve their ordinal encoding, so a middle insertion still
rewrites its required suffix; append and unchanged identities use the cached
rank map. These internals do not grant another writer permission to bypass an
Exclusive store's ownership contract. The acceptance cases are in
[TC-0041](../design/tests/cases/TC-0041-storage-isolation-and-scaling.md).

New entity ledgers use format 3: metadata packet IDs encode the table and record
identity, so inserting a metadata entry does not renumber unrelated packets.
The marker is inside `entity:state`; the outer entity packet format remains 2.
Existing format-2 stores retain their encoding through open, both reset modes,
reconfiguration and rebinding. There is no implicit conversion. Older binaries
cannot open format-3 entity state. The explicit offline operator tool
`hoyadb-convert-ledger` (SPEC-0037 R11; built only with the non-default
`ledger-convert-tool` feature) converts a closed store between formats 2 and 3
after a read-only validation, a byte-verified backup and one reserved-writer
transaction; it never runs from an SDK or owner. See [API-0009](../design/contracts/API-0009-entity-pull-reset.md).

> Supporting guide. Current design authority: [Transport integration](../design/spec/SPEC-0013-transport-integration.md); [HTTP contract](../design/contracts/API-0001-http-sync-transport.md); [Command protocol](../design/spec/SPEC-0008-command-protocol.md).
> Code and package names are illustrative design examples; no SDK implementation
> is supplied by this guide. Current API contracts define the supported SDK surface.
> The backend example leaves one transaction question unresolved: throwing
> `CorrectedError` from inside `db.transaction` may roll back the writes even
> though the recorded outcome says the command happened. Its correction path
> requires a defined transaction model before implementation.

[transport-http-example.md](../design/contracts/API-0001-http-sync-transport.md) fixes the *wire
shape* — what bytes move, in what request/response format. It does not
show what a developer actually has to build on either end of that wire:
server-side, a process for authenticating a request, dispatching a
[command](glossary.md#command) from an
[entity-diff](glossary.md#entity-diff-fallback-path) write, running
the right logic, and replying with a
[`WireOutcome`](../design/spec/SPEC-0008-command-protocol.md#4-reliability);
client-side, the actual object that performs the HTTP (or other transport)
call hoyadb itself never makes. This doc walks through one concrete way to
build both halves.

This guide covers sync wiring: client transport, backend dispatch, authorization
and authoritative outcomes. SDK lifecycle and non-sync integration are described
in the platform guides and `API-0011`.

## Non-goals

Carried forward from [VISION.md](../design/vision/VIS-0001-hoyadb.md)'s "What we deliberately
avoid", applicable to this doc:

- **hoyadb implementing authentication itself.** Every identity check
  below is code the developer writes and owns; hoyadb never runs on the
  backend and never sees this code.
- **hoyadb verifying or guaranteeing command determinism between a client
  and server run.** The `ctx.now()` convention shown below is one way to
  keep the two runs compatible — it doesn't make hoyadb check or enforce
  that compatibility.
- **hoyadb making network calls on the integrating app's behalf.**
  Everything in this doc runs on infrastructure hoyadb has no part in.

## What's fixed vs. what this guide chooses

Some of what follows is hoyadb's actual contract (fixed, the same for
every implementation): the envelope shapes from
[command-protocol.md](../design/spec/SPEC-0008-command-protocol.md) and
[entity-diff-fallback.md](../design/spec/SPEC-0010-entity-diff-fallback.md), the `CommandOutcome`
states, the [idempotency key](glossary.md#idempotency-key), the
[schema version](glossary.md#schema-version) field. Some of it is
this guide's own reference convention, chosen because hoyadb deliberately
leaves it to the backend implementer — the command-vs-entity-diff dispatch
rule below, the identity-re-derivation step, the `ctx.now()`-threading
convention. Each is called out as this doc's own choice where it isn't
already a hoyadb-level decision, the same way
[transport-http-example.md](../design/contracts/API-0001-http-sync-transport.md) labels its own HTTP
status codes as "this reference transport's own convention," not a
hoyadb rule.

## Client-side: implementing a real transport

hoyadb never opens a network connection itself — it calls into a
`transport` object the developer provides at
[`openDatabase`](platform-bindings.md#web-wasm) time,
whenever there's a command/entity-diff to push or a pull is due. The
shape [09-testing.md](testing.md#a-fake-transport-for-sync-tests)'s
`createFakeTransport` already implies (`openDatabase({ schema, transport
})`) is testing-specific sugar built on top of a plainer interface — the
real one only needs to forward whatever envelope it's given and return
whatever comes back, since dispatch, business logic, and outcome mapping
all happen server-side (see above), not in the transport itself:

```typescript
interface Transport {
  // Always called, even for a single queued write (a batch of one) — see
  // below for why there's no separate single-write method. Returns
  // `WireOutcome[]` (command-protocol.md), not `CommandOutcome[]` — the
  // correlation-bearing wire shape, not the app-facing one.
  push(envelopes: object[]): Promise<WireOutcome[]>;
  pull(since: string | null): Promise<{ checkpoint: string; changes: unknown[] }>;
}
```

hoyadb calls `push()` automatically whenever the
[offline queue](../design/spec/SPEC-0009-offline-queue.md) has one or more writes in the `queued`
state eligible to send (an initial send, or an automatic retry — see
[offline-queue.md](../design/spec/SPEC-0009-offline-queue.md#retry)), passing
them in the same local execution order
[command-protocol.md](../design/spec/SPEC-0008-command-protocol.md#3-handler-execution-model)
guarantees, and calls `pull()` on its own sync schedule or when the
developer calls
[`db.sync.retryPending()`](../design/spec/SPEC-0009-offline-queue.md#retry)-style
APIs. **There's no separate decision point where hoyadb chooses to batch**
— `push` is the one method it calls, always, whether there's one
queued write or several; a single write just means an array of length
one. This is ADR-0026's "batching is the default" answer, reflected directly
in the interface shape: hoyadb's core has nothing to decide, and the
`Transport` implementation has nothing to switch between.

The default reference implementation, wrapping
[transport-http-example.md](../design/contracts/API-0001-http-sync-transport.md)'s endpoint — one
request, the whole array:

```typescript
function createHttpTransport(baseUrl: string, getAuthToken: () => Promise<string>): Transport {
  const headers = async () => ({
    "Content-Type": "application/json",
    Authorization: `Bearer ${await getAuthToken()}`,
  });

  return {
    async push(envelopes) {
      const res = await fetch(`${baseUrl}/hoyadb/commands`, {
        method: "POST",
        headers: await headers(),
        body: JSON.stringify(envelopes),
      });
      return res.json(); // array of WireOutcome, per transport-http-example.md's ADR-0026 section
    },
    async pull(since) {
      const res = await fetch(`${baseUrl}/hoyadb/changes?since=${since ?? ""}`, {
        headers: await headers(),
      });
      return res.json();
    },
  };
}

const db = await openDatabase({
  schema,
  transport: createHttpTransport("https://api.example.com", () =>
    myAppsOwnAuth.getVerifiedIdentityToken()
  ),
});
```

**Opting into one-request-per-write** is something a developer's own
`push` implementation does internally, not a different interface method
or a different URL — it calls the exact same endpoint once per envelope,
each with a length-one array, instead of once with the whole array:

```typescript
function createSplitHttpTransport(baseUrl: string, getAuthToken: () => Promise<string>): Transport {
  const headers = async () => ({
    "Content-Type": "application/json",
    Authorization: `Bearer ${await getAuthToken()}`,
  });

  return {
    // Opts out of hoyadb's default batched behavior — one request per
    // write instead, e.g. to match an existing single-resource backend
    // that expects to see one write per request. hoyadb still calls this
    // as `push`, with the full array; only this function's own internals
    // (how many HTTP requests it makes) differ from the default above.
    //
    // Sent SEQUENTIALLY, not with Promise.all: `envelopes` arrives in the
    // local execution order command-protocol.md's "Handler execution
    // model" guarantees, and a dependent write (e.g. a note referencing an
    // order that hasn't been acknowledged yet) can only be resolved
    // correctly if the backend sees the order first. Firing all requests
    // concurrently would race them against each other and silently break
    // that ordering — this opt-out changes request *count*, not order.
    async push(envelopes) {
      const outcomes = [];
      for (const envelope of envelopes) {
        const res = await fetch(`${baseUrl}/hoyadb/commands`, {
          method: "POST",
          headers: await headers(),
          body: JSON.stringify([envelope]), // still an array — just length one
        });
        outcomes.push(...(await res.json())); // an array of one outcome
      }
      return outcomes;
    },
    async pull(since) {
      const res = await fetch(`${baseUrl}/hoyadb/changes?since=${since ?? ""}`, {
        headers: await headers(),
      });
      return res.json();
    },
  };
}
```

**Dependency eligibility:** the note/order example in the preserved code comment
above is historical rationale. The core passes only eligible writes to `push`.
A command referencing an unresolved local ID stays queued until an
`applied`/`corrected` outcome supplies the real ID; acknowledgement alone does
not release it. Sequential HTTP requests preserve the order of eligible writes.
See [dependent-write ordering](../design/spec/SPEC-0009-offline-queue.md#ordering-across-dependent-writes)
and the [corrected walkthrough](walkthrough.md#4-reference-it-before-its-confirmed).

Whatever a handler's `ctx.now()` call returns during local execution is
captured by hoyadb's core into the outgoing envelope's `context.capturedAt`
field automatically — the same way `context.identity` and `idempotencyKey`
are already attached without the developer's own code doing it. A
`Transport` implementation never populates this itself; it only forwards
whatever envelopes hoyadb hands to `push`, `capturedAt` already
included on each one. See [command-protocol.md](../design/spec/SPEC-0008-command-protocol.md#1-shape-of-a-command)'s
envelope example, which shows this field.

### Handling a write — `POST /hoyadb/commands`

One route. The request body is always an array (ADR-0026 — see
[transport-http-example.md](../design/contracts/API-0001-http-sync-transport.md)), even for a
single write, so there's no separate single-vs-batch branch here either.

#### 1. Authenticate the request

Don't trust `context.identity` in the request body as the source of truth
for *who this request is from* — hoyadb carries that field faithfully, but
never validates it (see
[command-protocol.md](../design/spec/SPEC-0008-command-protocol.md#7-identity-boundary)).
Re-derive the real identity from whatever the transport's own auth
actually is — in the HTTP reference transport, the `Authorization` header:

```typescript
async function authenticate(req: Request): Promise<Identity> {
  const token = req.header("Authorization")?.replace("Bearer ", "");
  if (!token) throw new UnauthorizedError();
  // The developer's own auth system — hoyadb has no part in this call.
  return myAppsOwnAuth.verifyToken(token);
}
```

Use the result of this check as the identity for every membership and
authorization decision below. `context.identity` in the body is available
for logging/cross-checking, never as the authorization source.

#### 2. Resolve each entry, independently, in order

```typescript
async function handleWrite(req: Request, res: Response) {
  const identity = await authenticate(req);
  // Independent per-entry resolution, in array order — no cross-entry
  // transaction (ADR-0026): one entry failing doesn't roll back or block the
  // others. A single-element array (the common case) still goes through
  // this same loop; there is no separate single-write branch to keep in
  // sync with this one.
  const outcomes = [];
  for (const entry of req.body) {
    try {
      outcomes.push(await resolveWrite(entry, identity));
    } catch {
      // resolveCommand/resolveEntityDiff only throw for a REAL failure
      // (not a business rejection/correction — those are returned, not
      // thrown, per §3's catch block). Per that same contract, a real
      // failure is deliberately never recorded as an outcome. Skip this
      // entry in the response array rather than letting the throw abort
      // the whole request: the client sees no outcome for this entry's
      // idempotency key, which it already treats exactly like a dropped
      // response — still `sent`, eligible for retry (see
      // transport-http-example.md) — without blocking the other,
      // independently-resolved entries in this same request.
    }
  }
  return res.status(200).send(outcomes);
}

// One payload down to one outcome. Both payload shapes travel over the
// same array entries — this reference implementation's own convention:
// the presence of a `command` field routes to the command-handler path;
// the presence of `table` + `op` routes to the entity-diff apply path —
// the two shapes don't overlap, so this dispatch is unambiguous in
// practice, it just isn't written down anywhere else.
async function resolveWrite(body, identity) {
  if ("command" in body) return resolveCommand(body, identity);
  if ("table" in body && "op" in body) return resolveEntityDiff(body, identity);
  // Best-effort correlation: use whichever key field is actually present,
  // since a payload this malformed may carry neither. A genuinely keyless
  // entry can't be correlated at all — an inherent limit of a payload that
  // doesn't match either known shape, not something to solve here.
  return {
    status: "rejected",
    reason: "unrecognized payload shape",
    ...(body.idempotencyKey ? { idempotencyKey: body.idempotencyKey } : {}),
    ...(body.recordId ? { recordId: body.recordId } : {}),
  };
}
```

#### 3. The command path

```typescript
// `tx`/`db` throughout this section are the backend's OWN database handle
// — its own separate infrastructure, nothing to do with hoyadb's client
// `db` object. `tx.table(...)` doing reads here (e.g. `findById` below)
// is not the same `db.table()` client-side code reserves for writes only
// (see command-protocol.md's handler example) — the two are unrelated
// APIs on unrelated databases that happen to share a method name.

// The backend's own registry of command handlers — the server-side half
// of what command-protocol.md's `defineCommand` declares client-side.
// Authored independently from the client handler; see command-protocol.md's
// "Determinism" section for why these are two separate implementations,
// not shared code.
const commandHandlers = {
  "add-order-v1": addOrderV1Handler, // see worked example below
  // ...one entry per command name+version
};

// The handler signals its two possible non-`applied` business outcomes by
// throwing one of two error types (see `addOrderV1Handler`'s
// `RejectedError` below for the shape) — `RejectedError` means the
// command didn't happen; `CorrectedError` means it happened but the
// result differs from what the client guessed (e.g. a different total
// after a discount rule the client didn't know about, carried in
// `err.correctedResult`). Any other thrown error is a real failure (a bug,
// a DB error), not a business outcome — see the catch block below for why
// that distinction matters.
async function resolveCommand(body, identity) {
  const handler = commandHandlers[body.command];
  if (!handler) {
    // Still correlated, even on this earliest-possible rejection — every
    // WireOutcome needs its idempotencyKey/recordId (command-protocol.md's
    // WireOutcome section) or hoyadb's core can never match it back to the
    // pending local command, which would otherwise retry this forever
    // without the app ever finding out it's permanently rejected.
    return { status: "rejected", reason: "unknown command", idempotencyKey: body.idempotencyKey };
  }

  // Determinism threading (command-protocol.md's "Determinism" section):
  // ctx.now() must return the SAME value the client's handler used when
  // it ran locally, not the server's own wall clock. This reference
  // implementation's own convention: the client-captured value travels in
  // the envelope as `context.capturedAt`, and the backend's `ctx.now()`
  // reads it from there instead of calling the system clock. Any other
  // non-deterministic input a handler needs (a random id, say) would be
  // threaded through the same way — this is one worked instance of the
  // general pattern, not a separate mechanism.
  const ctx = {
    identity, // the re-derived identity from step 1, not the body's copy
    now: () => new Date(body.context.capturedAt),
  };

  try {
    // The idempotency check, the handler's own writes, and recording the
    // outcome all happen in ONE transaction, against a real
    // `idempotency_keys` table with a unique constraint on `id` — not a
    // separate get()-then-run-then-set() sequence. That closes two real
    // gaps the separate-store version would have: (a) two concurrent
    // retries of the same key both passing a "not found yet" check before
    // either commits — the unique constraint makes the second insert fail
    // and that whole transaction (including its copy of the handler's
    // writes) rolls back, rather than double-applying; (b) a crash
    // between the handler's writes committing and the outcome being
    // recorded — there's no such window, since both commit together or
    // neither does.
    return await db.transaction(async (tx) => {
      const existing = await tx.table("idempotency_keys").findById(body.idempotencyKey);
      if (existing) return existing.outcome;

      const result = await handler(tx, body.args, ctx);
      const outcome = { status: "applied", result, idempotencyKey: body.idempotencyKey };
      await tx.table("idempotency_keys").insert({ id: body.idempotencyKey, outcome });
      return outcome;
    });
  } catch (err) {
    // Only a deliberate business outcome gets recorded — a rejection or a
    // correction the handler explicitly threw. Anything else (a DB error,
    // a bug in the handler, or the concurrent-loser's own unique-constraint
    // violation from the insert above, which also lands here) is NOT a
    // business outcome and must NOT be recorded: it's re-thrown instead,
    // failing this request and leaving the idempotency key unclaimed so a
    // retry re-enters this function and actually re-attempts the handler.
    // Silently filing a transient failure under "corrected" would poison
    // that key forever — every future retry would just replay the
    // malformed outcome instead of ever running the handler again.
    let outcome;
    if (err instanceof RejectedError) {
      outcome = { status: "rejected", reason: err.message, idempotencyKey: body.idempotencyKey };
    } else if (err instanceof CorrectedError) {
      outcome = { status: "corrected", result: err.correctedResult, idempotencyKey: body.idempotencyKey };
    } else {
      throw err;
    }

    // `insertIfAbsent`: a single atomic "insert, or return the existing
    // row if the unique constraint already has one" — e.g. `INSERT ...
    // ON CONFLICT (id) DO NOTHING RETURNING *`, then a follow-up read if
    // that returns nothing. Needed here because two concurrent rejections
    // of the same key can race the same way two concurrent successes can
    // (see the comment above `db.transaction` above) — whichever loses
    // must read back the winner's outcome, not fail outright.
    // insertIfAbsent resolves to the *stored row* (`{ id, outcome }`), not
    // the bare outcome — unwrap it so this catch path returns the same
    // flat `{ status, ..., idempotencyKey }` shape the success path above
    // does. Skipping this step is the kind of bug that only shows up on
    // the rejected/corrected path, easy to miss since it never affects
    // the common (applied) case.
    const stored = await db.transaction((tx) =>
      tx.table("idempotency_keys").insertIfAbsent(body.idempotencyKey, { id: body.idempotencyKey, outcome })
    );
    return stored.outcome;
  }
}
```

#### 4. The entity-diff path

Per [entity-diff-fallback.md](../design/spec/SPEC-0010-entity-diff-fallback.md)'s explicit rule,
this path gets the *same* authorization check a command would, not a
weaker one:

```typescript
async function resolveEntityDiff(body, identity) {
  // Same authorization bar as a command — see entity-diff-fallback.md's
  // "Lower-ceremony means skipping domain logic, not authorization."
  if (!(await isAuthorized(identity, body.table, body.recordId))) {
    return { status: "rejected", reason: "not authorized", recordId: body.recordId };
  }

  // Per-field last-write-wins, tie broken by real arrival order at the
  // backend (conflict-resolution.md's ADR-0017) — done inside a transaction so
  // two near-simultaneous writes can't both "win":
  await db.transaction(async (tx) => {
    await applyFieldsWithLWW(tx, body.table, body.recordId, body.changes, body.clientTimestamp);
  });

  return { status: "applied", recordId: body.recordId }; // used to correlate this outcome in the response array
}
```

Reusing `resolveCommand`/`resolveEntityDiff` as-is, unchanged by how many
entries arrived, is what keeps a many-entry request's resolution identical
to a one-entry request — nothing about idempotency dedup, the transaction
boundary, or the authorization check depends on array length.

### Handling a pull — `GET /hoyadb/changes`

```typescript
async function handleChanges(req: Request, res: Response) {
  const identity = await authenticate(req);
  // Membership rules compute what this identity is entitled to hold —
  // see scoping-topology.md. Not derived from the request in any other way.
  const buckets = await computeMembership(identity);
  const since = req.query.since;

  const changes = await db.queryChangesSince(buckets, since);
  return res.status(200).send({
    checkpoint: encodeCheckpoint(changes),
    changes,
  });
}
```

### Worked example: `add-order-v1` server-side

The backend half of [command-protocol.md](../design/spec/SPEC-0008-command-protocol.md#1-shape-of-a-command)'s
worked client example — independently authored, not shared code, per that
doc's "Determinism" section:

```typescript
async function addOrderV1Handler(tx, args, ctx) {
  // The independent, authoritative check the client handler can't make —
  // it only sees local/cached data. Omitting the client handler's
  // first-order-discount recompute here to keep this example focused on
  // that check; a real backend handler would include both.
  const stock = await tx.table("products").findById(args.items[0].productId);
  if (stock.quantity < args.items[0].quantity) {
    throw new RejectedError(`product ${args.items[0].productId} is no longer available`);
  }

  const order = await tx.table("orders").insert({
    customer_id: args.customerId,
    status: "pending",
    placed_at: ctx.now(), // reads the client-captured value, not this server's clock
  });
  for (const item of args.items) {
    await tx.table("order_items").insert({
      order_id: order.id,
      product_id: item.productId,
      quantity: item.quantity,
    });
  }
  return order;
}
```

This is the independent, authoritative check that gives the command path
its conflict-resolution property — see
[command-protocol.md](../design/spec/SPEC-0008-command-protocol.md#3-handler-execution-model)'s
"backend re-run is itself the resolution."

## What this doesn't decide

- A transaction/chain key for grouping commands that don't reference each
  other's data but are logically one operation — still open, unlike
  batching, [resolved as ADR-0026 above](#2-resolve-each-entry-independently-in-order); see
  [offline-queue.md](../design/spec/SPEC-0009-offline-queue.md#open). Per-command
  status, previously also open here, is resolved as ADR-0028 — see
  [observability.md](../design/spec/SPEC-0015-queue-observability.md#per-command-status).
- Backend language/framework — the code above is illustrative
  TypeScript/Node-flavored pseudocode, the same way
  [transport-http-example.md](../design/contracts/API-0001-http-sync-transport.md) is illustrative
  HTTP; nothing here requires a specific backend stack.
