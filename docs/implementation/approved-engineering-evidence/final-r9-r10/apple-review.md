# Final Apple R9/R10 evidence — independent read-only review (Claude Fable 5.1)

Date: 2026-10-31. Packet `/tmp/hoyadb-apple-r9-r10-final/review/{packet.md,source.json}`; durable
record `docs/implementation/approved-engineering-evidence/apple-r9-r10-verification-20261031.json`
(`f1be46ae…`). No repository file edited. Static evidence review: I read logs and JSON and
recomputed hashes/diffs; I did not rebuild, rerun Swift/native tests or the consumer.

## Checks I performed (hash/diff recomputation)

- `source.json`: all 9 owned docs/index/evidence files equal current main bytes, including
  `crates/hoyadb-native/third-party/index.json` `25cdfe0a…`.
- **Provider index delta** (current index vs retained `apple-notice-index-before-…` `f401663e…`):
  692 flattened keys each side, exactly 10 differ: six `packages[16|17].evidence[0..2].sha256`
  (old archives `b92e3f27…`/`45a3de49…`/`cc1c4649…` → fresh thin archives `4e753c24…`/
  `3d2a248d…`/`2ff2ce22…`), and `provider_evidence_source.{file,sha256}` →
  retained first parent `57baa8be…`, `derivation.{file,sha256}` → refresh record `42d80500…`.
  Nothing else changed. `final-source-artifact-comparison.json` and `inputs-final-{before,after}`
  agree: packaging group differs only in `index.json`; runtime 72 / sdk 45 / verification 44 /
  external 2,508 / toolchain 76 unchanged; all repo-relative inputs equal main today.
- **First vs final compilation parent** (`57baa8be…` vs `f6b92990…`): 155 flattened keys each,
  exactly one differs: `artifact_files_sha256["HoyaNative.xcframework/Info.plist"]`
  `3e65c693…` → `69e3575d…`. Archive, header and module-map hashes identical, matching
  `retained-parent-reference.json` (`plist_equal_after_sorting_libraries: true`) and
  `warm-rebuild-comparison.json` (changed: Info.plist, native-build-parent.json, packaging.json,
  native-build.json only).
- **Current generated output** (`bindings/swift/Artifacts`): Info.plist `69e3575d…`,
  `native-build-parent.json` = `native-build.json` = `f6b92990…`, `packaging.json` `0432e12a…`,
  `third-party/index.json` `25cdfe0a…`, macOS archive `4e753c24…`, iOS `3d2a248d…`, simulator
  `2ff2ce22…`, headers `2f61e513…`. Equals the verification record's `final_artifact_files` and
  the `artifact-check-final.log` result (`parent f6b92990…`, `packaging 0432e12a…`, `index
  25cdfe0a…`, 39 members = 10 native + 28 notice + Info.plist).
- **First runtime package** (`swift-sdk/Artifacts/HoyaNative.xcframework`): `packaging.json`
  `4b26d25e…`, `native-build-parent.json` `57baa8be…`, Info.plist `3e65c693…`, index
  `354fc720…` (post-refresh, pre-pointer-correction), same three archives and headers as the
  final package. So the Swift runtime evidence binds to byte-identical native code; only
  metadata JSON/plist ordering differs, as claimed.

## Evidence read (not rerun)

