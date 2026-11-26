# Final integrated workspace checks — read-only review (Claude Fable 5.1)

Date: 2026-10-31. Packet `/tmp/hoyadb-followup-20261031/r10-integrated-checks/packet.json`
`024444ec…6e574d` (matches). No repository edit; nothing rerun by me.

## Source boundary (verified)

- `source-before.json == source-after.json` (187 inputs). Against current main the only drift is
  `crates/hoyadb-core/src/command/ledger_txn.rs` (`b22c7338…` → `87ea05ed…`), exactly the
  DEF-0063 two-attribute delta recorded in `post_workspace_delta` and reviewed separately
  (`ledger-diagnostic-cfg/fable-review.md`, with my appended correction).
- `r10-final-runtime-source.json` (187) has no drift against main and differs from the
  workspace-run map only in `ledger_txn.rs`.
- Every `evidence_sha256` entry (12 files across `r10-integrated-capture.json`,
  `r10-final-runtime-source.json`, this packet and `ledger-diagnostic-cfg/`) matches on disk.
- `test-binaries.json`: 85 executables listed, all present under `r10-integrated-target`.

## Count methodology (independently recomputed from `workspace.log`)

- 91 `test result:` lines: sum passed 882, failed 1, ignored 0. 89 `Running`/`Doc-tests` blocks.
- The single failing line (`0 passed; 1 failed; … 2 filtered out`) is the nested child
  `quality07_json_parser_is_bounded ... FAILED` spawned by `tests/json_bounded.rs`; the outer
  block then reports `quality07_json_parser_is_bounded ... ok`, `…supervisor_rejects_empty_and_
  failed_children ... ok`, `…reaps_nonterminating_child ... ok`, i.e. the supervisor asserts
  that failure. Counting the last outer result per Cargo block, as `workspace-counts.json`
  states, gives 882 / 0 / 0 over 89 suites. My recount agrees; the 91 vs 89 difference is the two
  nested child result lines. Methodology is sound and stated explicitly.
- `cargo test --workspace --offline --locked` exit 0, 821.8 s, `CARGO_INCREMENTAL=0`,
  dedicated target dir.

## Other commands

- `cargo fmt --all --check` exit 0; `cargo clippy --workspace --all-targets -- -D warnings` exit 0,
  no warnings in `clippy.log`.
- `wasm-core` and `wasm-sqlite` builds exit 0 **with one warning each**: the pre-cfg
  `record_keys`/`metadata_keys` dead-code warning (`ledger_txn.rs:250`). These runs did not deny
  warnings, so they pass; the production `-D warnings` proof on the post-cfg source is the
  separate `ledger-diagnostic-cfg` build. The packet names this boundary; the final report must
  not describe the integrated WASM builds as warning-free.
- Toolchain recorded: rustc/cargo 1.98.1, LLVM 22.1.8.

## Findings

None actionable. Scope note (INFO): the 882-test workspace run certifies the pre-cfg 187-input
map; the post-cfg map is covered by the separate core 219 / production WASM `-D warnings` /
Clippy / fmt runs, not by a full workspace rerun. Root's stated intention to keep that boundary
explicit is correct.

## Limits

Not rerun by me. Host Rust only; platform lanes and R11 are separate.
