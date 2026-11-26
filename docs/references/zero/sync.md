---
source: https://zero.rocicorp.dev/docs/mutators (and docs/server-zql — see per-section headers)
source_type: doc-site
fetched: 2026-09-13
---

# Zero — Sync via custom mutators (observed)

Observed, not accepted — this describes Zero's sync mechanism, not
hoyadb's. This is the single most relevant file in this whole reference set
for NEEDS.md's preferred command-based-sync-with-context question.

## Mutators: definition and dual execution

Source: docs/mutators (fetched 2026-09-13)

- A mutator is defined with `defineMutator(validator, async ({tx, args}) =>
  {...})` — an async function, not a data shape. Mutators are collected
  into a registry (`defineMutators({...})`) which derives fully-qualified
  names (e.g. `"issue.update"`) — this named-function-with-arguments shape
  is exactly the `add-product-to-basket(...)`-style command hoyadb's
  NEEDS.md describes wanting.
- **Optimistic client execution, authoritative server re-execution**: calling
  `zero.mutate()` runs the mutator immediately against the *client-side*
  datastore — "any changes are immediately applied to open queries and the
  user sees the changes" — while a mutation record is sent to the server's
  push endpoint. The server then executes the **same mutator** (or a
  server-specific override) "in a transaction against your database." Once
  the server's result replicates back to the client, "any pending mutations
  which have been applied to the server have their local effects rolled
  back" — i.e. the optimistic local effect is a placeholder, discarded once
  the real (server) result arrives, not merged with it.
- **Client and server can run different implementations of the same named
  mutator**: `defineMutators(baseMutators, {...})` lets the server override
  specific mutators to add checks (permissions, audit logging) the client
  version doesn't perform — mirroring the queries architecture's
  client/server dual-implementation pattern (see
  [architecture.md](architecture.md#queries-language-sync-model-and-reactivity)).
- **Context, not just arguments**: mutators receive `tx` (a transaction
  object for reads/writes: `tx.mutate.<table>.insert/update/upsert/delete`,
  and `tx.run(zqlQuery)` for reads), `args` (validated, client-supplied),
  and `ctx`/`context` — a **server-controlled context the client cannot
  set**, explicitly recommended for things like the authenticated user id
  ("User cannot control ctx.userID, so this enforces authorship"). This is
  a concrete, named answer to "how do you carry trusted context alongside
  an untrusted command" — directly relevant to hoyadb's "sync commands...
  with context" phrasing in NEEDS.md.
- `tx.location` (`"client"` or `"server"`) lets one mutator body branch
  logic by where it's currently executing, rather than requiring two fully
  separate functions for every mutator.
- **Reads inside a mutator are always local**: on the client, reads only
  see already-synced/cached data; on the server, reads see everything —
  this is what lets the optimistic client-side run proceed without
  blocking on the network.
- **Transactional**: "the datastore is guaranteed to not change while your
  mutator is running," and a thrown error rolls back the *entire* mutation
  — both the client's optimistic attempt and (on the server) the database
  transaction.

## Error handling

- Client-side throw ⇒ the optimistic update is prevented/reverted before
  ever being shown as committed.
- Server-side throw ⇒ caught by `handleMutateRequest`, turned into a
  structured error response, which reverts the client's optimistic effect.
- The client can `await` a mutation's `.client` (did the optimistic attempt
  succeed locally) and `.server` (did the server actually acknowledge it)
  separately — a two-stage confirmation not seen as an explicit API surface
  in RxDB/WatermelonDB/Realm's push-confirmation models. HTTP 401/403 from
  the server puts Zero into an auth-required state; other server errors
  trigger a retry.

## Conflict resolution — as much as this pass found

The docs state directly that "mutators need to be registered with Zero
because Zero calls them during sync for conflict resolution," but (at
least on this page) don't spell out the mechanics beyond that. What *is*
explicit: the server's re-execution of the mutator is authoritative, and
any local optimistic result that turns out to differ is simply discarded
and replaced — so unlike RxDB's document-merge or WatermelonDB's per-column
merge, **there may be no separate merge step at all**: whichever result the
server's own execution of the mutator produces (against whatever the
server's data looked like *at that time*) just wins outright, since the
mutator is a function, not a value, and can be safely re-run against
different starting state. This lines up with the "server reconciliation ...
pioneered by the video game industry" claim in [overview.md](overview.md) —
client-side prediction + server-authoritative re-simulation is exactly that
technique's shape (e.g. client-side prediction in networked games). Not
verified beyond this — flagged as a gap if a deeper mutators/conflict page
exists that this pass didn't find.

## Server-side execution details

Source: docs/server-zql (fetched 2026-09-13)

- Server-side mutator/query execution runs ZQL directly against Postgres,
  via adapters for common libraries (Drizzle, Kysely, Prisma, node-postgres,
  postgres.js) wrapped as a `ZQLDatabase`. This confirms the earlier
  observation that Zero has **no bespoke server-side data store of its
  own** — the server's authoritative state is just an ordinary Postgres
  database, accessed through the same ZQL a mutator would use to read.
- Execution happens inside a transaction; `tx.dbTransaction.wrappedTransaction`
  exposes the underlying native client so app code can drop down to
  raw Drizzle/Prisma/etc. calls inside the same transaction if needed.
  `tx.location === 'server'` is the branch point used to run this
  server-only logic.
- **Named limitation, stated directly**: "ZQLDatabase currently does a read
  of your postgres schema before every transaction" — an acknowledged
  per-transaction overhead the docs flag as a potential scaling concern,
  not yet solved.

**Relevance to hoyadb**: reinforces that Zero's "authoritative side" is
just a normal relational database with no special sync-only bookkeeping
table/columns of its own (contrast with WatermelonDB's baked-in
`_status`/`_changed` schema columns) — the mutator/query re-execution model
does its own bookkeeping instead of tagging every row. The named
per-transaction schema-read overhead is a concrete, citable example of a
"pay for correctness with performance" trade-off worth keeping in mind if
hoyadb pursues a similar re-execution based approach.

## Relevance to hoyadb

This is the closest thing found across *all six* imported sources
(RxDB, WatermelonDB, Realm, and now Zero) to hoyadb's originally-preferred
"sync commands, not just resulting state" design:

- The command *is* the sync unit (a named mutator + validated args), not a
  derived diff of before/after state.
- A **trusted context object separate from client-supplied arguments** is
  a first-class, named concept — directly answering the "with context" part
  of NEEDS.md's `add-product-to-basket` example.
- Conflict handling is sidestepped rather than solved: because the unit of
  sync is a *function*, the server can just re-run it against current
  truth and treat that as authoritative, rather than needing to merge two
  independently-arrived-at states — a fundamentally different strategy from
  RxDB/WatermelonDB/Realm's all-state-based approaches (Realm's OT
  included).
- Trade-off worth flagging: this only works cleanly if mutators are
  deterministic given `(tx state, args, ctx)` — a mutator with hidden
  non-determinism (e.g. reading wall-clock time and branching on it
  differently client vs. server) would silently produce different
  client/server outcomes. Not addressed on this page; a real design
  question for hoyadb to resolve explicitly if it takes this approach,
  where domain commands could plausibly have such hidden non-determinism.
