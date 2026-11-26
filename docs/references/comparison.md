---
sources: rxdb, watermelondb, realm, zero, powersync, electricsql, cr-sqlite, pouchdb
---

# Reference comparison: 8 local-first / sync systems

Observed, not accepted. Framed as candidate "Considered options" input for
future hoyadb ADRs (ARCH-2 requires ≥2 considered options) — this file does
the comparison, it does not decide anything for hoyadb. See each source's
own `overview.md`/`architecture.md`/`requirements.md`/`sync.md` under
`references/<source>/` for the full detail and citations behind each row.

Sources, in the order they were imported: **RxDB**, **WatermelonDB**,
**Realm** (local model current; Device Sync deprecated/historical),
**Zero** (Rocicorp — command/mutation-based, successor to the now-archived
Replicache), **PowerSync**, **ElectricSQL**, **cr-sqlite** (vlcn.io —
CRDT extension for SQLite), **PouchDB** (the oldest here — CouchDB
replication protocol; historically RxDB's original storage adapter).

## Data model / storage

| | RxDB | WatermelonDB | Realm | Zero | PowerSync | ElectricSQL | cr-sqlite | PouchDB |
|---|---|---|---|---|---|---|---|---|
| Model | Document, relations bolted on | Relational (SQLite); belongs-to native, has-many/many-to-many via query convention | Relational/object; to-one, to-many, inverse all native; embedded objects first-class | Relational, schema declared *against* an existing Postgres DB; native to-one/to-many/many-to-many (2-level chaining cap) | Relational (existing Postgres/MySQL/MongoDB/SQL Server schema, unmodified) | Relational (existing Postgres schema, unmodified); Shape is single-table-rooted, no native cross-table join | Relational — ordinary SQLite tables, opt-in per-table CRDT-ness | **Fully schemaless** — "unstructured documents," no schema concept at all |
| Storage engine | Pluggable (`RxStorage`) | SQLite/LokiJS via `DatabaseAdapter` | Fixed embedded engine, not pluggable | Client: fixed local store; server: your own Postgres | Fixed: SQLite client-side, existing server DB | Fixed: SQLite/PGlite client-side, Postgres server-side | SQLite itself, extended via loadable extension | Pluggable by environment (IndexedDB/WebSQL browser, LevelDB Node) behind one API |
| Owns its own schema? | Yes | Yes | Yes | No — schema maps onto existing Postgres | No — syncs an existing DB's data | No — Shape reads an existing DB's data | Yes (it *is* your SQLite schema, marked up) | N/A — no schema at all |

**Where PouchDB anchors the spectrum**: it's the least relational source in
the entire set — more document-oriented even than RxDB, with zero schema
concept. It's also the earliest "one API, pluggable local storage per
environment" design found (predating RxDB's near-identical `RxStorage`
idea, which historically built on PouchDB directly) — good evidence that
pattern is genuinely durable, not a recent trend.

## Reactivity / subscriptions

| | RxDB | WatermelonDB | Realm | Zero | PowerSync | ElectricSQL | cr-sqlite | PouchDB |
|---|---|---|---|---|---|---|---|---|
| Default read | `.exec()` / `.$` | `.fetch()` / `.observe()` | Always-live references; listeners are opt-in notification only | `zero.run()` / `useQuery()` | one-shot query / `watch()` | Shape subscription itself is the primitive (no separate one-shot vs. live split found) | plain SQL query / `rx-tbl` subscription | `db.changes({since})` one-shot / `{live: true}` continuous |
| Granularity | Field, document, query, collection | Query (set-membership default; per-field opt-in) | Realm-wide, query-diff, single-object-diff | Query-result (via `useQuery`), with an explicit "complete vs. partial" status | 3 tiers: full re-emit, diff-gated re-emit, explicit add/remove/update delta | Shape-level only (whole filtered table stream) | Table, single-row (via SQLite `rowid`), whole-DB — **no query-result-level reactivity yet** (roadmap) | Whole-database change stream only — no query/collection-scoped observation found |
| Performance guard | EventReduce (incremental) | `observeCount()` throttled 250ms | Not detailed | Per-query TTL + local-only queries (no server round trip) | Named tiers let the app opt into more precision only as needed | HTTP caching (CDN/browser) + request collapsing | None found (reactivity itself is still coarse) | Pagination via `last_seq` for catch-up; no live-mode throttling found |

**Where PouchDB fits**: alongside cr-sqlite and Electric, it's a third
source with **no query-result-level reactivity** — its changes feed is
whole-database, and the app is expected to filter/re-query itself in a
change handler. Three of eight sources lacking this (all three being the
ones with the least "owned" schema/query layer — cr-sqlite extends raw
SQL, Electric syncs raw Postgres rows, PouchDB is schemaless) is a pattern
worth noting: query-result-level reactivity seems to correlate with a
system that owns its own query/schema layer, not just its storage.

## Sync architecture (what/how much gets synced)

