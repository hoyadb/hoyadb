---
source: https://electric.ax/docs/sync/guides/shapes
source_type: doc-site
fetched: 2026-09-13
---

# ElectricSQL — Architecture: Shapes

Observed, not accepted — this describes Electric's structure, not
hoyadb's.

## What a Shape is

A **Shape** is the sync unit: "a subset of [table] data that you'd like to
sync into a local app," always rooted at a single table, with:

- **table** (required) — the root table, e.g. `projects` or `schema.table`.
- **where** (optional) — a Postgres-expression row filter, e.g.
  `status IN ('backlog', 'todo')` or a parameterized `org_id=$1`.
- **columns** (optional) — a column subset; defaults to all columns.
  Related tables can only be pulled in via subqueries inside the `where`
  clause, not as a native join — a Shape is always single-table at its
  root.
- **queryable columns** (optional) — restricts which columns a client is
  even allowed to filter on.

A shape is addressed directly over HTTP, e.g.
`/v1/shape?table=projects&where=org_id=$1&params[1]=org_123&columns=id,title` —
the shape definition *is* the URL/query, not a separately named/registered
object (contrast with PowerSync's YAML-declared bucket definitions, or
Zero's named, code-registered queries).

## Subscription and update mechanics

- Client library (`ShapeStream` + `Shape`) fetches an **initial snapshot**
  of matching rows, then transitions into a live-update mode, receiving
  subsequent changes as **"Shape Log" entries** appended over time.
- Server-side, Postgres changes are evaluated against each active shape's
  `where` clause; matching changes are streamed to every client subscribed
  to that shape.
- The docs describe this as "live-polling mode" for the post-snapshot
  phase — the exact transport (HTTP long-polling vs. streaming) wasn't
  fully detailed beyond that phrase on this page.

## HTTP protocol mechanics

Source: docs/sync/api/http (fetched 2026-09-13)

- Single endpoint, `GET /v1/shape`. Initial sync: `?table=foo&offset=-1`
  ("give me the whole log from the start"); server responds with **Shape
  Log entries** (`operation`: insert/update/delete, `key`, `value` as
  JSON) and paginates large results via an `electric-offset` response
  header.
- **Three named control messages** interleaved with data: `up-to-date`
  (client is caught up, may switch to live mode), `must-refetch` (client
  must discard everything and resync from scratch — an explicit
  "we can't give you an incremental path, start over" signal), and
  `snapshot-end` (marks the end of an initial/subset snapshot, carrying
  Postgres transaction-visibility metadata: `xmin`/`xmax`/`xip_list`).
- **Live mode has two transport options**, both opt-in via query params on
  the same endpoint: **long-polling** (`live=true` + last offset + a shape
  `handle`; server holds the connection until timeout or new data) and
  **SSE** (`live=true&live_sse=true`; persistent server-push connection,
  with a documented ~21s keep-alive and a noted operational requirement —
  reverse proxies must have streaming/buffering disabled, e.g. nginx's
  `proxy_buffering off;`, or SSE silently breaks).
- `offset=now` explicitly skips all history and jumps straight to
  live-only — for a client that doesn't need backfill.
- **Two log modes**: full (snapshot then live changes — the default) vs.
  `log=changes_only` (skip the snapshot, deliver only subsequent changes)
  — a client-selectable trade-off between "get everything" and "get only
  what changes from now."
- **Subset snapshots**: a `POST` variant accepts a JSON body
  (`where`/`params`/`order_by`/`limit`) for one-off filtered fetches
  outside the standing-subscription model, still returning Postgres
  snapshot metadata so the client can reconcile what transactions are
  already covered.
- **Caching is a first-class design goal, not an afterthought**: responses
  carry standard HTTP caching headers (`cache-control`, `max-age`,
  `stale-age`, `etag`) specifically so that CDNs can cache initial-sync
  responses at the edge; browser HTTP cache alone makes repeat data access
  "instant and available offline" across page loads; and ordinary HTTP
  **request collapsing** (a CDN feature, not Electric-specific) lets many
  concurrent clients' long-polling requests coalesce into one upstream
  request, explicitly cited as how Electric scales to large numbers of
  concurrent clients without proportional origin-server load.

**Relevance to hoyadb**: building the sync protocol entirely on standard
HTTP semantics (cacheable GETs, standard headers, request collapsing) so
that ordinary HTTP infrastructure (CDNs, browser cache) does most of the
scaling work is a distinctive and clever choice — worth serious
consideration for hoyadb's own transport-agnostic-but-HTTP-friendly design,
since it gets a lot of scaling/offline-caching behavior "for free" from
existing infrastructure rather than needing bespoke server-side
optimization. The `must-refetch` control message is also a useful, simple
concept: an explicit "give up on incremental sync, full resync required"
escape hatch, distinct from WatermelonDB's more elaborate Replacement Sync
but solving a similar problem.

## Relevance to hoyadb

Shapes are structurally closest to **PowerSync's buckets** among all
sources studied — both are server-evaluated row filters a client
subscribes to, syncing a *subset* rather than a client's whole accessible
dataset or exactly its currently-active query results (Zero). The key
difference: a Shape is single-table-rooted and addressed directly as a
query (URL-as-definition, no separate declaration step), while a
PowerSync bucket is a named, separately-declared server config object that
can join across the parameter/data-query split. Worth comparing directly
against both when designing hoyadb's own sync-scoping unit — "one
filtered table stream, addressed by its filter" is the simplest of the
three models seen so far, at the cost of pushing any cross-table
composition onto the client (fetch multiple Shapes, join client-side) or
into a `where`-clause subquery.
