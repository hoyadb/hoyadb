---
source: github.com/realm/realm-js (community branch, docs/node/*.md — see per-section headers)
source_type: source
fetched: 2026-09-13
---

# Realm — Architecture (observed)

Observed, not accepted — this describes Realm's structure, not hoyadb's.
Scope note: only the still-current local database is covered here (see
[overview.md](overview.md) for why — Device Sync is deprecated/removed).

## Object schema

Source: docs/node/model-data/define-a-realm-object-model.md
(fetched 2026-09-13)

- Schema is declared per object type as a `name` + `properties` map, either
  as a plain object or as a static `schema` field on a class extending
  `Realm.Object` — the class-based form is what's used for CRUD (e.g.
  `realm.create(Car, {...})`).
- **Strongly, narrowly typed properties**: `bool`, `int`, `double`,
  `Decimal128`, `string`, `date` (maps to JS `Date`), `data` (maps to
  `ArrayBuffer`), `objectId`. A trailing `[]` makes a property a list of
  that type. Optionality is a `?` suffix on the type string (e.g. `"int?"`),
  not a separate flag — more compact than WatermelonDB's `isOptional: true`
  but the same underlying concept.
- A `primaryKey` field on the schema designates the primary key explicitly
  (WatermelonDB, by contrast, gives every table an automatic `id` and
  doesn't let you choose a different key).
- Indexing and full-text-search indexing are both opt-in per property (same
  shape as WatermelonDB's `isIndexed`).
- Class/property names can be remapped to a different stored name — useful
  for e.g. keeping a JS-idiomatic class name while the underlying storage
  name differs, or renaming without a data migration.
- **Embedded objects**: a schema can be marked `embedded: true`, meaning
  instances of it can only exist nested inside exactly one parent object,
  are deleted automatically when the parent is deleted or the reference is
  overwritten, and cannot have a primary key. This is a genuine third
  category alongside "own table with relation" and "plain scalar/list
  column" — neither RxDB nor WatermelonDB has an equivalent first-class
  embedded/owned-object concept.

**Relevance to hoyadb**: the `type?` optional-suffix syntax and explicit
`primaryKey` are minor API-shape choices, but **embedded objects as a
first-class schema concept** is a genuinely new idea relative to RxDB/
WatermelonDB — worth evaluating for hoyadb's own relational-ish model,
since "this sub-record can never be shared/orphaned and dies with its
parent" is a real, common data shape (e.g. an address on an order) that
both other references would model as either a plain relation (extra
lifecycle bookkeeping) or a denormalized JSON blob (loses queryability).

## Relationships

Source: docs/node/model-data/define-a-realm-object-model.md +
docs/node/model-data/relationships-and-embedded-objects.md
(fetched 2026-09-13)

- **To-one**: a property typed as the related object's name (optional —
  Realm *requires* to-one relationship properties to be optional; declaring
  one as required throws at runtime). Directly comparable to WatermelonDB's
  `@relation`, but expressed as a schema-level type rather than a decorator.
- **To-many**: a property typed as `"Car[]"` — the list-of-related-objects
  is a native schema construct, unlike WatermelonDB where a "has-many" is
  just a separately-issued query filtered by a foreign key. This makes
  to-many relations traversable directly as `manufacturer.cars` without a
  hand-written query.
- **Inverse relationship (`linkingObjects`)**: a native, automatically
  maintained backlink — e.g. a `Car.assignee` property that Realm keeps in
  sync with whichever `Manufacturer.cars` list currently contains it, with
  no manual bookkeeping and explicitly **not settable directly** by app
  code. This is a first-class feature; both RxDB and WatermelonDB require
  the app to write and query a separate FK/pivot manually to get the
  equivalent.
- No native many-to-many construct was found in the fetched pages (not
  fully explored — out of scope for this pass); to-many + inverse covers
  one-to-many natively.

**Relevance to hoyadb**: Realm's to-many and inverse-relationship being
native schema constructs — not query patterns the app has to write itself
— is the strongest relational-modeling feature seen across all three
references. This directly informs the "how much of relational-ish is
first-class vs. DIY" question flagged in
[../watermelondb/architecture.md](../watermelondb/architecture.md#relations):
Realm shows the more relational end of that spectrum is achievable in an
embedded local-first database, not just in a server-side relational engine.

## Live objects and reactivity

Source: docs/node/react-to-changes.md (fetched 2026-09-13)

- Core claim: "Data in Realm is live" — an object reference always reflects
  its latest saved state and reads never block; this is a stronger claim
  than RxDB/WatermelonDB's opt-in `.observe()`/`.$` — in Realm, *every*
  object/collection reference is inherently live, observing is opt-in only
  for getting a *notification*, not for the data itself staying current.
- **Three listener granularities**, mirroring RxDB's field/document/query
  split almost exactly: a **realm listener** (fires on any change anywhere,
  no detail about what changed), a **collection listener** (fires with a
  structured diff: arrays of changed `deletions`/`insertions`/
  `modifications` indices for a specific query's result set), and an
  **object listener** (fires with a `changedProperties` list, or a
  `deleted` flag, for one specific object).
- **Ordering contract**: collection-change handlers must process
  `deletions`, then `insertions`, then `modifications`, in that order —
  documented as required, not a suggestion, to avoid acting on stale
  indices.
- **A documented real limitation**: change notifications do not fire for
  changes nested more than four levels deep in embedded/related objects;
  the docs' own workarounds are "flatten your schema" or "add manual
  pull-to-refresh" — i.e. this is an accepted, unfixed limitation of the
  live-object mechanism, not a solved problem.

**Relevance to hoyadb**: the three-tier listener granularity (whole-store /
query-result-diff / single-object-diff) is now confirmed as a pattern
independently arrived at by both RxDB and Realm — good evidence it's the
right shape for hoyadb's own subscription API. The structured
insertions/deletions/modifications diff (vs. RxDB/WatermelonDB just handing
back a new full result set) is a distinct, more efficient design worth
weighing for hoyadb if consumers need to do incremental UI updates (e.g.
list diffing) rather than just re-rendering from a fresh array each time.
The four-level nesting notification limit is a valuable cautionary data
point: "live" reactivity through relations/embeds has a real, load-bearing
limit in at least one production system, worth deciding on deliberately
for hoyadb rather than discovering it after the fact.
