# hoyadb

> Offline-first embedded database with local persistence, reactive queries, and sync — in the spirit of Realm, RxDB, and WatermelonDB.

Public contribution instructions live here and in [CONTRIBUTING.md](CONTRIBUTING.md).
The build and contribution checks require no private repositories. Maintainers
may use additional private process tooling; it is not a contributor dependency.

## Required workflow

1. Read [docs/README.md](docs/README.md), the relevant design contracts and nearby
   code/tests before editing. Reuse established patterns and dependencies.
2. Inspect Git status and preserve existing work. Describe the change and its
   acceptance cases before implementation; add a meaningful failing regression
   test before fixing behavior. Keep structural changes behavior-preserving.
3. Update affected guides and source links in the same change. Allocate formal
   artifact IDs in `docs/design/spec/registry.yaml`; preserve accepted ADR bodies,
   append-only decision history and recorded historical evidence.
4. Run the relevant build, test, formatting, lint and traceability checks from
   [CONTRIBUTING.md](CONTRIBUTING.md). Do not disable or weaken checks to pass.
5. Review the complete diff. Report actual verification, failures and remaining
   limits. Commit, push, publish or rewrite history only when explicitly asked.

## Goal

Build a local-first database: reads/writes always go to an on-device store
first and work fully offline, with a separable sync layer to reconcile with a
backend when connectivity is available. See [README.md](README.md) for the
comparison to Realm, RxDB, and WatermelonDB that frames this goal. The formal artifacts record the chosen direction; the
architecture's concrete
binding mechanisms and other explicit open questions still require validation.

## Layout

- **[crates/](crates/)** — initial Rust core/storage contract, in-memory adapter and
  native/browser SQLite adapters and the public Rust data model and Phase 3 reactive owner.
  These packages are unpublished; see `docs/implementation/phase-three-reactivity.md`.
- **[Cargo.toml](Cargo.toml)** — workspace; spike crates are explicitly excluded.
- **[packages/](packages/README.md)** — npm client, testing and React Native SDKs.
- **[bindings/](bindings/)** — Swift and Android SDKs.
- **[examples/](examples/README.md)** — runnable public tutorials used by the docs.
- **[tools/traceability/](tools/traceability/README.md)** — standalone public
  artifact checks and RTM generation; scan only `docs/design`.
- **[Repository layout](docs/guides/repository-layout.md)** — current boundaries,
  commands and mapping from historical source paths.
- **[docs/](docs/README.md)** — documentation entry point.
- **[docs/design/](docs/design/README.md)** — formal source of truth: vision,
  objectives, decisions, requirements/NFRs, risks, security threats, architecture,
  data models, specifications, contracts, UX, test plans/cases, and generated RTM.
  [docs/design/spec/registry.yaml](docs/design/spec/registry.yaml) allocates stable
  IDs; its file paths are relative to `docs/design/` (ADR-0029).
- **[docs/guides/](docs/guides/overview.md)** — glossary, worked examples,
  platform bindings, testing, relations and sync integration. Examples illustrate
  the formal design; guide TypeScript syntax remains illustrative. The implemented
  Rust surfaces are API-0004/API-0005 with runnable memory examples.
- **[docs/references/](docs/references/README.md)** — prior-art observations,
  comparison, original fetched metadata, hidden import trackers and historical
  decision alternatives. These are research, not accepted product requirements.
- **[Documentation history](docs/references/documentation-history.md)** — source
  coverage and pinned historical versions. Old source paths inside accepted
  ADRs refer to that snapshot, not live files. Preserve accepted ADR bodies.
- **[DECISIONS.md](DECISIONS.md)** — append-only root decision log.
- **[ROADMAP.md](ROADMAP.md)** — implementation sequence starting with the
  Rust/WASM/native-binding validation spike (ADR-0008).

An isolated Rust spike with a Swift desktop bridge exists under
`spikes/cross-platform/`; no
complete production database implementation exists yet. Current accepted design choices
include Rust/WASM/native bindings, a pluggable storage interface with a
SQLite-family default and command-first authoritative-backend sync; unresolved
mechanisms remain explicit in the architecture, specifications and risks.

## Working rules

- Preserve public APIs, error behavior, data formats and protection guarantees.
  New runtime dependencies or breaking changes require an explicit decision.
- Never publish credentials, keys or private data. Use task-owned fixtures for
  destructive storage/security checks, never an application's existing store.
- Prefer focused changes. Do not add speculative platform packages or duplicate
  database behavior in language wrappers. Core owns shared semantics.
- Keep generated artifacts separate from source. A moved binary or old passing
  report is not evidence for changed source. Do not claim unsupported platforms.
- Don't state something as fact unless it's been checked in this session —
  against the actual file, command output, or another primary source. If it
  hasn't been checked, say so instead of guessing.
- Back claims with what was checked: the file/line, command, or output that
  supports them, rather than restating from memory or general knowledge.
- Don't invent file paths, commands, APIs, config keys, or values that
  haven't been confirmed to exist in this project.
- If something can't be found or verified, say "not found" / "unknown"
  rather than offering a plausible-sounding guess.
- Don't ask clarifying questions when the request is already clear enough to
  act on — proceed, and note any assumption made instead of stopping to
  confirm it. Only ask when genuinely blocked (missing input, a decision
  only the user can make, or an irreversible/out-of-scope action).

## Commands

| Task | Command |
| --- | --- |
| Install | Rustup selects the pinned toolchain/components from `rust-toolchain.toml` |
| Run | `cargo run -p hoyadb-storage-memory --example reactive --offline --locked` |
| Dependency fetch | `cargo fetch --locked` |
| Workspace tests | `cargo test --workspace --offline --locked` |
| Format | `cargo fmt --all --check` |
| Lint | `cargo clippy --workspace --all-targets --offline --locked -- -D warnings` |
| WASM library build | `cargo build -p hoyadb-core -p hoyadb-storage-memory --target wasm32-unknown-unknown --offline --locked` |
| Host spike tests | `cargo test --manifest-path spikes/cross-platform/Cargo.toml --offline --locked` |
| Native spike | See `spikes/cross-platform/native/README.md` for Rust and Swift commands |
| JavaScript tests | `npm ci --offline --ignore-scripts --no-audit --no-fund` then `npm test` |
| React Native host tests | `npm run test:react-native` |
| Public tooling tests | `python3 -m unittest discover -s tools/traceability -p 'test_*.py'` and `python3 -m unittest discover -s tests/packaging -p 'test_*.py'` |
| Traceability | `python3 tools/traceability/trace_check.py docs/design` |
| RTM check | `python3 tools/traceability/rtm_gen.py docs/design --check` |
| RTM regenerate | `python3 tools/traceability/rtm_gen.py docs/design` |
