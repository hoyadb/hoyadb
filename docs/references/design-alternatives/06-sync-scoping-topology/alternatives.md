# Part 6 — Sync scoping & topology (how much data, and peer shape)

Not directly named in [NEEDS.md](https://gitlab.com/alocas-ia/hoyadb/-/blob/1647b178d4fb3ee925d574367d2f9d526a4bb116/legacy/NEEDS.md) beyond "sync online over
api **or other communication layer**" (leaving topology open) — this is
[references/comparison.md](../../comparison.md)'s own flagged
gap: "a genuinely open, first-order question... deserves its own explicit
ADR, independent of the command-vs-state question," and separately, sync
topology "worth an explicit decision rather than defaulting to
client-server." Two sub-questions are bundled here (how much data a client
holds, and whether there's a designated server) because both are about the
shape of the sync relationship, not the sync unit ([part 5](../05-sync-mechanism/)).

## Option A — Client-server, declarative membership-based scoping (chosen in VISION.md)

One authoritative backend; what a client syncs is computed server-side from
declared membership rules (e.g. "this user's org"), not from which queries
happen to be mounted in the UI right now.

- **Precedent**: PowerSync's Sync Rules/buckets — server-side YAML
  declaring a parameter query (e.g. deriving `user_id` from the JWT) plus
  data queries using those parameters; "a client syncs exactly the buckets
  its parameter queries resolve to... computed server-side, reused by every
  matching client" — contrasted explicitly with Zero's per-active-query
  model. Named scale limit: bucket count per client capped (default 1,000)
  as an explicit performance guard
  ([powersync/architecture.md](../../powersync/architecture.md)).
- **Pros**: predictable, stable data footprint per client (doesn't shrink
  or grow depending on which screens are open); declared once, reused by
  every matching client rather than recomputed per session — a good fit
  for NEEDS.md's "must not feel slow" concern about large local datasets
  (echoing WatermelonDB's own lazy-loading motivation).
- **Cons**: coarser than Zero's exact per-query result set — a client may
  hold data no currently-open query needs; the named 1,000-bucket cap is a
  real, load-bearing scaling constraint to design around, not a free
  abstraction.

## Option B — Client-server, sync the whole accessible dataset

Every client holds everything it's permitted to see; no finer-grained
scoping.

- **Precedent**: RxDB and WatermelonDB both — "Whole accessible dataset
  (permission-filtered)" per [comparison.md](../../comparison.md)'s
  sync-architecture table.
- **Pros**: simplest possible scoping — no bucket/query-membership
  machinery to design at all.
- **Cons**: doesn't scale to large permission-filtered datasets (the exact
  problem WatermelonDB's lazy-loading/off-JS-thread design otherwise works
  hard to avoid at the storage layer — scoping everything back out at the
  sync layer undercuts that effort).

## Option C — Client-server, scoped to currently-active queries with TTL

A client syncs the union of whatever queries its UI currently has mounted;
data for a deactivated query sticks around briefly (TTL) then drops.

- **Precedent**: Zero — "Union of currently active queries, with TTL after
  deactivation" ([comparison.md](../../comparison.md) sync
  architecture table); reads inside a mutator only see already-synced data
  client-side, everything server-side
  ([zero/sync.md](../../zero/sync.md)).
- **Pros**: tightest possible footprint — a client never holds data no
  query currently needs (modulo the TTL grace period); pairs naturally
  with Option A of [part 5](../05-sync-mechanism/) since Zero's query and
  mutator models share the same client/server-dual-implementation pattern.
- **Cons**: footprint is a function of UI state, not identity/permissions —
  less predictable capacity planning; a query that's briefly unmounted
  (e.g. tab switch) either re-syncs from scratch or relies on the TTL
  window, a tuning knob PowerSync's model doesn't need.

## Option D — Peer-to-peer / multi-master, no designated server

Any node (including another client) can be written to and read from
directly; no single authoritative server role.

- **Precedent**: PouchDB/CouchDB — "any node can be written to or read
  from," framed explicitly via CAP theorem as prioritizing availability
  over strong consistency
  ([pouchdb/sync.md](../../pouchdb/sync.md)); cr-sqlite's whole-CRR
  symmetric convergence model ("all of the CRRs on different databases
  identical after they merge") is architecturally the same peer shape,
  achieved through row/column-level CRDTs instead of a replication
  protocol ([cr-sqlite/sync.md](../../cr-sqlite/sync.md)).
- **Pros**: the only option that's genuinely production-proven at scale
  across multiple independent vendors (CouchDB, Cloudant, Couchbase, all
  speaking the same protocol) — real evidence this is viable, not
  theoretical; most directly honors NEEDS.md's "or other communication
  layer" phrasing, which doesn't actually commit to a client-server shape.
- **Cons**: the biggest architectural commitment of the four — no source
  in this set treats it as an incremental add-on to a client-server design;
  requires solving conflict resolution symmetrically (see
  [part 7](../07-conflict-resolution/)) since there's no authoritative side
  to defer to. Not chosen for v1 given no current signal (multi-device
  editing without any backend at all) that hoyadb needs it, but flagged as
  the option most worth revisiting if that signal ever appears — not
  rejected on technical grounds, only on "no evidence it's needed yet."
