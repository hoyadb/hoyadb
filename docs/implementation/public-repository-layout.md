# Public repository preparation

Updated: 2026-10-30. Source scope: REQ-0048, SPEC-0045 and TC-0049, authorized by
the operator's approval of both the folder reorganization and private-governance
separation. No commit, publication, license selection or history rewrite occurred.

## Implementation

The [layout guide](../guides/repository-layout.md) records the source moves.
Rust bridge lockfiles and standalone workspace boundaries are preserved. The root
npm coordinator retains client/testing membership and React Native retains its
separate platform setup. Shared JavaScript builders, ordinary consumer fixtures
and notice corpora have separate homes. No Rust database algorithm or public API
was changed by this task.

[CONTRIBUTING.md](../../CONTRIBUTING.md) and [AGENTS.md](../../AGENTS.md) replace
the public workflow's private dependency. The Git index no longer tracks the
private submodule or `.claude/skills`; local contents remain intact and ignored,
as do the existing local `.agents` skills. New/moved public files are marked
intent-to-add for rename review; their contents are not staged. The tracked
submodule configuration is removed. Historical Git references remain.

The public traceability subset retains the exact implementation, original tests
and fixture bytes recorded in [its provenance](../../tools/traceability/provenance.json).
An initial working-tree snapshot was saved under `/private/tmp/hoyadb-layout-before`.
The integrity comparison found no missing existing source files other than the
intentionally removed `.gitmodules`. All 2,262 checked historical/research,
accepted-ADR and third-party notice files retained their initial bytes. The
React Native copied-source provenance record and existing documentation browser
report are also retained as historical evidence, without recapturing their hashes.

## Rules

Keep existing source edits through the moves. Do not turn historical reports into
current certificates by changing their paths or hashes. Native documentation
builds still use an archive of the recorded HEAD commit, selecting that commit's
old or new native path. Their report does not verify uncommitted native/core edits.
The Swift and C tutorials ran against that explicitly archived source, not the
current full working-tree core. Native-boundary tests and WASM/npm builds exercised
the working-tree sources separately.

Do not relax a gate because a baseline hangs or an artifact certificate is stale.
The public repository is prepared structurally, not declared ready for release.
Licensing, private vulnerability reporting, registry ownership, supported target
claims and remote Apple delivery remain release decisions.

## Required checks

The following were executed; local raw logs are `/private/tmp/hoyadb-layout-*.log`.
Counts refer to the named suite, not total coverage or additional platform support.

| Command / check | Observed result |
| --- | --- |
| `python3 -m unittest discover -s tests/packaging -p 'test_*.py'` | Initially failed because the public tools were absent; now 3 pass. Isolated checker rejects invalid artifacts and RTM check rejects stale output. |
| `python3 -m unittest discover -s tools/traceability -p 'test_*.py'` | Original 6 regression tests pass. |
| `python3 tools/traceability/trace_check.py docs/design` | 0 violations. |
| `python3 tools/traceability/rtm_gen.py docs/design --check` | Regenerated matrix is current. |
| Root `npm ci --offline --ignore-scripts --no-audit --no-fund` and `npm test` | Install passes; client 162 and testing 11 tests pass. The pre-move suite passed the same total of 173. |
| `npm run test:react-native` | 10 host tests pass before and after moves. |
| WASM bridge build and `node crates/hoyadb-web/tests/memory.mjs` | Optimized ordinary build and real retained-memory/CRUD/UTF8/hermetic-capability smoke pass, using the existing exact-version CLI override. |
| `npm run build:packages`, `npm run check:packages`, `npm run test:packaging` | Package assembly, notice checks and outside-checkout tarball installation/typechecking/real WASM consumer pass. Packing used `npm_config_cache=$PWD/target/npm-cache` because the default cache is outside the writable sandbox. |
| `cargo test -p hoyadb-core --offline --locked` | 142 unit, 3 bounded-parser supervisor and 2 compile-fail doctests pass. The supervisor intentionally observes a failing synthetic child. |
| Native bridge test command from its README | 3 unit and 6 boundary tests pass before and after moves. |
| `cargo test --manifest-path examples/Cargo.toml --offline --locked` | Passes, including 6 command tutorial tests. |
| Workspace and standalone native Clippy commands | Pass with `-D warnings`. |
| Core/memory WASM library build | Passes offline/locked. |
| Native/web standalone formatting; `sh tests/tooling/formatting.sh` | Pass. |
| Public documentation Python tooling | 45 pass, including a red-first archived-layout regression. |
| `docs/public/check_sources.py` for Kotlin and RN bootstrap; public symbol inventory | Pass source checks; these are not mobile runtime execution. |
| JavaScript, application, TypeScript and shared RN tutorials | Execute successfully against the rebuilt WASM/packages. |
| Swift and C documentation tutorials | Execute successfully against the explicitly archived native build; Swift needs the normal compiler/manifest sandbox permissions. |
| Apple notice tooling / Android tooling / RN security / RN Android repository tooling | 53 / 52 / 130 / 16 tests pass respectively. These use synthetic packaging/security fixtures, not actual mobile applications. |
| Benchmark tooling / consumer-server fixtures / quality tooling | 93 / 3 / 29 tests pass. Installer fixture exposed old import substitutions, then passed after the path correction. No new benchmark measurements are claimed. |
| `python3 tests/tooling/git_provenance.py` | Passes actual Git clean/checkout byte checks with an isolated index under all three autocrlf modes. |
| Python compilation and new public-guide links | Pass. |
| `git diff --check` | Pass after final review. |

Unresolved checks:

- Full `cargo test --workspace --offline --locked` stalled before the moves in
  `c09_c12_c16_c19_dispatch_delivery`, and the post-move run timed out after
  180 seconds in the same test. No test was skipped or weakened to claim a pass.
- Root `cargo fmt --all --check` reports formatting differences in
  `command/entity.rs`, `command/reconcile.rs`, `command/records.rs`,
  `tests/conformance/entity_overlap.rs` and `tests/conformance/entity_promotion.rs`.
  Their bytes match the initial working-tree snapshot; this task did not reformat
  unrelated user/core work.
- The application browser check could not start installed Chrome, even with
  permission for its temporary profile and loopback server. There is no new browser
  runtime or full documentation-site verification certificate from this task.
- The Apple builder compiled the requested slices and created the XCFramework,
  then correctly rejected it with `stale-provider-evidence`: the retained
  CommonCrypto evidence does not match rebuilt library bytes. Its provider evidence
  must be regenerated by the reviewed verification procedure before distribution;
  the existing corpus was not rewritten to make it pass.

## Update this page when

Record reruns that resolve these specific baseline/platform gates, and keep source
provenance explicit. Publication or opening the remote source repository needs its
own concrete release review; this source-layout task grants neither a license nor
new platform support.
