---
source: https://github.com/Nozbe/WatermelonDB (docs-website/docs/docs/, multiple files — see per-section headers)
source_type: source
fetched: 2026-09-13
---

# WatermelonDB — Architecture (observed)

Observed, not accepted — this describes WatermelonDB's structure, not
hoyadb's.

## Base objects

Source: Implementation/Architecture.md (fetched 2026-09-13)

- `Database` — root object; owns one `DatabaseAdapter` and a map of
  `Collection`s.
- `DatabaseAdapter` — the boundary between Watermelon's reactive object
  world and the low-level imperative database (SQLite or LokiJS on web).
  Analogous in role to RxDB's `RxStorage`, though WatermelonDB's adapter
  concept is narrower (mainly SQLite vs. LokiJS, not an open plugin
  ecosystem the way RxStorage is).
- `Collection` — manages all records of one kind: holds a `RecordCache` of
  already-fetched records, exposes `find`/`query`/`create`, implements
  fetch/update/delete.
- `Model` — one record instance; a subclass (`Post`, `Comment`, ...)
  declares `table` and `associations` statically, exposes `update()`,
  `markAsDeleted()`, `destroyPermanently()`, and record-level `observe()`.
  Convention: `Model` owns a single instance's state; `Collection` owns the
  whole set and is delegated to for anything collection-visible (e.g.
  `model.markAsDeleted()` updates local state then delegates to the
  collection to notify observers and persist the delete).
- `Query` — built via `collection.query()`; wraps a `QueryDescription`
  (the actual condition structure, built from `Q.*` helpers) and exposes
  `observe()`/`fetchCount()`, delegating the real work to `Collection`.
  Caches the `Observable`s it creates so repeated `.observe()` calls on an
  equivalent query share one underlying subscription rather than each
  re-running the query independently.
- Most non-object-owning logic (query condition matching, observer
  reload/field-diff strategies, decorators) is implemented as free/pure
  functions rather than methods, by explicit design intent ("objects and
  classes are meant to be as minimal as possible").

**Relevance to hoyadb**: `Database` → `Collection` → `Model`/`Query`, with
one narrow `DatabaseAdapter` seam to the real storage engine, is a clean
minimal shape worth comparing directly against hoyadb's own core-object
design once that's drafted. The caching of per-query Observables (so
identical queries share a subscription) is a concrete answer to "how do you
avoid N independent recomputations for N components observing the same
data" — directly relevant to hoyadb's reactive-subscription + performance
requirements together.

## Schema / data model

Source: Schema.md (fetched 2026-09-13)

- Explicit two-layer model: app-facing **Models/Collections** sit on top of
  an **underlying database schema** of tables/columns (SQLite or, on web,
  LokiJS) — the schema must be declared upfront (`appSchema`/`tableSchema`),
  it is not inferred from the Model classes.
- Column types are deliberately minimal: `string`, `number`, or `boolean`
  only (dates are stored as `number` Unix timestamps; there is no native
  date/object/array column type).
- **Relations are plain foreign-key columns by convention**, not a distinct
  schema construct: a string column named `<thing>_id` (e.g. `post_id`)
  is the relation; there's no dedicated "reference" column type. Naming
  conventions are enforced by convention only (snake_case tables/columns,
  `is_`-prefixed booleans, `_at`-suffixed timestamp numbers), not by the
  schema validator.
- **Every table automatically gets an `id` column**, plus **reserved
  `_status` and `_changed` columns used internally for sync** — explicitly
  documented as "you shouldn't touch them yourself." This means sync-related
  bookkeeping (dirty/changed tracking) is baked into every table at the
  schema level, not layered on separately per table.
- **Schema changes require a version bump**; without a migration, bumping
  the version wipes the local database on next launch. Migrations (a
  separate mechanism, not detailed in this pass) are required once an app
  has shipped, to change schema without data loss.
