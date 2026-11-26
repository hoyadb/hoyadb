# Part 7 — Conflict resolution

Not named as a hard requirement in [NEEDS.md](https://gitlab.com/alocas-ia/hoyadb/-/blob/1647b178d4fb3ee925d574367d2f9d526a4bb116/legacy/NEEDS.md) (listed under
Open Questions as "Conflict resolution strategy" — undecided), but tightly
coupled to [part 5](../05-sync-mechanism/): this part is really "what
happens to the entity-diff fallback path when two writes disagree," since
the command-sync path sidesteps the question by construction (see below).
[references/comparison.md](../../comparison.md)'s "Sync unit /
conflict resolution" table and its ranked list of six strategies is the
primary source here.

## Option A — Per-field/column LWW default, deletes win, pluggable per-table hook (chosen in VISION.md)

For entity-diff writes: the most recently written value for each
individual field wins; a delete always beats a later update to the same
row; an app can override the resolver per table.

- **Precedent**: PowerSync's default backend behavior — "the last update
  (as received by the server) to each individual field wins... deletes
  always win: a deleted row ignores any later update"
  ([powersync/sync.md](../../powersync/sync.md)); cr-sqlite's
  default per-column CRDT type is also LWW
  ([comparison.md](../../comparison.md) conflict-resolution
  table) — two independent, unrelated systems converging on the same rule
  without coordinating is comparison.md's own basis for calling this "the
  closest thing to an industry default." The pluggable-hook part mirrors
  RxDB's conflict-handler object (detect + resolve, customizable) and
  PowerSync's fully backend-authored resolution point
  ([rxdb/architecture.md](../../rxdb/architecture.md),
  [powersync/sync.md](../../powersync/sync.md)).
- **Pros**: cheapest correct-enough default, backed by two independent
  sources; deletes-win removes a specific named class of bug (a stale
  update reviving a row the user meant gone); staying pluggable per table
  means Option B or D below remain available without redesigning the
  mechanism.
- **Cons**: comparison.md ranks this only as tier 3 of 6 in sophistication
  — it still silently loses one side of a genuine field-level double-write
  (e.g. two users incrementing the same counter in the same window both
  see their own value overwritten, not summed) — the exact counter-CRDT gap
  that motivates [part 5](../05-sync-mechanism/)'s command-sync path in the
  first place. This option is deliberately *not* trying to solve that; it's
  the fallback path's floor, not its ceiling.

## Option B — Per-column, content-based, client-wins-on-top-of-server-state

On conflict, start from the entire remote record, then re-apply only the
specific local columns the client actually changed since last sync.

- **Precedent**: WatermelonDB's sync algorithm — "server wins as a
  starting point, client wins per individual changed field on top of
  that" — explicitly named as distinct from both a whole-document handler
  and naive LWW
  ([watermelondb/sync.md](../../watermelondb/sync.md)).
- **Pros**: comparison.md calls this "probably the single most reusable
  concrete algorithm" in the whole reference set; more nuanced than plain
  LWW — a local edit isn't silently lost just because the server's clock
  or write-order happened to be later.
- **Cons**: needs local `_status`/`_changed` per-row bookkeeping
  (WatermelonDB bakes this into every table's schema) — real, ongoing
  storage/complexity cost, not free; still doesn't solve the counter
  problem (summing vs. overwriting is a merge-semantics question this
  approach doesn't address either). Kept as an available per-table
  override rather than the default, since it's a heavier mechanism for
  users who need it.

## Option C — Fully automatic per-column CRDT dispatch, no manual hook

Each column is assigned a CRDT type (LWW, fractional-index for ordering,
counters, ...) at the schema level; merging is fully automatic with no
app-level resolution hook at all.

- **Precedent**: cr-sqlite — "no options for manual merging... Rows are
  identified by primary key... Tables are unioned... Conflicts are then
  resolved per column, using whatever CRDT type that column was assigned"
  ([cr-sqlite/sync.md](../../cr-sqlite/sync.md)). Notably, its
  own docs and GitHub README **disagree on whether counter CRDTs are
  actually supported** — an unresolved discrepancy flagged, not
  invented-around, in that same file.
- **Pros**: handles ordering (fractional index) and, if the counter
  discrepancy resolves in its favor, counters — correctly *by
  construction*, without per-app resolver code.
- **Cons**: "no manual hook at all" is the opposite pole from Option A/B's
  pluggability — an app that hits a case the built-in CRDT set doesn't
  cover has no escape hatch in this model. The counter-support
  discrepancy is itself evidence this is harder to get fully right than it
  looks, even for the project that specializes in it. Worth revisiting
  specifically if [part 5](../05-sync-mechanism/)'s command-sync path turns
  out not to fully absorb the ordering/counter cases it's meant to.

## Option D — Deterministic winner, but the loser is preserved for the app to reconcile

A fixed algorithm picks a winning revision automatically (so every peer
converges without coordination), but the losing revision isn't discarded —
it stays fetchable, and the app decides what to do with it.

- **Precedent**: PouchDB/CouchDB — "both users will see the same winner
  once they're back online... the losing revision is not discarded," visible
  via a `_conflicts` attribute and individually fetchable by revision id;
  three named app-level patterns sit on top (manual resolve, a
  delta-function upsert via `pouchdb-upsert`, and an append-only "ledger"
  design — "accountants don't use erasers" — that avoids conflicting
  updates by never updating at all)
  ([pouchdb/sync.md](../../pouchdb/sync.md)).
- **Pros**: comparison.md calls this a genuinely distinct fourth
  philosophy — strictly safer against silent data loss than Option A/B/C,
  since nothing is ever discarded automatically; the "ledger" pattern is
  worth noting on its own even outside this option, since it structurally
  avoids conflicts rather than resolving them.
- **Cons**: defers the actual reconciliation work to the app instead of
  resolving it — every table using this option needs app code that
  actually looks at `_conflicts` and does something, or losing revisions
  accumulate unseen; same granularity as whole-document LWW (comparison.md
  ranks it tier 2 of 6, just above plain master-wins) — doesn't solve the
  counter problem either, just makes the loss visible instead of silent.
  Worth offering as a per-table option for cases where "flag it for a
  human" is preferable to any automatic rule (e.g. financial or
  safety-sensitive fields), rather than as the default.
