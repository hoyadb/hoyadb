# Final R9/R10 verification records

Final disposition: Claude Fable 5.1 found no actionable defects in the implemented
packets or final workspace, web, browser, Android and Apple evidence. Each review
states what the reviewer ran, inspected or independently hashed. Historical
at-capture status fields remain unchanged; the final reviews below supersede
their pending-review status without changing the reviewed packet hashes.

These records supplement the [engineering report](../../approved-engineering-completion.md).
They retain final source identities, command results, artifact identities and
independent review conclusions. Raw build/test logs and complete dependency
inventories remain under `/tmp/hoyadb-followup-20261031/`; Apple records use
`/tmp/hoyadb-apple-r9-r10-final/`. The paths identify this local execution, not
portable prerequisites. Historical records in the parent directory are unchanged.

- [Final runtime sources](final-runtime-sources.json): 187 captured inputs,
  SHA256 `8549dc12cf14db7e5ffb47bb0687e799767f29c6350ed5aad0eac5d8f13c1818`.
- [Workspace packet](workspace-packet.json): 882 tests, no failures or ignored
  tests; formatting, Clippy and both WASM builds passed. The workspace snapshot
  precedes exactly two diagnostic `cfg` attributes. The
  [separate review](diagnostic-cfg-review.md) records the final-source 219 core
  checks and ordinary production WASM build with warnings denied, including
  the correction to an earlier claim about all-target feature coverage.
  The [workspace review](workspace-review.md) independently verifies the counts,
  retained pre-correction warnings and exact source boundary.
- [Web packet](web-packet.json) and [Fable review](web-review.md): fresh ordinary
  WASM, 218 JavaScript tests, ten React Native host tests, package checks and
  an independent offline packed consumer. Node execution alone is not a browser
  or mobile certification.
- [Android packet](android-packet.json) and [verification](android-verification.md):
  two fresh native ABI builds, 103 Kotlin cases and 25 packaged-consumer groups,
  including separate-process restart, on x86_64 API 37 with 16-KiB pages.
  ARM64 is build/ELF-only; no AGP or React Native consumer claim.
  The [Android review](android-review.md) verifies the complete artifact chain.
  The [cleanup addendum](android-emulator-cleanup.md) records stopping only the
  verified task-owned emulator after all checks; its original packet remains
  unchanged.
- [Ordinary browser packet](browser-packet.json) and
  [verification](browser-verification.md): actual Chrome application tutorial
  and caller-key encrypted OPFS write/reopen using the final ordinary WASM.
  The initial startup timeout and unchanged passing retry are both retained;
  broader reload, tamper and worker-retry matrices were not run. The
  [browser review](browser-review.md) confirms that boundary.
- [Apple review](apple-review.md): final source/provider/package linkage,
  102 packaged Swift tests, nine native tests, consumer writer/reader, API/export
  checks and the separately built native-dylib downgrade probe. The
  [submitted record](../apple-r9-r10-verification-20261031.json) retains its
  historical pending-review field; this review is the final disposition.
- [Earlier Engine and Apple inventory review](engine-delta-apple-inventory-review.md):
  retained notification text whose evidence location was missing from the
  aggregate review index. The original notification's truncated ending is
  identified explicitly, without reconstructing missing text.

Implementation reviews are also retained: [R9 core](r9-core-review.md),
[pull/merge](r9-pull-merge-review.md), [counter supplement](r9-counter-review.md),
[R10](r10-review.md), [memory rollback](memory-rollback-review.md),
[reactive cancellation](reactive-cancellation-review.md),
[SQLite fault fixture](sqlite-fault-review.md) and
[aggregate coverage audit](aggregate-audit.md). Corrections and later scopes are
appended rather than silently removing earlier findings.

The [preservation audit](preservation-audit.md) checks baseline copies, unchanged
HEAD and staged content, accepted decisions, historical evidence and runtime
source. Its counts are explicitly timestamped before the final evidence copies.
[Final local checks](local-checks.json) retain the current proof-site, source
compatibility and formatting results. [Tooling tests](tooling-tests.json) record
9 packaging, 32 quality, 20 proof-scanner, 13 traceability and 22 compatibility
passes; [documentation checks](documentation-checks.json) record zero violations,
an up-to-date requirements matrix and a clean diff check. The
[earlier JavaScript review](earlier-js-policy-review.md) retains the reviewer's
explicit 40-host/20-checker execution statement, separate from later results.

Each platform packet identifies its own complete source/tool/artifact capture;
the 187-input workspace map alone is not a universal platform provenance claim.
The offline R11 converter and publication choices remain outside these passing
results. No user store was converted and no package was published.
