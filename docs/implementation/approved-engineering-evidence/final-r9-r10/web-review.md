# Final ordinary WASM / JavaScript package evidence closure — read-only review (Claude Fable 5.1)

Date: 2026-10-31. Packet `/tmp/hoyadb-followup-20261031/final-web-refreshed` (`packet.json`
`9f49ce9b…309fd`, `review-packet.json` `08a266e6…b6ab9`, both match). No repository edit; nothing
rerun by me. Not a browser/OPFS, React Native device, Android, Apple or whole-workspace verdict.

## Source / artifact linkage (verified)

- `recorded_artifacts`: every listed file matches its sha256. `sources-before.json ==
  before-sources.json`, and all eight `*-after-sources.json` plus `final-sources.json` are
  byte-identical to it; all `*-after-cargo-metadata.json` equal `before-cargo-metadata.json`.
  The 5,359-input closure (511 `repo/` + 4,848 `external/`) was stable across every stage.
- All 511 repository inputs equal current main bytes; all 4,848 dependency inputs equal the
  on-disk Cargo sources they name (including the pinned rusqlite `661107f` checkout).
- `runtime-freeze.json` (root's 187-input freeze) has no drift against main;
  `ledger_txn.rs` is the post-DEF-0063 `87ea05ed…`.
- `final-generated.json` matches every file under `final-generated/`. The four hashes quoted in
  `REVIEW.md` resolve to real files: raw `hoyadb_web.raw.wasm` `4ffee501…`, bindgen
  `pkg/hoyadb_web_bg.wasm` = `dist/client/src/wasm/hoyadb_web_bg.wasm` `9a37bbce…`, client and
  testing tarballs `21205995…` / `70266fad…`.
- Both pack-time `SOURCE-MANIFEST.json` files: 76/76 `sources` entries equal the independently
  captured pre-build `repo/` hashes; their `artifacts` entries (`hoyadb_web.js`,
  `hoyadb_web_bg.wasm`, vendored `Cargo.lock`) equal the copied files; `testControls: false`.
  So the manifest is bound to the compile inputs, not self-described.

## Stage claims (verified against logs and scripts)

| Stage | Observed |
| --- | --- |
| `sh crates/hoyadb-web/build.sh` | release profile, exit 0, 0 `warning` lines, no `--test-controls` |
| `node crates/hoyadb-web/tests/memory.mjs` | `PASS real preloaded WASM retained-memory/CRUD/UTF8/hermetic capabilities` |
| `npm test` | 207 + 11 = 218 pass, 0 fail/skip/cancelled/todo |
| `npm run test:react-native` | 10 pass, 0 fail/skip |
| `build:packages`, `check:packages` | exit 0 |
| `test:packaging` | `npm pack --ignore-scripts`, `npm install --offline --ignore-scripts --no-audit --no-fund` of the two local tarballs in a `mkdtemp` consumer outside the checkout; three `tsc --noEmit --strict` passes including the no-DOM portable surface; real WASM memory and offline examples; `PASS independent packed npm consumer`; consumer directory removed in `finally` |
| `compatibility.py sources` | exit 0 with the drift report printed (`check_inputs` raises and returns 1 on any mismatch; the `sources` report carries no `passed` key), scope line "reviewed source drift; not runtime or semantic API certification" |

Environment: `CARGO_TARGET_DIR` popped, `CARGO_BUILD_JOBS=2`, `CARGO_INCREMENTAL=0`, explicit
LLVM 22 clang/ar and wasm-bindgen 0.2.129, `npm_config_cache` pointed at the task-owned
`final-web-refreshed/npm-cache`. `tools.json` records Rust 1.98.1, Node 22.18.0, npm 11.17.0,
TypeScript 7.0.2 with executable digests, as the report states. No registry fetch, no permission
change; the superseded `../final-web` EPERM attempt is separate and not reused as evidence.

## Findings

None. The REVIEW.md claims are supported by the packet; scope limits are stated correctly.

Observation (INFO): `build.sh` does not itself deny warnings; the "no warnings" statement rests
on the 0-warning log, while the production `-D warnings` proof for the helper gate is the separate
DEF-0063 WASM build. Both hold; wording could cite them distinctly.

## Limits

Not rerun by me. Host Node and ordinary WASM only. Apple, Android and the full workspace run are
still executing and are not certified here.
