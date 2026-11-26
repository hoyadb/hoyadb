# Contributing to hoyadb

This source tree is being prepared for public distribution. Packages remain
unpublished. First-party code is licensed under [MIT](LICENSE); third-party
notices retain their own terms. Maintainers must configure the security-reporting
channel and complete release gates before public launch.

## Start here

Read [AGENTS.md](AGENTS.md), the [repository layout](docs/guides/repository-layout.md)
and the relevant contracts under [docs/design](docs/design/README.md). The public
tools and instructions are self-contained: do not initialize a private submodule.
The archived process references in historical records are background only.

Rustup selects the toolchain and components in `rust-toolchain.toml`. JavaScript
verification currently uses Node 22 and npm 11. Install/fetch once before offline
checks:

```sh
cargo fetch --locked
cargo fetch --manifest-path crates/hoyadb-native/Cargo.toml --locked
cargo fetch --manifest-path crates/hoyadb-web/Cargo.toml --locked
npm ci --offline --ignore-scripts --no-audit --no-fund
```

The JavaScript application tests use the real WASM bridge. Generate its ignored
ordinary assets before `npm test`; a clean checkout does not contain them:

```sh
cargo install wasm-bindgen-cli --version 0.2.129 --locked --root target/tools/wasm-bindgen
sh crates/hoyadb-web/build.sh
```

The install step needs network access. The build uses the fetched bridge lockfile
offline and requires `rg`, a WASM-capable Clang, and `llvm-ar`. On Linux, install
`clang`, `llvm`, and `ripgrep` and export `CC_wasm32_unknown_unknown=clang` and
`AR_wasm32_unknown_unknown=llvm-ar`; on macOS see the
[bridge toolchain instructions](crates/hoyadb-web/README.md). An existing exact
wasm-bindgen 0.2.129 CLI may be selected with `HOYADB_WASM_BINDGEN`. CI's
`javascript-wasm` job builds the same ordinary assets and supplies them to the
Node job through an artifact dependency. The GitHub CI workflow builds WASM and
validates packed consumers in one job; see [GitHub delivery](docs/guides/github-delivery.md).
Host tests exercise the memory bridge;
they do not establish browser OPFS or mobile runtime support.

The npm coordinator contains only the client/testing workspaces, as before the
reorganization. React Native's real app dependencies and platform toolchains
remain governed by its [activation instructions](packages/react-native/ACTIVATION.md).
No mobile compiler or runtime support is implied by passing host tests.

## Change workflow

Inspect Git status and preserve existing changes. Describe the intended behavior,
scope, acceptance cases and rollback in the relevant design spec before coding.
Use the next available artifact IDs in `docs/design/spec/registry.yaml`; do not
reuse IDs or change accepted ADR bodies. Append material decisions to
`DECISIONS.md`. For a behavior fix, reproduce it with a meaningful failing test,
make the smallest correction, then rerun affected checks.

Keep public contracts, storage compatibility and security boundaries intact.
Do not add dependencies, new abstractions or platform claims without evidence.
Never weaken tests, suppress first-party coverage or label missing results as
passing. Update current guides with the code; preserve historical evidence.
Changes require maintainer review before merge. Do not commit, publish, rewrite
shared history or make destructive changes on behalf of someone without their
explicit instruction. Existing release versioning is `vMAJOR.MINOR.PATCH`.

Public API changes also run `python3 tools/release/compatibility.py sources` and
the compatibility tool's tests in `tools/release/test_compatibility.py`. That
source gate pins the explicitly reviewed declarations; it does not certify
native ABI or runtime behavior. On macOS, the fresh Swift comparison and native
export checks are documented in [the release tools guide](tools/release/README.md).
Keep historical snapshots unchanged and record any approved API additions
separately before updating the current review manifest.

## Required checks

Run the affected language/platform checks and these source-tree checks:

```sh
cargo test --workspace --offline --locked
sh tests/tooling/formatting.sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --offline --locked -- -D warnings
RUSTFLAGS='-D warnings' cargo build -p hoyadb-core -p hoyadb-storage-memory --target wasm32-unknown-unknown --offline --locked
npm test
npm run test:react-native
python3 -m unittest discover -s tools/traceability -p 'test_*.py'
python3 -m unittest discover -s tests/packaging -p 'test_*.py'
python3 -m unittest discover -s tests/tooling -p 'test_sync_proof.py'
python3 tests/tooling/sync_proof.py
python3 tools/traceability/trace_check.py docs/design
python3 tools/traceability/rtm_gen.py docs/design --check
```

When artifact relationships change, regenerate the matrix with
`python3 tools/traceability/rtm_gen.py docs/design`, review the output and include
it in the change. Do not hand-edit it. Test fixtures stay outside `docs/design`
so intentionally invalid fixture artifacts never enter the product trace graph.

The sync proof checker pins the single core `SyncDelta` construction and its
validated Exclusive gate (SPEC-0037 / TC-0041 SI-Y03). Its negative-control tests
exercise rejected constructor references, duplicate sites, trait implementations
and weakened gates. It checks a narrow Rust token convention; visibility,
compile-fail tests and semantic review remain necessary.

The native and web bridge crates remain standalone with separate lockfiles. Use
their [native](crates/hoyadb-native/README.md) and [web](crates/hoyadb-web/README.md)
commands as well as the [SDK guide](docs/guides/sdk-integration.md). Public docs
have their own [verification workflow](docs/public/README.md). See
[test quality](docs/guides/test-quality.md) for coverage/mutation requirements and
[release checks](tools/release/README.md) for artifact consumption checks.

In a review, state the problem, resulting behavior, relevant artifact IDs, actual
commands/results and any unverified targets. A successful build alone does not
establish runtime support. Publication remains a separate explicit action.