- **Indexing is opt-in per column** (`isIndexed: true`), with explicit
  guidance: index most `_id` foreign-key columns (fast joins/lookups),
  occasionally index booleans, essentially never index free-text `string`
  columns or `_at` timestamps — framed as a real create/update-speed and
  disk-size trade-off, not a free performance switch.
- **Escape hatch**: `unsafeSql` lets an app inject/rewrite the generated SQL
  per table or globally, explicitly marked as an unstable API subject to
  breaking changes.

**Relevance to hoyadb**: this is the most directly on-point precedent for
the "relational-ish" data model question — a genuinely relational
(SQLite) schema with FK-by-convention relations, not a document store with
relations bolted on (contrast with RxDB, see
[../rxdb/overview.md](../rxdb/overview.md)). The baked-in `_status`/
`_changed` per-row sync-tracking columns are a concrete, minimal mechanism
worth weighing for hoyadb's own change-tracking, and the required
version+migration story is a real NFR to plan for (schema evolution without
data loss) rather than an afterthought.

## Relations

Source: Relation.md (fetched 2026-09-13)

- A `Relation` is built from exactly the FK column described in Schema.md
  plus a `@relation('<table>', '<fk_column>')` (or `@immutableRelation` for
  a relation that can never be reassigned, which is also a stated
  performance optimization) decorator on the owning Model.
- **Lazy by default**: `comment.author` returns a `Relation` wrapper
  object, not the related record — reading the actual `User` requires
  either `.fetch()` (Promise) or `.observe()` (Observable, same duality as
  Query — see [../rxdb/architecture.md](../rxdb/architecture.md) for RxDB's
  analogous Promise/Observable split on queries). `.id` is available
  synchronously without loading the related record at all.
- **Belongs-to only, natively**; has-many is expressed as a *query* (e.g.
  `post.comments` isn't a `Relation`, it's a `Query` filtered by
  `post_id`), and many-to-many requires manually building a pivot table +
  model (`PostAuthor`) with two `@immutableRelation`s and querying through
  it (`Q.on('post_authors', 'post_id', this.id)`) — there is no native
  many-to-many construct, it's a documented convention on top of the same
  primitives.

**Relevance to hoyadb**: shows the practical cost of a minimal relational
primitive set — belongs-to is native, everything else (has-many,
many-to-many) is a derived query pattern layered on top rather than first-
class schema support. Worth deciding early whether hoyadb's own "relational-
ish" model gives has-many/many-to-many first-class treatment or accepts this
same "just a filtered query" approach.

## Query execution and observing

Source: Query.md (fetched 2026-09-13; note: `Advanced/Flow.md` was
mistakenly fetched first for this topic — see tracker Notes — it's about
the Flow type-checker, not reactivity)

- Same one-shot-vs-live duality seen elsewhere (RxQuery's `.exec()`/`.$`,
  Relations' `.fetch()`/`.observe()`): `query.fetch()`/`.fetchCount()`
  return a Promise for a point-in-time read; `.observe()`/`.observeCount()`
  return an Observable that emits on every relevant change. In practice
  most reads happen implicitly via `withObservables(...)` binding a React
  component directly to a query/relation.
- `observeCount()` is throttled to at most one emission per 250ms by
  default (disable via `observeCount(false)`) — an explicit, named
  performance guard against over-triggering re-renders on rapid successive
  writes.
- Default `observe()` on a query only re-emits when the *matching set*
  changes (records added/removed from the result); it does **not** re-emit
  just because a field changed on an already-matched record. Watching
  individual field changes on matched records requires the more expensive
  opt-in `observeWithColumns(['foo', 'bar'])`.

**Relevance to hoyadb**: the throttling and the "set-membership-only by
default, field-level by explicit opt-in" split are two concrete,
named-in-the-docs performance levers — both worth deliberately deciding on
for hoyadb's own query-subscription design rather than defaulting to "always
emit on every possible change," which is the more expensive behavior these
docs treat as opt-in.
