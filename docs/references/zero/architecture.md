---
source: https://zero.rocicorp.dev/ (multiple pages, see per-section headers)
source_type: doc-site
fetched: 2026-09-13
---

# Zero — Architecture (observed)

Observed, not accepted — this describes Zero's structure, not hoyadb's.

## Schema and relationships

Source: docs/schema (fetched 2026-09-13)

- Tables are defined in code via a `table()` builder: `.columns({...})` +
  `.primaryKey(...)`. Column types: `boolean()`, `number()`, `string()`,
  `json()`, `enumeration<...>()`. Optionality is `.optional()` (a method,
  not a type suffix — contrast with WatermelonDB's `isOptional: true` flag
  and Realm's `"type?"` suffix).
- Explicit distinction: `null` (explicit absence of value) vs. `undefined`
  (use the backend's default) — a nuance not seen called out this
  explicitly in RxDB/WatermelonDB/Realm's schema docs.
- `.from('org_id')` remaps a TypeScript-side field name to a different
  underlying (Postgres) column/schema name — including reaching into a
  different Postgres schema entirely (`table('event').from('analytics.event')`).
- Compound primary keys are supported directly: `.primaryKey('orgID', 'userID')`.
- **Relationships are declared separately from tables**, via a
  `relationships(table, ({one, many}) => ({...}))` function specifying
  `sourceField`/`destField`/`destSchema` explicitly — both to-one (`one()`)
  and to-many (`many()`) are native declarations (not, as in WatermelonDB,
  a has-many being just a manually-issued query).
- **Many-to-many** is expressed by chaining relationship hops through a
  junction table, with an explicit documented limit: "only two levels of
  chaining are supported."
- Compound-key relationships and **circular references** are both
  explicitly supported.
- All tables/relationships are assembled via `createSchema({tables, relationships})`
  and registered once with TypeScript (`DefaultTypes`) so types propagate
  without manual threading.

**Relevance to hoyadb**: this is a schema built directly against an
existing Postgres schema (via `.from()` remapping), not a schema Zero owns
independently — a notably different posture from RxDB/WatermelonDB/Realm,
which all define their own canonical local schema. Native to-many *and*
many-to-many (with a stated 2-level chaining limit) puts Zero at least as
relationally rich as Realm's native-relations approach (see
[../realm/architecture.md](../realm/architecture.md#relationships)), and
ahead of WatermelonDB's query-based has-many/manual-pivot approach — good
evidence that a schema-declared many-to-many is achievable, at some
complexity cost (the 2-level chaining cap is a concrete, named limitation
worth expecting if hoyadb goes this route).

## Queries: language, sync model, and reactivity

Source: docs/queries (fetched 2026-09-13)

- Queries are written in **ZQL** (Zero Query Language) and defined via
  `defineQuery(validator, (args) => zqlExpression)` — the validator
  (Zod/Standard Schema) is mandatory because, per the docs, "Zero queries
  run on both the client and on your server. In the server case, the
  parameters come from the client and are untrusted."
- **The same named query runs on both sides**, but the two implementations
  are allowed to differ — e.g. the server-side version can add a permission
  filter the client-side one doesn't have. Queries are registered centrally
  (`defineQueries({...})`) rather than constructed ad hoc.
- **Sync is query-driven, not whole-dataset**: "Zero syncs the union of all
  active queries' results" — the client holds "a consistent partial replica
  of the backend database" made up of exactly what active queries need,
  not a full table replica. This is architecturally distinct from RxDB/
  WatermelonDB/Realm, all of which sync a client's full accessible dataset
  (filtered by permissions) via pull/push regardless of what's currently
  queried on-screen.
- **Sync pipeline**: a server component ("zero-cache") receives changes
  from Postgres via **logical replication**, recomputes which active
  queries are affected, and pushes incremental row changes to clients —
  i.e. the source of truth is Postgres itself, not a bespoke sync-specific
  store.
- **Reactivity**: `useQuery(query)` (React) / `useQuery(() => query)`
  (SolidJS) subscribe a component directly to a query, returning cached
  local results instantly while the server resolves/streams the rest;
  a result-status field (`.type === 'complete'`) lets the app distinguish
  partial/loading from fully-synced results — a distinction not seen
  named this explicitly in RxDB/WatermelonDB/Realm.
- **Explicit resource-management levers**: each query has a **TTL**
  governing how long its synced data is retained locally after the query
  deactivates (`preload()` defaults to no caching; other queries default to
  5 minutes) — framed explicitly as "queries consume resources on both the
  client and server," so keeping stale query subscriptions alive
  indefinitely has a real cost. `zero.preload()` fetches without
  materializing JS objects (cheaper than `useQuery` for large result sets).
  `zql.issue.orderBy(...).limit(...)` run directly against already-synced
  local data with **no server round trip at all** for data already present
  locally (e.g. typeahead). Queries can be conditionally paused by passing
  `undefined` until their inputs are ready, avoiding wasted sync requests.

**Relevance to hoyadb**: "sync exactly the union of what's actively
queried, with a TTL after de-registration" is a genuinely different and
more resource-conscious sync-scoping model than any of RxDB/WatermelonDB/
Realm's "replicate your whole accessible dataset" approach — directly
relevant to hoyadb's "must not feel slow" / footprint-conscious performance
stance from NEEDS.md, especially for a mobile/embedded target where syncing
a full accessible dataset may be wasteful. The client/server dual-execution
of the *same* declared query (with the server allowed a stricter
implementation) is also a clean answer to "how do you keep permissions
consistent between local query results and the source of truth" — worth
comparing against RxDB/WatermelonDB's simpler "server just filters what it
returns" approach.
