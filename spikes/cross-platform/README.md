# Cross-platform validation spike — first host slice

Implements the narrow experiment in
[SPEC-0016](../../docs/design/spec/SPEC-0016-initialize-validation-spike.md).
This is a non-publishable, dependency-free crate with one hardcoded typed
table and an in-memory store. Unique IDs are fixture inputs; duplicate-ID
behavior is unspecified. Query result order is unspecified.

Run from the repository root with Rust/Cargo and the rustfmt and Clippy
components installed:

```sh
cargo test --manifest-path spikes/cross-platform/Cargo.toml --offline --locked
cargo fmt --manifest-path spikes/cross-platform/Cargo.toml --check
cargo clippy --manifest-path spikes/cross-platform/Cargo.toml --offline --locked --all-targets -- -D warnings
```

Initially validated with Rust/Cargo 1.92.0-nightly on x86_64 macOS; all spike
checks also pass on Rust 1.98.1 after the Phase 1 foundation was added.
No minimum supported Rust version is claimed for this experiment.
The crate owns its manifest and lockfile and is excluded from the root Cargo
workspace. Commands inherit the root's Rust 1.98.1 toolchain pin; `cargo +nightly`
selects the separately installed nightly toolchain for original-toolchain checks.

## TDD evidence

Before implementing storage, the five tests compiled and failed with exit 101
at explicit unimplemented insert/query operations (2026-10-20). S01, S02,
S04 and S05 reached the missing insertion; S03 reached the missing query.
After implementation, rerun the commands above to verify the current result.

The [native slice](native/README.md) adds a real Swift desktop binding over this
shared core. Its subscription drain and async-scope experiments exercise
core-owned membership and staged writes. The [browser slice](web/README.md)
executes the same core in real WASM; native SQLite mode additionally proves
restart persistence. The [Stage 0 report](../../docs/implementation/spike-report.md)
records all seven bounded findings, including the negative browser-persistence
result for the selected memory path. These experiments establish neither
production readiness nor complete product requirement coverage.
