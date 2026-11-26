---
source: https://docs.powersync.com/ (multiple pages, see per-section headers)
source_type: doc-site
fetched: 2026-09-13
---

# PowerSync — Architecture (observed)

Observed, not accepted — this describes PowerSync's structure, not
hoyadb's.

## Sync scoping: Sync Rules and buckets

Source: sync/rules/organize-data-into-buckets (fetched 2026-09-13)

- Sync Rules are declared server-side (YAML), independent of client code —
  a different posture from RxDB/WatermelonDB (pull/push handlers the
  backend author writes ad hoc) and from Zero (queries defined once in
  shared client/server code).
- A **bucket definition** has two parts: a **parameter query** (e.g.
  `SELECT request.user_id() as user_id`, typically deriving values like
  the authenticated user id from the JWT) and one or more **data queries**
  that select rows using those parameters (e.g.
  `SELECT * FROM lists WHERE owner_id = bucket.user_id`).
- Bucket types observed: **global** (same data to every client, no
  filtering), **user-filtered** (per-user, via `user_id`), and
  **custom-filtered** (arbitrary parameters — project, org, etc.).
- **A client syncs exactly the buckets its parameter queries resolve to**
  for that client's credentials — computed server-side, and declared
  once, reused by every matching client (vs. Zero's model of syncing the
  union of that client's currently *active* queries — buckets are
  membership-based and largely static per session, not tied to which UI
  queries happen to be mounted right now).
- **Named scale limit**: total bucket count across all of one user/client's
  parameter-query results is capped (default 1,000) — stated explicitly as
  a performance guard, i.e. this is a real, load-bearing constraint on how
  finely buckets can be sliced, not a free abstraction.

**Relevance to hoyadb**: this is a third distinct answer (after
WatermelonDB/RxDB's "sync your whole accessible dataset" and Zero's "sync
the union of active queries") to "how much data does a client actually
sync": PowerSync's is **declarative, server-computed, membership-based
partitioning** — closer to a pub/sub topic model than either of the other
two. Worth weighing directly against Zero's per-query model for hoyadb's
own sync-scoping design: buckets are cheaper to keep in sync with "what
should this user always have" (doesn't depend on which screens are open)
but coarser-grained than Zero's exact per-query result set.

## Reactivity: three tiers of watch query

Source: client-sdks/watch-queries (fetched 2026-09-13)

PowerSync names and exposes three explicit tiers of the same underlying
idea — "turn a SQL query into a subscription" — trading implementation
cost for update precision:

1. **Basic watch queries** (all SDKs): the query re-runs and emits a fresh
   result set on *any* change to a dependent table, whether or not the
   query's actual results changed — simplest, but can trigger unnecessary
   re-renders.
2. **Incremental watch queries** (JS SDKs only): still re-run the query on
   every dependent-table change, but diff the new result set against the
   previous one with a configurable comparator and only emit when they
   actually differ; React's `useQuery` additionally supports a
   `rowComparator` to preserve object identity for unchanged rows
   (row-level memoization).
3. **Differential watch queries**: go further and compute an explicit
   added/removed/updated delta, delivered via an `onDiff` callback — for
   UI code that wants to patch a list incrementally rather than
   re-rendering from a fresh array.

**Relevance to hoyadb**: this is now the **third** independent source
(after RxDB's EventReduce and Realm's insertions/deletions/modifications
listener) offering some form of incremental-diff-instead-of-full-recompute
as an explicit, named performance tier for live queries — strong
convergent evidence this is a real, recurring need, not a nice-to-have.
PowerSync's tiered approach (opt into more precision only as needed) is a
good API-shape precedent: hoyadb could offer "just re-emit," "re-emit only
on real change," and "give me the diff" as three distinct subscription
modes rather than one fixed behavior.