| | RxDB | WatermelonDB | Realm (hist.) | Zero | PowerSync | ElectricSQL | cr-sqlite | PouchDB |
|---|---|---|---|---|---|---|---|---|
| What syncs | Whole accessible dataset (permission-filtered) | Whole accessible dataset | Whole accessible dataset | Union of currently *active* queries, with TTL after deactivation | Declarative server-computed "buckets" (membership-based, ~session-static) | Single-table "Shapes" (URL-addressed row filter) | Whole-DB convergence (symmetric merge), or partial (not explored) | Whole database (filtered replication exists per CouchDB protocol generally, not explored in this pass) |
| Transport | Any JSON transport (pull/push) | Any transport (`pullChanges`/`pushChanges`) | Proprietary service | HTTP push endpoint + `zero-cache` (Postgres logical replication) | Dedicated centralized "PowerSync Service" | Plain HTTP (long-poll or SSE), CDN-cacheable | App-chosen (REST/WebSocket/WebRTC all mentioned, none mandated) | HTTP, CouchDB replication protocol specifically (widely implemented: CouchDB, Cloudant, Couchbase) |
| Requires a dedicated sync service? | No | No | Yes (was Atlas) | Yes (`zero-cache`) | Yes | No (just HTTP + Postgres) | No | No (any CouchDB-protocol endpoint) |
| Topology | Client↔server | Client↔server | Client↔server | Client↔server | Client↔server | Client↔server (read only) | Peer↔peer (symmetric) | **Multi-master — any peer↔any peer** |

**Where PouchDB is unique**: it's the second source (after cr-sqlite) with
no designated "server," and the *only* one with a widely-implemented,
multi-vendor standard protocol behind it (the same protocol runs across
PouchDB, CouchDB, Cloudant, Couchbase) — evidence that a
peer-to-peer/multi-master topology is a viable, production-proven choice,
not just a theoretical option. Directly relevant to NEEDS.md's "sync
online over api **or other communication layer**" phrasing, which doesn't
actually commit hoyadb to a client-server shape.

## Sync unit / conflict resolution — the core comparison

| | RxDB | WatermelonDB | Realm (hist.) | **Zero** | PowerSync | ElectricSQL | cr-sqlite | PouchDB |
|---|---|---|---|---|---|---|---|---|
| Sync unit | Entity state (doc) | Entity state (CRUD record) | Low-level ops (OT) | **Named mutation + args** | Entity state (CRUD, minimal payload) | N/A (read-only; writes are the app's own problem) | Entity state (per-row/column changeset) | Entity state (whole document per revision) |
| Conflict detection | Revision-token staleness | Server timestamp vs. `lastPulledAt` | Per-property/op (OT) | N/A — mutator is just re-run | Per-field timestamp (LWW) | N/A (app-defined, per its own write pattern) | Per-column CRDT type at merge time | Revision-tree branch (two peers extended the same parent rev) |
| Conflict resolution | Pluggable handler, default doc-level master-wins | Per-column, content-based merge | Fixed 4-rule OT, not pluggable | **Sidestepped**: server re-executes the mutator against current truth — authoritative result simply replaces optimistic local guess | Pluggable (fully backend-authored); default per-field LWW, deletes win | App's own responsibility entirely | **Fully automatic**, no manual hook; per-column CRDT (default LWW; Fractional Index for order; counters disputed in cr-sqlite's own docs) | **Deterministic auto-pick-a-winner, but loser is preserved** and app-retrievable/resolvable later |
| Context/intent alongside a write | None | None | None (OT ops are structural, not domain-level) | **`ctx`: a server-controlled, client-unmodifiable context object, explicit first-class concept** | **`metadata`/`trackMetadata`: arbitrary app JSON attached to a queued write, read by the backend handler** | N/A | None | None |
| Command/intent support | None | None | Partial (counter op only) | **Yes — this *is* the model** | No | No | No | None |

**This is still the most important row for hoyadb's NEEDS.md decision, and
the picture has changed substantially since the first 3-source pass.**
Zero is a real, actively-maintained (1.0 as of June 2026), production
precedent for **exactly** hoyadb's preferred approach: a named command with
arguments, executed optimistically client-side, re-executed authoritatively
server-side, with an explicit trusted-context mechanism separate from
client-supplied arguments. This substantially de-risks hoyadb's
command-based direction — it is no longer "unprecedented," it has one
solid working example (with the caveat flagged in `zero/sync.md` that
deterministic mutator behavior across client/server is a real requirement
Zero's docs don't fully spell out how to guarantee).

Separately, **PowerSync's `metadata` field** shows a lighter-weight middle
ground: keep sync fundamentally entity-diff-based, but let a write carry
free-form context the backend can act on — cheaper to build than a full
command/mutator system, though it doesn't get the "server can safely
re-derive the result from scratch" property Zero's approach has.

