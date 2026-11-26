# Part 4 — Reactivity / subscriptions

[NEEDS.md](https://gitlab.com/alocas-ia/hoyadb/-/blob/1647b178d4fb3ee925d574367d2f9d526a4bb116/legacy/NEEDS.md): "Applications must be able to
**subscribe/observe** an entity or a query, and be notified as the
underlying local data changes — not just read it once."
[references/comparison.md](../../comparison.md)'s "Reactivity /
subscriptions" table compares all 8 sources' approaches directly.

## Option A — Query-result-level subscriptions, one-shot + live on the same query object (chosen in VISION.md)

A query object supports both a one-shot read and a live subscription; by
default, the subscription only re-emits when the *matched set* changes
(records entering/leaving the result), with field-level or count-only
observation as an explicit, cheaper-by-default opt-in.

- **Precedent**: RxDB — `.exec()` (Promise) / `.$` (Observable, re-emits on
  change) on the same `RxQuery`, with an "EventReduce" algorithm that
  updates the result incrementally rather than re-running the full query
  ([rxdb/architecture.md](../../rxdb/architecture.md)).
  WatermelonDB — `.fetch()`/`.observe()` on the same `Query`, default
  `observe()` re-emits only on set-membership change (not on a
  matched-record's field changing), with the more expensive per-field
  watching (`observeWithColumns`) and `observeCount()` (throttled 250ms) as
  explicit opt-ins
  ([watermelondb/architecture.md](../../watermelondb/architecture.md)).
  Two independent systems converging on the same one-object/two-consumption-modes
  shape, with the same "cheap by default, precision is opt-in" performance
  philosophy, is comparison.md's basis for calling this the best-precedented
  option in the set.
- **Pros**: matches NEEDS.md's requirement exactly; the convergent
  precedent gives real confidence in the shape; the performance guard
  (default = coarse/cheap, opt in to precision) directly serves NEEDS.md's
  "must not feel slow" constraint without extra design work.
- **Cons**: EventReduce-style incremental diffing is real, non-trivial
  engineering — RxDB names it as its own algorithm, not a small helper;
  WatermelonDB's per-query Observable caching (so N identical `.observe()`
  calls share one subscription) is a second non-trivial piece needed to
  make this perform well at scale
  ([watermelondb/architecture.md](../../watermelondb/architecture.md)).

## Option B — Multi-tier configurable granularity

Expose several named reactivity tiers explicitly, letting the app trade
precision for cost per query.

- **Precedent**: PowerSync's three tiers — basic (re-run + re-emit on any
  dependent-table change, even if the result didn't change), incremental
  (re-run + diff, only emit on an actual result change), and explicit
  delta (add/remove/update events)
  ([powersync/architecture.md](../../powersync/architecture.md)).
- **Pros**: most flexible; app authors can pick the cheapest tier that's
  correct for their UI.
- **Cons**: three behaviors to design, document, and keep consistent
  instead of one with an opt-in fast path; more surface area than Option A
  for a v1. Worth revisiting once Option A's single tier is built and its
  limits are actually felt.

## Option C — Coarse whole-table / whole-database change feed only

No query-result-level reactivity; the app subscribes to a raw change
stream and re-filters/re-queries itself in the handler.

- **Precedent**: this is what the 3 sources *without* owned schema/query
  layers fall back to — cr-sqlite (table/row/whole-DB, "no query-result-level
  reactivity yet," on its own roadmap), ElectricSQL (Shape-level only — the
  subscription primitive *is* the whole filtered table stream, no separate
  one-shot/live split), PouchDB (`db.changes({since, live})` is a
  whole-database feed; no query or collection-scoped observation found) —
  per [comparison.md](../../comparison.md)'s reactivity table
  and its explicit note that this correlates with *not* owning a
  schema/query layer (see [part 2](../02-data-model/)).
- **Pros**: much less to build — a change feed is far simpler than
  incremental query diffing.
- **Cons**: pushes real work onto every app built on hoyadb (re-filtering
  on every change event); given [part 2](../02-data-model/) already commits
  to owning the schema, comparison.md's own correlation argues hoyadb is
  positioned to do better than this, and NEEDS.md asks for observing "an
  entity or a query" specifically, not a raw feed. Rejected as the primary
  mechanism, though a raw change feed underneath Option A is a reasonable
  implementation detail (not a user-facing design choice).
