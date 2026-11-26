# Retained Fable response: Engine delta forwarding and Apple build inventory

Transcribed verbatim from the completed Paseo notification in this conversation.
Agent: 0b48cccf-9809-4e39-90aa-ce9291333283. Message: c222bc15-2d27-4ee9-9a04-f8d9c289ae26.
The original notification ended with a truncated final sentence; no missing words are inferred.

Both packets reviewed. Nothing further to request.

## 1. Engine `apply_sync_delta` forwarding

**Method verified.** `engine-delta-current-work` and `engine-delta-no-forward-work` each bind 177 inputs to their manifests with zero mismatches. The two trees differ in exactly one file, `engine.rs`, and the diff is exactly the seven-line `apply_sync_delta` match-forward (same shape as the other Engine forwards). Current main `engine.rs` is byte-identical to the forwarding tree.

**Regression meaning confirmed.** The negative control fails for the intended reason, not a setup fault: with the method removed the Engine inherits the provided default, which runs `load_sync` and the full plan, so whole-patch work grows with M. RED values: `packets_parsed` 112 vs 7042 (memory and SQLite), `rows_cloned` 43 vs 2023 / 25 vs 1015, SQLite `rows_written` 90 vs 6030 and `fullscan_steps` 426 vs 36066, which also trips the zero-fullscan assertion. With forwarding, both tests pass with equal counts at M=10 and M=1000. The test (`bounded_delta_work`) asserts positive encode/clone counts, equality of parsed, encoded, cloned and written across M, and zero fullscan on both sizes. Adequate for SI-Y03's "Engine forwarding test proves its delta call reaches the adapter".

**Scope note, not a defect.** The GREEN equality at M=1000 is only possible because the compiled core in `engine-delta-current-work` already uses the `LedgerTxn` persist (`owner.rs` `a5460691…`, `SyncDelta::validated_exclusive(SyncDeltaParts{…})` at line 828), not the earlier full-validation bridge; main `owner.rs` is already different (`00cdf4b8…`). The Engine test therefore demonstrates forwarding under that in-progress core snapshot. Record the compiled `owner.rs` hash with the packet and rerun once the planner conversion freezes; no planner completion claim follows from this packet, as root states.

## 2. DEF-0060 Apple builder source inventory

**Change is scoped and correct.** `sources()` now captures root `Cargo.toml`, root lockfile and toolchain, the builder itself, `crates/hoyadb-native/Cargo.lock`, every file under each of the five crates' `src` (not only `.rs`), each crate `Cargo.toml`, `build.rs` when present, `.cargo/config` and `.cargo/config.toml` at root/`crates`/crate, and all header files; it is evaluated before and after compilation (`build-artifacts.py:46,78`). A missing required input raises `FileNotFoundError` via `digest`, so a build record cannot be written. Generated outputs and credentials are excluded by construction. Packaging suite: 9 pass here; confirmed RED log shows 8 failures / 1 control against the old function with the canonicalised fixture. The fixture executes the real function definitions via AST without running the platform build, which is the right way to test a script with module-scope side effects.

**Observations, none blocking.**
- Cargo config discovery is cwd-based and the builder runs with `cwd=root`, so only `root/.cargo` actually applies to these invocations; capturing `crates/.cargo` and per-crate `.cargo` is harmless over-capture. The user-level `~/.cargo/config.toml` and process environment (`RUSTFLAGS`, `CC_*`, `MACOSX_DEPLOYMENT_TARGET` is set explicitly) remain outside, which DEF-0060 and the README already declare as the separate toolchain-capture gate.
- No `build.rs` exists today in any of the five crates and `crates/hoyadb-native` has no own `rust-toolchain.toml`, so the root toolchain file governs; the fixture-only `build.rs`/resource cases are the correct way to pin future additions.
- Resources read from outside `src` or graph changes are explicitly out of scope and stated in the README; acceptable as a documented limit rather than a universal provenance claim.
- Historical certificates not re-verified here beyond the README diff; the DEF states they are untouched.

**Verdict.** Both scoped changes approved as described: Engine forwarding with a meaningful negative control, and the Apple inventory fix with behavioural RED. No
[original notification truncation: 79 characters omitted]
