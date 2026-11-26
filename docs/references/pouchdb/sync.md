---
source: https://pouchdb.com/guides/replication.html (and guides/changes.html, guides/conflicts.html — see per-section headers)
source_type: doc-site
fetched: 2026-09-13
---

# PouchDB — Replication and conflict resolution (observed)

Observed, not accepted — this describes PouchDB's sync mechanism, not
hoyadb's. This is the CouchDB replication protocol, the oldest and most
widely-implemented protocol in this whole reference set (PouchDB, CouchDB,
Cloudant, Couchbase, and others all speak it).

## One-way replication vs. two-way sync

Source: guides/replication (fetched 2026-09-13)

- `localDB.replicate.to(remoteDB)` — one-directional; target gets
  source's changes, nothing flows back.
- `localDB.sync(remoteDB)` — a convenience wrapper that just runs
  replication in both directions; not a structurally different mechanism
  from two one-way replications.
- **`{live: true}`** turns a one-shot replication into continuous,
  real-time propagation; **`{retry: true}`** adds automatic reconnection
  for intermittent connectivity (mobile scenario named explicitly), with
  `paused`/`active` events marking connection loss/recovery.
- **Any PouchDB instance can replicate with any other** — the docs
  explicitly note this composability supports topologies like an
  in-memory cache syncing with a local on-disk DB, multiple remote sources
  feeding one local store, or a local DB backing up to a remote — i.e.
  replication is a general primitive, not a fixed client-server shape.

## Multi-master architecture

- **No designated primary/authoritative node**: "any node can be written
  to or read from" — a genuinely different topology from every other
  source in this reference set except cr-sqlite's whole-database
  convergence model. RxDB/WatermelonDB/Zero/PowerSync/Electric all assume
  one authoritative server; PouchDB/CouchDB assume peers of equal standing.
- Explicitly framed via CAP theorem: prioritizes availability and
  partition tolerance over strong consistency — "eventually consistent."
  A widely quoted framing cited in the docs: "CouchDB is bad at
  everything, except syncing" — i.e. its whole design center of gravity is
  the replication protocol, everything else is secondary.

## Conflict resolution: revision trees

Source: guides/conflicts (fetched 2026-09-13)

- Revisions form a **tree** ("similar to a Git history tree"), not a
  linear history — because multi-master means two peers can each extend
  the *same* revision independently, producing branches. On replication,
  CouchDB/PouchDB **deterministically pick a "winning" revision** via a
  fixed algorithm — "both users will see the same winner once they're back
  online" — so every peer converges on the same choice with no
  coordination needed.
- **The losing revision is not discarded.** A document with unresolved
  conflicts carries a `_conflicts` attribute (visible via
  `db.get(id, {conflicts: true})`) listing the losing revision ids, each
  still individually fetchable (`db.get(id, {rev: '2-y'})`). A plain
  `get()` (no options) transparently returns only the deterministic
  winner — conflicts are invisible unless the app specifically asks.
- **Three named resolution patterns**, all app-driven (PouchDB itself
  doesn't auto-merge): (1) manually inspect both revisions and
  `remove()` the unwanted one; (2) the `pouchdb-upsert` plugin, which
  retries a write as a *delta function* (a function computing the new
  state from whatever the current state is) rather than a fixed
  before/after pair, sidestepping `409`s entirely instead of resolving
  them after the fact; (3) a **"ledger" design** — never update documents,
  only ever create new ones representing events — under the explicit
  philosophy "accountants don't use erasers," making conflicting *updates*
  structurally impossible because there are none, only new facts.

This is a real, distinct third conflict-handling philosophy alongside
"auto-merge" (WatermelonDB/PowerSync/cr-sqlite) and "auto-replace, no
trace of the loser" (RxDB default, Zero): **auto-pick-a-winner-but-keep-
the-loser-visible-for-the-app-to-reconcile**.

## Reactivity: the changes feed

Source: guides/changes (fetched 2026-09-13)

- `db.changes({since: 0, include_docs: true})` — one-shot, returns every
  historical change since a given sequence position, in the order they
  were made.
- `db.changes({since: 'now', live: true, include_docs: true})` — live
  mode, emits a `'change'` event for each subsequent create/update/delete
  as it happens; explicitly recommended for "update the UI whenever
  something in the database changes" and named as the basis for
  "a real-time chat application"-style UIs.
- Each change event distinguishes add/modify from delete via a `deleted`
  boolean property — no separate delete-specific feed.
- **Large histories paginate** via `limit` + tracking the last-seen
  sequence (`last_seq`), rather than the app manually tracking timestamps.
- **Likely the same underlying primitive live replication uses**: neither
  fetched page explicitly states that `sync({live: true})` is implemented
  in terms of the changes feed, but the two features are documented with
  near-identical mechanics (`since`, `live`, per-change events) and the
  wider CouchDB replication protocol is well known to use a `_changes`
  endpoint for exactly this. Flagged here as a strong inference, not a
  confirmed claim from what was fetched this session — worth verifying
  directly against source/protocol docs before relying on it.

## Relevance to hoyadb

Two genuinely distinctive ideas not seen (in this combination) anywhere
else in this reference set:

1. **Multi-master with deterministic, content-independent winner
   selection, but the loser is preserved rather than discarded.** This
   is a real third conflict-handling philosophy alongside "auto-merge" and
   "auto-replace" — worth weighing directly, especially since it composes
   naturally with peer-to-peer topologies (no single node needs to be "the
   server"), which NEEDS.md doesn't rule out (it says "sync online over
   api **or other communication layer**").
2. **Replication as a generic, composable primitive** ("any PouchDB
   object can replicate with any other") rather than a fixed
   client-server contract — and the changes feed and live replication
   share strikingly similar mechanics (see the inference flagged above).
   Worth considering whether hoyadb's own reactivity (observe local
   changes) and sync (propagate changes to a peer) could share one
   underlying primitive, the way PouchDB's design at least strongly
   suggests, rather than building them as two separate systems from
   scratch.

The deterministic-winner-but-keep-loser approach also has no obvious
answer to the counter-CRDT problem documented elsewhere in this set
([../realm/sync-historical.md](../realm/sync-historical.md),
[../cr-sqlite/sync.md](../cr-sqlite/sync.md)) — picking one whole document
revision as winner has the same "loses concurrent partial updates" issue
as RxDB's default handler, just with the option for the app to notice and
fix it after the fact rather than automatically.
