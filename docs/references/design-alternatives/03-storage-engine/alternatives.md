# Part 3 — Storage engine

[NEEDS.md](https://gitlab.com/alocas-ia/hoyadb/-/blob/1647b178d4fb3ee925d574367d2f9d526a4bb116/legacy/NEEDS.md) Open Questions: "expected to differ per
environment rather than being one engine everywhere, but leaning toward
SQLite or an equivalent embedded relational engine per platform. Not
finalized."

## Option A — Pluggable storage interface, SQLite-family default per platform (chosen in VISION.md)

One narrow storage contract; the concrete engine (SQLite, IndexedDB, OPFS,
in-memory, ...) is swapped per platform behind it, independent of the rest
of the stack.

- **Precedent**: RxDB's `RxStorage` interface — "RxDB is not a
  self-contained database... the underlying engine [can be swapped]
  depending on the JavaScript environment and performance requirements,"
  with Memory/IndexedDB/OPFS (browser), Filesystem/SQLite (Node/Electron/RN/
  Capacitor), Expo Filesystem, Dexie.js, and server-side engines (MongoDB,
  Deno KV, FoundationDB) all implementing the same interface
  ([rxdb/architecture.md](../../rxdb/architecture.md)).
  WatermelonDB's narrower `DatabaseAdapter` (SQLite vs. LokiJS on web) is
  the same idea at smaller scope
  ([watermelondb/architecture.md](../../watermelondb/architecture.md)).
  PouchDB is the earliest example of this pattern (one API,
  IndexedDB/WebSQL/LevelDB swapped per environment), and is believed to
  have directly inspired RxDB's design, which historically built on
  PouchDB as a storage adapter
  ([pouchdb/overview.md](../../pouchdb/overview.md)).
- **Pros**: directly matches NEEDS.md's own expectation ("expected to
  differ per environment"); three independent/sequential systems converged
  on this shape, including the oldest one in the set — evidence it's
  durable, not a fad; RxDB additionally shows storage *wrappers* (encryption,
  compression, worker-thread offload) can be composed on top of any engine
  choice without touching the engine itself.
- **Cons**: an abstraction layer to design and maintain; behavior can leak
  across engines (e.g. a query feature or transaction guarantee that one
  backend supports and another doesn't) if the interface isn't drawn
  carefully — not called out as a *solved* problem in any source read this
  session, worth treating as real design risk.

**Concrete engine choices, now that [part 1](../01-platform-language/) picked a
Rust core** (checked this session, 2026-09-13 — none of this was in the
original 8-source research, since none of those sources are Rust): the
engine behind this interface would most likely be `rusqlite` (SQLite
bindings for Rust) to stay aligned with NEEDS.md's own SQLite lean, or a
pure-Rust embedded KV store like `redb` underneath a hand-rolled relational
layer. **`sled`, a once-popular pure-Rust embedded database, should be
ruled out**: it's now explicitly described as being "in maintenance
mode" and unsuitable for new projects, with `redb` named as its
successor. `rusqlite` shows active 2026 commit history; `redb`'s
crates.io listing describes it as stable with a committed upgrade path
(though older docs pages still carry beta-era warnings — worth
re-verifying directly before depending on it). None of this changes which
*option* is picked here (A still stands), only which concrete crates
would implement it. Sources: [sled docs.rs](https://docs.rs/sled),
[Rust FAQ — embedded databases](https://www.rustfaq.org/en/how-to-use-embedded-databases-sled-redb-in-rust/),
[redb crates.io](https://crates.io/crates/redb),
[rusqlite GitHub](https://github.com/rusqlite/rusqlite).

## Option B — Fixed, non-pluggable embedded engine

One engine, the same everywhere, not swappable.

- **Precedent**: Realm — "Fixed embedded engine, not pluggable"
  ([comparison.md](../../comparison.md) storage row).
- **Pros**: simpler to build and reason about; no interface-leakage risk
  from Option A.
- **Cons**: directly contradicts NEEDS.md's stated expectation that the
  engine will differ per environment; ties hoyadb to whatever a single
  engine can/can't do on every platform (e.g. no OPFS-in-browser option if
  the fixed engine is SQLite-only).

## Option C — Sync an existing server database directly; local storage is a byproduct, not a designed layer

Local storage exists only to mirror rows pulled from an existing backend
DB; no separately-designed local storage contract.

- **Precedent**: PowerSync (fixed: SQLite client-side, but the point of
  the product is mirroring an existing server DB unmodified), ElectricSQL
  (SQLite/PGlite client-side, syncing an existing Postgres schema) — per
  [comparison.md](../../comparison.md) storage row, both answer
  "no" to "owns its own schema."
- **Cons**: same coupling concern as [part 2](../02-data-model/)'s Option D
  — commits hoyadb to a specific backend technology before that's decided.
  Rejected for the same reason.
