# Part 1 — Platform & language

Open question from [NEEDS.md](https://gitlab.com/alocas-ia/hoyadb/-/blob/1647b178d4fb3ee925d574367d2f9d526a4bb116/legacy/NEEDS.md): "Target platform(s)/language(s)
for the core implementation" — explicitly undecided.

## Option A — TypeScript/JS core, portable across JS runtimes

One implementation language, reused across browser, React Native/Capacitor
(mobile), and Electron/Tauri (desktop) shells via per-runtime storage
adapters (see [part 3](../03-storage-engine/)).

- **Precedent**: RxDB (browser/RN/Capacitor/Node/Electron/Tauri/Deno from
  one core — [rxdb/overview.md](../../rxdb/overview.md));
  WatermelonDB (iOS/Android/Windows/web/Node from one framework-agnostic
  JS API — [watermelondb/overview.md](../../watermelondb/overview.md));
  PouchDB (one API, storage swapped per environment —
  [pouchdb/overview.md](../../pouchdb/overview.md)); Zero and
  PowerSync's client SDKs are also JS/TS-first.
- **Pros**: fastest path to covering NEEDS.md's "at least mobile and web"
  scope from one codebase; largest precedent set (7 of 8 researched
  systems); reactive-query and JSON-based sync ecosystems are mature here.
- **Cons**: JS-thread overhead / GC pauses are a real ceiling for very
  large local datasets — WatermelonDB's entire "lazy loading + off-JS-thread
  SQLite" design exists specifically to work around this; a JS core
  inherits that ceiling rather than avoiding it.

## Option B — Rust core, compiled to WASM (web) and native bindings (mobile/desktop) (chosen 2026-09-13)

A single Rust implementation, exposed to each platform via
WASM (browser) and FFI bindings (Swift/Kotlin/Node).

- **Precedent**: **none in this reference set** — no researched source
  (RxDB, WatermelonDB, Realm, Zero, PowerSync, ElectricSQL, cr-sqlite,
  PouchDB) uses this shape for its client core. This is general industry
  knowledge (e.g. the pattern used by some newer embedded-DB projects),
  **not verified against hoyadb's own reference set** — flagged per
  CLAUDE.md's rule against stating unchecked things as fact. cr-sqlite is
  the closest thing to a Rust-adjacent precedent in the set: it's a SQLite
  loadable extension implemented in Rust, though it's a storage-layer
  extension, not a full client core like what this option proposes
  ([cr-sqlite/overview.md](../../cr-sqlite/overview.md)).
- **Pros**: one implementation, no per-platform JS-thread ceiling; likely
  better raw performance headroom.
- **Cons**: materially higher upfront engineering cost (FFI/WASM bindings
  per platform, toolchain complexity); NEEDS.md's performance bar is
  explicitly "fast enough for real apps," not "fastest local database," so
  this cost isn't clearly justified on performance grounds alone; no
  reference project de-risks the overall shape.
- **Why picked despite the gap above**: every option in this whole
  `refined-vision/` folder is an authored judgment call, not something the
  reference research derives on its own — `comparison.md` says this
  directly ("it does not decide anything for hoyadb"). Options A and C in
  [part 5](../05-sync-mechanism/) and [part 6](../06-sync-scoping-topology/)
  are picked the same way, on stated reasoning rather than precedent
  density alone. This one's reasoning is a direct preference for Rust's
  performance ceiling over the JS option's; the "no precedent" line above
  is recorded as evidence-basis bookkeeping, not as a mark against the
  pick's legitimacy — it matters later because an ADR needs to name what
  is and isn't precedented, not because unprecedented options need extra
  justification to be chosen.
- **Ecosystem check, done this session** (see also
  [03-storage-engine/alternatives.md](../03-storage-engine/alternatives.md)
  for how this affects the storage-layer pick specifically): the Rust
  embedded-storage crates that would realize this option are not uniformly
  healthy — `sled` is now in maintenance mode and explicitly not
  recommended for new projects, while `redb` (its suggested successor) and
  `rusqlite` (SQLite bindings) both show active 2026 commit activity. This
  doesn't change the pick, but it rules out one naive default (`sled`)
  before it gets baked into an ADR. Sources: [sled docs.rs](https://docs.rs/sled),
  [Rust FAQ — embedded databases](https://www.rustfaq.org/en/how-to-use-embedded-databases-sled-redb-in-rust/),
  [redb crates.io](https://crates.io/crates/redb),
  [rusqlite GitHub](https://github.com/rusqlite/rusqlite).

## Option C — Separate native reimplementation per platform (Swift, Kotlin, JS, ...)

Each platform gets its own from-scratch implementation of the core.

- **Precedent**: **actively avoided** by every researched source —
  WatermelonDB explicitly ships one "framework-agnostic JS API" specifically
  to avoid this; no source reimplements its core logic per platform.
- **Cons**: N times the engineering/maintenance cost, N chances for
  behavioral drift between platforms (especially dangerous for a sync
  protocol, where client/server must agree on semantics — see
  [part 5](../05-sync-mechanism/)'s point about mutator determinism).
- Not seriously considered; included for completeness since it's a
  theoretical fourth option.
