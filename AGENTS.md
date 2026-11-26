# hoyadb

> Offline-first embedded database with local persistence, reactive queries, and sync.

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
backend when connectivity is available. The current architecture and technical contracts define the database and
platform boundaries. Verify platform support against fresh acceptance results.

## Layout

- **[crates/](crates/)** — Rust core, storage adapters, shared facade and bridges.
  Standalone native/web crates retain their own lockfiles and build requirements.
- **[packages/](packages/README.md)** — npm client, testing and React Native SDKs.
- **[bindings/](bindings/)** — Swift and Android SDKs.
- **[examples/](examples/README.md)** — runnable public tutorials.
- **[docs/](docs/README.md)** — documentation entry point.
- **[docs/design/](docs/design/README.md)** — current architecture, requirements,
  specifications, contracts, risks and test cases. Stable IDs are allocated in
  [the registry](docs/design/spec/registry.yaml).
- **[docs/guides/](docs/guides/overview.md)** — integration and contributor guides.
- **[DECISIONS.md](DECISIONS.md)** — current architecture decision index.
- **[tools/traceability/](tools/traceability/README.md)** — public artifact checks;
  scan only `docs/design`.
- **[tools/release/](tools/release/README.md)** — compatibility and release checks.
- **[Repository layout](docs/guides/repository-layout.md)** — package boundaries.
- **[GitHub delivery](docs/guides/github-delivery.md)** — CI, publishing and Pages.

Packages remain unpublished. Native platform support and publication require
their own current acceptance evidence; host tests do not establish those gates.

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
