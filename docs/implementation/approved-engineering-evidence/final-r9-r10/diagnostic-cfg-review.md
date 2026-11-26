# DEF-0063 LedgerTxn diagnostic helper cfg boundary — read-only review (Claude Fable 5.1)

Date: 2026-10-31. Scope: `crates/hoyadb-core/src/command/ledger_txn.rs` `87ea05ed…` (main) vs the
frozen R10 import `b22c7338…`, with `ledger-diagnostic-cfg/{red.log,red.json,green-results.json,
green-source-{before,after}.json}`. No repository edit; no build/test run by me.

## Binding

- `diff` frozen → main: exactly two added lines, `#[cfg(any(test, feature = "work-counters"))]`
  on `record_keys` (line 250) and `metadata_keys` (line 254). No other change.
- `red.json`: `cargo build -p hoyadb-core --lib --target wasm32-unknown-unknown --offline
  --locked` with `RUSTFLAGS=-D warnings` on `r10-integrated-work` (`ledger_txn` `b22c7338…`),
  exit 101; `red.log` shows the dead-code error for exactly those two methods. Genuine production
  compile failure, not a test-only diagnostic.
- `green-source-before == green-source-after` (187 inputs); `green-work` matches the manifest and
  current main has no drift. Green checks: production WASM build of core + memory with
  `-D warnings` exit 0 (`production-wasm.log` no warnings); `cargo test -p hoyadb-core` 210 + 3 +
  6 pass (the nested `quality07…` FAILED is the supervisor's negative control); core
  `--all-targets` Clippy exit 0, no warnings; `cargo fmt --all --check` exit 0.

## Boundary analysis

Callers of the two helpers (repo-wide grep):
- `owner.rs:551-552` inside `#[cfg(feature = "work-counters")]` block; the receiving field
  `ledger_work_keys` (owner.rs:106) and its initializer (211) are gated the same way.
- `reconcile.rs:3203-3207` inside `#[cfg(test)] mod tests` (module opens at line 1099).
- `ledger_txn.rs` own `#[cfg(test)]` tests.

Every caller's cfg implies `any(test, feature = "work-counters")`, so no configuration can
reference a compiled-out helper: lib without feature (ordinary native/WASM) has neither callers nor
helpers; lib with feature (adapter counter suites, `ledger_work` consumers) has both; `cfg(test)`
has both. Clippy `--all-targets` exercised the lib-without-feature arm (no dead code) and the test
arm together; the memory/SQLite `--features work-counters` suites in the R10 integrated run
exercise the feature arm. This is a visibility gate, not an `allow(dead_code)`; no runtime code
path changes.

## Findings

None. Observation (INFO): the fix was necessary only because `record_keys`/`metadata_keys`
exist solely for TC-0041 observation; gating them with the same predicate as their only production
caller is the minimal correct change. The permanent `RUSTFLAGS='-D warnings'` WASM build in
`CONTRIBUTING.md:83` / `.gitlab-ci.yml:26` now covers this class.

## Limits

Not rerun by me. The root full R10 workspace run against the 187-input pre-cfg snapshot and the
platform recaptures are pending and not treated as passed; the pre-cfg snapshot's test
configurations already enable the helpers, so their results are unaffected by this change, but
its production WASM build must be re-executed on the post-cfg source (done here once, exit 0).

## Correction (appended 2026-10-31, after root's evidence check)

The "Boundary analysis" paragraph says core `--all-targets` Clippy "exercised the
lib-without-feature arm". That is wrong. `crates/hoyadb-core/Cargo.toml:13` declares a
dev-dependency on itself with `features = ["work-counters"]`, so any `cargo clippy --all-targets`
or `cargo test` on the core package unifies the feature into the lib and never compiles the
no-feature arm; this is why the earlier all-target gates did not catch the dead helpers.

The actual observed no-feature proof is the dedicated ordinary build
`RUSTFLAGS='-D warnings' cargo build -p hoyadb-core -p hoyadb-storage-memory --target
wasm32-unknown-unknown --offline --locked` (RED exit 101 on `b22c7338…`, GREEN exit 0 on
`87ea05ed…`). Core tests and the adapter counter suites cover the `test` and `feature` arms.
Verdict unchanged.