| Claim | Observed |
| --- | --- |
| First builder refused stale provider proof | `native-build-result.json` exit 1, scope "finalizer rejected stale-provider-evidence"; all four Rust builds and xcframework creation completed |
| Fresh provider inspection, all thin archs | `provider-symbols/*` stdout/stderr per slice/arch; `provider-refresh.log` old `f401663e…` → new `354fc720…`, parent `57baa8be…`; README: 9 CommonCrypto + 2 Security imports, no provider definitions, no LTC patterns |
| Refreshed normal builder | `normal-builder-refreshed.log` `HOYA_APPLE_ARTIFACT_BUILD_GREEN source_files=72`, packaging `0061dfcd…` with index `354fc720…` (superseded by the pointer correction) |
| Finalizer behaviour | `notice-finalizer-result.json` exit 0; `…-restaged` exit 0 on the fresh 10-member stage; `…-final` exit 2 = intentional refusal of an already-noticed framework (`REJECTED unexpected: … found ['NOTICE.md', …]`) |
| Five artifact checks | `artifact-check.log` (parent `57baa8be…`) and `artifact-check-final.log` (parent `f6b92990…`): `Ran 5 tests … OK` |
| Export checks | three `compatibility.py exports` exit 0 using rustup `llvm-nm -gU` on each slice; macOS report lists the four `_hoya_v1_*` symbols, library sha `4e753c24…` |
| Swift API | `compatibility.py swift` exit 0 |
| Swift tests | first run exit 1: sandboxed SwiftPM/clang module cache `Operation not permitted` before compiling the manifest; normal-host rerun exit 0, `Executed 102 tests, with 0 failures`, `-warnings-as-errors` |
| Native | `cargo test` 3 + 6 + 0 doc = 9, exit 0; `clippy --all-targets -- -D warnings` exit 0; fmt exit 0 |
| Consumer | build exit 0 with warnings as errors; restricted writer `HOYA_SWIFT_GENERAL_CONSUMER_FAILED backupPolicyFailure` retained; normal-host writer and reader `…GREEN` (`protected=SQLCipher4.19.0`, `protected_restart=independent`, `wrong_key=unchanged`) |
| Downgrade probe (CP14) | `policy-downgrade/report.json`: old dylib `d79c4339…` opens the control, `old_discarded.open.ok=false` kind `BackendFailure`, `sealed.sqlite` hash equal before/after, no WAL/SHM; new dylib `0d3d8c32…` reopens retained row/state |
| Release tooling | 22 tests OK; `compatibility.py sources` exit 0 |
| Tools/SDK stability | `tools-final-verification.json` 10 tools + 7,340 SDK files unchanged; `tools-comparison.json` symlink-vs-versioned SDK path spelling only, resolved path equal |
| Runtime freeze | `freeze-recapture.json` 187 parent inputs all match; only `ledger_txn.rs` changed since the pre-warning capture (DEF-0063) |

## Findings

No defects. Scope/wording notes:

1. **INFO — verification record status field.** `review_status` still reads "submitted for
   independent Fable review; not yet received". Update after this review lands (doc edit, root).
2. **INFO — probe dylib provenance.** The CP14 "new" binary is
   `crates/hoyadb-native/target/release/libhoyadb_native.dylib` (`0d3d8c32…`), produced by the
   native crate build on the frozen source, not an xcframework member. The record states the hash;
   it would help to state explicitly that the probe exercises the native dylib rather than a
   packaged archive.
3. **INFO — first package index.** The runtime-tested package embeds index `354fc720…`, whose
   provider pointer referenced the live `native-build.json` at the time. That pointer became stale
   only because the warm rebuild rewrote the live file; the retained-parent correction fixes the
   final package and `retained-parent-reference.json` documents both hashes. Not a defect; worth
   keeping in the limits text so nobody re-derives provenance from the first package's index.

Unsupported-platform statements (iOS build/export only, no macOS arm64, no publication, no R11)
are consistent with the evidence; nothing in the logs claims more.

## Limits

Nothing rebuilt or rerun by me. CoreSimulator diagnostics in xcodebuild output were read, not
investigated beyond the recorded success line.

## Correction (appended 2026-10-31, after root's check)

The "Current generated output" bullet decomposes the 39 framework members as "10 native + 28 notice
+ Info.plist". Wrong split. Verified against `final-native-restaging.json` (`native_members`, 10
entries, Info.plist among them) and `crates/hoyadb-native/tools/apple_notices.py:365-368`
(`expected = native ∪ {PARENT_COPY} ∪ notices`): the count is 10 native members (including
`Info.plist`, three archives, three headers, three module maps) + 28 notice members +
`native-build-parent.json` = 39. Hash bindings and verdict unchanged.