**PouchDB adds a fourth conflict philosophy**, distinct from all three
found in the first pass (auto-merge / auto-replace / fixed-OT):
**auto-pick-a-deterministic-winner but keep the loser around** for the app
to notice and reconcile on its own schedule, rather than either merging
automatically or discarding losing data. Its three named app-level
patterns (manual resolve, delta-function upsert, append-only ledger) are
also worth noting — the "ledger" pattern in particular (never update,
only ever create new event-like documents) is structurally an *event*
model, which edges toward hoyadb's command-based interest from a different
angle: not "sync the command" but "never have anything to conflict-resolve
because nothing is ever mutated in place."

**The counter-CRDT problem is now confirmed by three independent sources**:
Realm's historical OT needed a special increment operation; cr-sqlite's own
docs say counter CRDTs aren't supported (though its GitHub README
disagrees — an unresolved discrepancy, see `cr-sqlite/sync.md`). Three
separate real systems hitting the same wall on pure state/value-based
conflict resolution is strong, convergent evidence that this class of
problem is structurally hard without an operation/intent concept — the
single strongest piece of evidence in this whole reference set for why
hoyadb's command-based instinct has real merit, not just a stylistic
preference. (PouchDB's whole-document winner-pick would have the identical
gap for the same reason, though this wasn't specifically tested against a
counter example in its docs.)

**Conflict-resolution strategies observed, ranked by sophistication**:
1. Whole-document master-wins (RxDB default) — simplest, most data loss.
2. Whole-document deterministic-winner-but-loser-preserved (PouchDB) —
   same granularity as #1, but nothing is silently lost; resolution is
   deferred to the app instead of forced automatically.
3. Per-field/per-column LWW (PowerSync default, cr-sqlite default) —
   finer-grained, still purely mechanical, arrived at independently by two
   unrelated systems — good evidence this is close to a "default of least
   surprise" for entity-diff sync.
4. Per-column client-wins-on-top-of-server-state (WatermelonDB) — a step
   beyond plain LWW, still mechanical.
5. Fixed multi-rule OT (Realm, historical) / fully-automatic per-column
   CRDT dispatch (cr-sqlite) — handle more cases (lists, ordering)
   correctly by construction, at the cost of little-to-no
   customizability.
6. **Sidestepped via re-execution** (Zero) — arguably not "conflict
   resolution" at all in the traditional sense; correctness depends
   entirely on mutator determinism, a different kind of engineering
   burden than merge-rule design.

## Net implications for hoyadb's open questions (from NEEDS.md)

- **Command-based sync with context**: materially de-risked since the
  original 3-source pass. Zero is a real, current, production precedent;
  PowerSync's `metadata` field is a lighter partial precedent; PouchDB's
  "ledger" pattern offers a different event-oriented angle on the same
  underlying instinct (avoid mutation conflicts by never mutating). Still
  no source does *domain-level* commands like `add-product-to-basket`
  specifically (Zero's mutators are closer to "typed RPC with local
  prediction" than business-level intent), so there's real design work
  left, but "sync commands, not diffs" is now a validated direction, not
  a purely speculative one. Entity-diff sync remains a credible, even
  more thoroughly precedented fallback (6 of 8 sources).
- **Sync scoping ("how much data does a client hold")**: a genuinely open,
  first-order question with five different precedents found — deserves its
  own explicit ADR, independent of the command-vs-state question.
- **Sync topology (client-server vs. peer-to-peer)**: PouchDB/CouchDB and
  cr-sqlite both show multi-master/peer-to-peer as viable and (for
  PouchDB) production-proven at scale across multiple vendors — worth an
  explicit decision rather than defaulting to client-server, especially
  given NEEDS.md's "or other communication layer" phrasing.
- **Storage pluggability**: well-precedented (RxDB, WatermelonDB, and now
  PouchDB as the likely originator of the pattern); the "sync an existing
  DB, don't own the schema" pattern (Zero/PowerSync/Electric/cr-sqlite) is
  a whole different, equally-precedented option worth weighing against it
  directly.
- **Reactive subscriptions on entity/query**: well-precedented by 5 of 8
  sources at the query-result level; genuinely unsolved/coarse in the
  other 3 (cr-sqlite, Electric, PouchDB) — worth treating as real
  engineering effort to budget for, not an assumed-easy layer. Notably,
  all 3 without it are also the 3 with the least "owned" schema/query
  layer — a pattern, not a coincidence.
- **Relational-ish data model**: still a real spectrum, now anchored at
  both ends — PouchDB (fully schemaless) through WatermelonDB's
  minimal-native-relations and Realm's/Zero's rich-native-relations to
  cr-sqlite's "it's just SQLite." Same open decision for hoyadb's own ADR,
  now with a clearer sense of the full range.
- **Conflict resolution**: per-field/column LWW is the closest thing to an
  industry default among entity-diff systems (2 independent sources); the
  counter-CRDT gap (3 independent sources) is the clearest concrete proof
  point that pure state-diffing has real, structural limits worth solving
  via hoyadb's command-based approach. PouchDB's "preserve the loser"
  approach is worth weighing as a lower-risk middle step even if hoyadb
  doesn't go fully command-based: less data loss than silent auto-merge,
  much less engineering than OT/CRDT, at the cost of exposing conflicts to
  the app to handle.
