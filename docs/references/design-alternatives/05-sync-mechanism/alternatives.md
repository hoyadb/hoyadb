# Part 5 — Sync mechanism (what gets synced: commands vs. state)

[NEEDS.md](https://gitlab.com/alocas-ia/hoyadb/-/blob/1647b178d4fb3ee925d574367d2f9d526a4bb116/legacy/NEEDS.md): "Preferred approach: command-based
synchronization, not raw state/entity diffing... This is not a hard
requirement... a more conventional change/entity-based sync... is an
acceptable fallback." This is the single most-researched question in
[references/comparison.md](../../comparison.md) (see its
"Sync unit / conflict resolution" table and "Net implications" section).

## Option A — Command-based sync as primary, entity-diff as fallback (chosen in VISION.md)

Domain actions are named, validated commands (e.g. `add-product-to-basket`)
executed optimistically against the local store, then re-executed
authoritatively against the backend's real state; a server-controlled
context object travels alongside client-supplied args. Simple
tables/fields that don't need domain logic can instead use the entity-diff
CRUD path directly (Option B) — the two coexist per table/command, not
globally exclusive.

- **Precedent**: Zero's mutators are the concrete model — `defineMutator`
  functions collected into a named registry (`"issue.update"`), called
  optimistically client-side (`zero.mutate()`), re-executed "in a
  transaction against your database" server-side, with `ctx` an explicit
  "server-controlled context the client cannot set" — named directly as
  the mechanism for trusted fields like `ctx.userID`
  ([zero/sync.md](../../zero/sync.md)). PowerSync's `metadata`/
  `trackMetadata` field is a lighter partial precedent for the "carry
  context alongside a write" half of this, without the "server re-derives
  the result" property
  ([powersync/sync.md](../../powersync/sync.md)).
- **Pros**: directly matches NEEDS.md's stated preference; the
  counter-CRDT problem — found independently in Realm's historical OT
  (needed a special increment op) and in cr-sqlite's own
  docs-vs-README discrepancy over counter support
  ([cr-sqlite/sync.md](../../cr-sqlite/sync.md)) — is
  structurally solved by a command model (`increment` is trivially
  correct as a re-executable function, unlike a value diff); keeping
  entity-diff as a fallback means tables that don't need domain logic
  don't pay the cost of designing a command for them.
- **Cons**: comparison.md's own flagged risk applies in full — "this only
  works cleanly if mutators are deterministic given (tx state, args, ctx)"
  ([zero/sync.md](../../zero/sync.md)); Zero's docs don't fully
  spell out how to guarantee this, so hoyadb would have to solve it, not
  inherit a solution. No source does *domain-level* commands specifically
  (Zero's mutators are closer to "typed RPC with local prediction" than
  true business intent per comparison.md) — real, unprecedented design
  work remains even after choosing this option. Running two sync models
  side by side (commands + entity-diff) is itself extra surface area
  versus picking one exclusively.

## Option B — Pure entity-diff / CRUD sync (the "acceptable fallback" from NEEDS.md, standing alone)

Sync unit is create/update/delete records; no command/intent concept.

- **Precedent**: the majority approach — RxDB (whole-document
  push/pull with `assumedMasterState`/`newForkState` —
  [rxdb/architecture.md](../../rxdb/architecture.md)),
  WatermelonDB (a `Changes` object per table:
  `created`/`updated`/`deleted` —
  [watermelondb/sync.md](../../watermelondb/sync.md)),
  PowerSync (PUT/PATCH/DELETE upload queue derived straight from SQL
  statements — [powersync/sync.md](../../powersync/sync.md)),
  PouchDB (whole document per revision —
  [pouchdb/sync.md](../../pouchdb/sync.md)). 6 of 8 sources use
  this model per comparison.md.
- **Pros**: by far the most precedented option — WatermelonDB's protocol
  alone has "~13 independent backend implementations" per its own docs
  ([watermelondb/sync.md](../../watermelondb/sync.md)); no
  mutator-determinism risk since there's no function to keep deterministic,
  only data; simpler mental model for backend authors.
- **Cons**: this is exactly the model that hits the counter-CRDT wall in
  three independent sources; carries no intent/context by default (RxDB,
  WatermelonDB, PouchDB: "None" in comparison.md's context row) — doesn't
  satisfy NEEDS.md's "preserve *why* a change happened" goal on its own.

## Option C — Read-only sync; writes are the app's own problem

The sync layer only pulls authoritative state down; how writes get to the
backend at all is left entirely outside the sync layer's scope.

- **Precedent**: ElectricSQL — "does not handle writes back to Postgres —
  delegating that responsibility elsewhere"
  ([electricsql/overview.md](../../electricsql/overview.md)).
  Its own docs frame writes as a **spectrum** apps choose a point on: online
  writes → optimistic UI state → shared persisted optimistic state →
  through-the-database sync via triggers/changelog
  ([electricsql/sync.md](../../electricsql/sync.md)) — notably,
  only the last point on that spectrum works offline for writes at all.
- **Pros**: smallest, most self-contained piece to build (per
  [electricsql/overview.md](../../electricsql/overview.md)'s own
  framing); the "resolve optimistic state by watching for it to reappear in
  the authoritative stream" reconciliation idea is clean and worth
  borrowing regardless of which option is picked.
- **Cons**: directly conflicts with NEEDS.md's core offline-first
  requirement ("reads and writes always go to a local database first") —
  three of Electric's four named patterns don't work offline for writes at
  all, and the one that does (through-the-database + trigger changelog)
  is really Option A/B built on top, not an alternative to them. Rejected
  as insufficient on its own, though the spectrum framing and the
  reconciliation idea are worth keeping in mind.
