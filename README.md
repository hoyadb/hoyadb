# HoyaDB

HoyaDB is an offline-first embedded database with local persistence, reactive
queries, and a separable sync layer. Applications read and write their local
store; the host supplies transport and trusted backend context when syncing.

## Capabilities

- A shared Rust core with memory and SQLite storage adapters.
- Declared schemas, typed queries, relations, migrations, and atomic batches.
- Reactive query subscriptions with explicit count and column opt-ins.
- Durable command queues, receipts, reconciliation, and entity synchronization.
- JavaScript/TypeScript, React Native, Swift, and Android integration surfaces.
- Protected storage and bounded diagnostics with explicit platform requirements.

The [technical contracts](docs/design/contracts/) and
[SDK integration guide](docs/guides/sdk-integration.md) define the available
interfaces and their limits. Packages remain unpublished. Host checks do not
establish mobile runtime support, and native publication requires fresh platform
acceptance. See the [delivery guide](docs/guides/github-delivery.md).

## Getting started

Start with the [runnable tutorials](examples/README.md),
[documentation](docs/README.md), and [contributor guide](CONTRIBUTING.md).
The [repository layout](docs/guides/repository-layout.md) maps packages and tools.
The [AI integration catalog](AI.md) provides packaged integration guidance.

Rustup selects the pinned toolchain in `rust-toolchain.toml`:

```sh
cargo fetch --locked
cargo test --workspace --offline --locked
cargo run -p hoyadb-storage-memory --example data_model --offline --locked
cargo run -p hoyadb-storage-memory --example reactive --offline --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --offline --locked -- -D warnings
```

The host drives notification drains and count deadlines. Core performs no
network calls. Standalone bridges and experiments have separate lockfiles and
target requirements; use their documented commands as well as workspace checks.

## Design and verification

Read the [vision](docs/design/vision/VIS-0001-hoyadb.md),
[architecture](docs/design/architecture/ARC-0001-system.md),
[decisions](DECISIONS.md), and [test plan](docs/design/tests/plan/TP-0001-test-plan.md).
Formal contracts describe intended behavior; passing checks establish only the
scope actually exercised. Platform and publishing gates remain explicit.

GitHub Actions checks, builds, and prepares manually approved releases.
[MIT](LICENSE) covers first-party code; [third-party notices](THIRD_PARTY_NOTICES.md)
retain their own terms.
