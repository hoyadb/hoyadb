# hoyadb

> Offline-first embedded database with local persistence, reactive queries, and sync — in the spirit of Realm, RxDB, and WatermelonDB.

## What this is

hoyadb is a local-first database: the client always reads and writes to an
on-device store first, and stays fully usable with no network connection.
Remote sync, when available, reconciles that local state with an authoritative
backend in the background rather than gating reads/writes on it.

Reference points for the shape of this problem:

- **[Realm](https://www.mongodb.com/docs/realm/)** — embedded object database
  with a live, reactive object model: query results and objects auto-update
  in place as the underlying data changes, and (via Atlas Device Sync) sync
  to a backend with built-in conflict resolution.
- **[RxDB](https://rxdb.info/)** — a JavaScript/reactive local database (built
  on storage adapters like IndexedDB/SQLite) exposing RxJS observables over
  queries, with a replication protocol that's pluggable to different backends
  and transports.
- **[WatermelonDB](https://watermelondb.dev/)** — an offline-first database
  for React/React Native, optimized for large local datasets (lazy loading,
  fast launch) with an explicit sync adapter pattern (`pullChanges` /
  `pushChanges`) for a custom backend.

Common threads to take from these: a reactive/observable query layer, storage
that works fully offline, and a sync layer that's separable from the storage
layer rather than baked in.

## Status

An isolated [Rust spike](spikes/cross-platform/README.md) exercises typed
insertion and equality queries in memory, including a real
[Swift desktop binding](spikes/cross-platform/native/README.md), explicit
subscription draining and staged async handler scopes. A real
[browser/WASM bridge](spikes/cross-platform/web/README.md) passes five cases,
including typed integer boundaries and subscription drains. Native restart
persistence passes; the complete production database remains in progress. The
separate [Phase 1 foundation](docs/implementation/phase-one-foundation.md) now
contains a dependency-free core storage contract, memory and native SQLite
adapters with typed queries, engine-owned indexes, committed changes and a tested
[worker/OPFS browser path](spikes/storage-browser/README.md);
it is an unpublished internal subset, not the complete database API. The
chosen direction is one Rust core compiled to WASM for web and exposed via
native bindings, with a pluggable storage interface and a SQLite-family
default engine. Sync uses named commands as its primary write path and an
authoritative backend.

The [Phase 2 Rust data model](docs/implementation/phase-two-data-model.md) adds
owned schema declarations, generated IDs, lazy relations with managed links,
composable queries and atomic authored migrations or guarded missing-chain reset.
It runs against memory and native/browser SQLite. The
[Phase 3 reactive model](docs/implementation/phase-three-reactivity.md) adds
shared incremental subscriptions, membership and per-column events, projected
throttled counts and owned host-driven delivery. The
[Phase 4 command sync](docs/implementation/phase-four-command-sync.md) adds staged
handlers, atomic durable enqueue, frozen retries, receipt/outcome recovery,
application-defined correction and guarded rollback/cancellation. Its Rust
memory/native SQLite tests and actual Swift/optimized-browser command gates pass.
The [Phase 5 implementation](docs/implementation/phase-five-entity-sync.md) adds
opt-in generated entity patches, durable comparison metadata, table merge hooks,
atomic resumable pull and explicit recovery-preserving or discard reset. Its
Rust, actual Swift and optimized-browser acceptance gates pass. The
[Phase 6 security implementation](docs/implementation/phase-six-security.md) adds
supplied-key SQLCipher storage, fail-closed opens, verified native backup hints
and bounded redacted diagnostics. Actual native/Swift/OPFS and pinned Linux gates
pass; [security integration](docs/guides/security-integration.md) records the
key, backup and memory-copy limits. Packaged platform SDKs remain Phase 7 work.

These design choices are only partially validated. The bounded Stage 0
experiment records results for every exit question; see its
[evidence and remaining gaps](docs/implementation/spike-report.md)
and the [implementation roadmap](ROADMAP.md). Production binding tools remain
open. Initial-sync behavior is specified by ADR-0033/API-0009.
Engine-owned indexing follows ADR-0030.
See the [risk register](docs/design/risks/).

## Getting started

See [runnable tutorials](examples/README.md) for local examples and
[CONTRIBUTING.md](CONTRIBUTING.md) for setup and verification. The
[repository layout](docs/guides/repository-layout.md) maps Rust crates, npm
packages, platform SDKs and release tooling. Packages remain unpublished.

Start with the [documentation index](docs/README.md).
For AI-assisted application integration, use the [tool, skill and reference
catalog](AI.md) and its [portable integration skill](skills/hoyadb-integration/SKILL.md).
The skill describes implementing app features with remotely installed SDK releases,
using packaged guidance and public APIs without a Hoya source checkout.

The formal artifacts indexed by [the registry](docs/design/spec/registry.yaml)
are the source of truth for the current design. Suggested reading order:

1. [Product vision](docs/design/vision/VIS-0001-hoyadb.md) — goals, audience, and non-goals.
2. [System architecture](docs/design/architecture/ARC-0001-system.md) and
   [platform bindings](docs/design/architecture/ARC-0002-platform-bindings.md) — the core
   components and platform boundaries.
3. [Decisions log](DECISIONS.md) and [ADRs](docs/design/decisions/) — chosen directions
   and their trade-offs.
4. [Requirements](docs/design/requirements/), [specifications](docs/design/spec/), and
   [contracts](docs/design/contracts/) — the behavior to implement.
5. [Test plan](docs/design/tests/plan/TP-0001-test-plan.md) and [test cases](docs/design/tests/cases/)
   — how the design is to be verified.
6. [Implementation roadmap](ROADMAP.md) — the build sequence, starting with
   the validation spike.

Development follows the self-contained [contributor guide](CONTRIBUTING.md)
and [project instructions](AGENTS.md). No private framework checkout is required.

[Developer guides](docs/guides/overview.md) retain the glossary, worked examples,
and integration guidance. [Prior-art research](docs/references/README.md) retains
source observations and decision alternatives. Superseded documents are mapped
to current destinations and pinned Git history in the
[documentation history](docs/references/documentation-history.md).

## Workspace development

With Rustup installed, `rust-toolchain.toml` selects Rust 1.98.1, rustfmt,
Clippy and the WASM target. Initial toolchain installation and the locked
dependency fetch require network access; later commands run offline. Core and
memory remain
dependency-free; the native SQLite adapter uses the approved locked dependency.

```sh
cargo fetch --locked
cargo test --workspace --offline --locked
cargo run -p hoyadb-storage-memory --example data_model --offline --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --offline --locked -- -D warnings
cargo build -p hoyadb-core -p hoyadb-storage-memory --target wasm32-unknown-unknown --offline --locked
```

The workspace suite covers shared memory/SQLite CRUD, typed queries, atomic
batches and committed changes, plus native restart, format validation and
operational-error rollback. The browser SQLite experiment now verifies real
worker/page restart persistence
through the same adapter. The isolated [Rust/Swift spikes](spikes/cross-platform/README.md) have separate
commands and lockfiles. GitLab CI is configured; remote execution remains
unverified. See the [foundation evidence](docs/implementation/phase-one-foundation.md)
and [typed storage evidence](docs/implementation/phase-one-storage.md)
for the current API and limitations.

Run the unpublished Rust reactive example:

```sh
cargo run -p hoyadb-storage-memory --example reactive --offline --locked
```

The host drives notification drains and count deadlines; see API-0005 and the
[Phase 3 evidence](docs/implementation/phase-three-reactivity.md) for lifecycle
and scheduling limits.

The opt-in command owner is `hoyadb_core::command::CommandDatabase`; its
[execution and sync contract](docs/design/contracts/API-0006-command-owner.md)
uses the [atomic storage capability](docs/design/contracts/API-0007-command-storage.md).
A host samples trusted context, drives begin/scoped operations/finish or abort,
and supplies transport, timers and application-specific reconciliation plans.
Core performs no network calls. These APIs remain unpublished; queue snapshots
and serialization copy/validation costs grow with retained work. See the
[measured costs, profile-specific results and limits](docs/implementation/phase-four-command-sync.md).

### Code formatting

[`rustfmt.toml`](rustfmt.toml) defines the Rust style using stable options:
four spaces, LF line endings, a 100-column layout target, multiline struct
literals and named enum variants, and multiline conditional blocks. Keep one
blank line between declarations and methods, with attributes and documentation
attached to their item. Stable rustfmt preserves these separators but does not
insert or enforce them. Long strings and other unbreakable tokens can exceed
the width target. [EditorConfig](.editorconfig) supplies editor defaults.

Format the workspace with `cargo fmt --all`. The spikes are excluded from the
workspace, so format each separately with `cargo fmt --manifest-path <path>`.
All packages inherit the root formatter configuration. Check the full Rust tree:

```sh
sh tests/tooling/formatting.sh
cargo fmt --all --check
cargo fmt --manifest-path spikes/cross-platform/Cargo.toml --check
cargo fmt --manifest-path spikes/cross-platform/native/Cargo.toml --check
cargo fmt --manifest-path spikes/cross-platform/web/Cargo.toml --check
cargo fmt --manifest-path spikes/storage-browser/Cargo.toml --check
```

GitLab CI checks these layouts. [SPEC-0030](docs/design/spec/SPEC-0030-rust-formatting.md)
records the scope and [TC-0033](docs/design/tests/cases/TC-0033-rust-formatting.md)
records verification. JavaScript, Swift and Python formatting is outside this pass.
