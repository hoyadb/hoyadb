# Part 2 — Data model

[NEEDS.md](https://gitlab.com/alocas-ia/hoyadb/-/blob/1647b178d4fb3ee925d574367d2f9d526a4bb116/legacy/NEEDS.md) states a preference: "Preferably relational-ish
(as opposed to pure document or key-value), though the exact shape ...
is not yet decided."
[references/comparison.md](../../comparison.md) ("Data model /
storage" table) lays out the full spectrum found across all 8 sources.

## Option A — Own relational schema, first-class relations (chosen in VISION.md)

hoyadb defines its own schema (tables/columns), with to-one, has-many, and
many-to-many as first-class, not just FK-columns-by-convention.

- **Precedent**: WatermelonDB owns a real SQLite schema with relations, but
  only to-one is native — has-many/many-to-many are "just a filtered query"
  layered on top by convention, no dedicated schema construct
  ([watermelondb/architecture.md](../../watermelondb/architecture.md)).
  Realm and Zero go further: to-one, to-many, and inverse relations are all
  native ([comparison.md](../../comparison.md) data-model row).
  This option combines "own the schema" (WatermelonDB) with "richer native
  relations" (Realm/Zero).
- **Pros**: backend-agnostic (doesn't require an existing external schema —
  see Option D); relations usable without hand-building pivot-table query
  conventions for every many-to-many.
- **Cons**: more schema/migration machinery to build than a document store;
  WatermelonDB's own experience shows even "just" to-one relations plus a
  schema-version+migration story is real, non-trivial work
  ([watermelondb/architecture.md](../../watermelondb/architecture.md)).

## Option B — Document-oriented, relations bolted on

Schema-optional documents (JSON-shaped), with a "population"/reference
mechanism added on top to link documents.

- **Precedent**: RxDB — "Schema-based, document-oriented (not relational),
  with a 'Population' feature and 'Relations' support layered on top"
  ([rxdb/overview.md](../../rxdb/overview.md)).
- **Pros**: simplest storage model; flexible schema evolution (no rigid
  column set).
- **Cons**: works against NEEDS.md's explicit relational-ish preference;
  relations are a bolt-on, not a first-class concept.

## Option C — Fully schemaless

No schema concept at all; arbitrary JSON documents.

- **Precedent**: PouchDB — "unstructured documents, no schema concept"
  ([comparison.md](../../comparison.md) data-model row). Also the
  source with the *least* query-result-level reactivity
  (comparison.md's reactivity table) — a correlation comparison.md calls
  out directly: systems without an owned schema/query layer tend to lack
  query-result-level subscriptions, which conflicts with
  [part 4](../04-reactivity-subscriptions/)'s requirement.
- **Cons**: directly contradicts NEEDS.md's relational-ish preference;
  would put hoyadb's reactivity goal at risk per the correlation above.
  Not seriously considered.

## Option D — Don't own the schema; map onto an existing external DB

The local schema mirrors an existing server-side database's schema
(typically Postgres) rather than being hoyadb's own design.

- **Precedent**: Zero (schema declared *against* an existing Postgres DB),
  PowerSync (syncs an existing Postgres/MySQL/MongoDB/SQL Server schema
  unmodified), ElectricSQL (Shape reads an existing Postgres schema
  unmodified) — all three explicitly "No — schema maps onto/syncs/reads an
  existing DB" per [comparison.md](../../comparison.md)'s
  "Owns its own schema?" row.
- **Pros**: no schema-design or migration story to build in hoyadb itself;
  backend team's existing schema is the single source of truth.
- **Cons**: hard-couples hoyadb to a specific backend database technology
  (typically Postgres) before NEEDS.md's own "storage engine ... not
  finalized" and "sync protocol ... intentionally left open/pluggable"
  questions are settled — works against the generic-core goal in
  [NEEDS.md](https://gitlab.com/alocas-ia/hoyadb/-/blob/1647b178d4fb3ee925d574367d2f9d526a4bb116/legacy/NEEDS.md)'s Scope section. Rejected for now on that
  basis, not on technical merit.
