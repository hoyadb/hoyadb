# Phase 7 implementation checkpoint

Phase 7 / implementation Stage 9 is **in progress**. This is an evidence index,
not a support or completion claim. Preparation is committed in SPEC-0033,
API-0011, TC-0036/0037 and the RFD-0006 dependency packet.

The current shared facade uses the existing Rust core, real memory adapter and
protected SQLCipher adapter. It accepts caller schemas and typed requests; no
fixed spike schema is used. Focused tests exercise typed values, arbitrary CRUD,
queries/relations, command staging, receipts/reconciliation and retained-memory
migration. Response budgets include complete before images before mutation.
The initial large-before-image regression demonstrated a committed write hidden
behind a reply-size error; preflight now preserves the original row.

The WebAssembly boundary has executed real retained-memory CRUD and Unicode
round trips after explicit preload, with network, OPFS and entropy access made
to throw. JavaScript consumer tests also exercise real WASM schemas, migrations,
relations, observations and awaited commands. A real dedicated-worker protected
open now reports SQLCipher4.19.0 and unsupported browser backup control. These
were development artifacts. Ordinary and instrumented optimized release assets
have since built, and the real retained-memory smoke test passed against the
ordinary release. The [JavaScript lane checkpoint](phase-seven-web.md) now records final installed
consumer/browser conformance and security reruns on matching release hashes. The fixed `hoyadb-v1` OPFS pool
permits one active worker per origin, including across different logical stores.

Native C ABI and Swift wrappers are being validated independently. Initial real
Swift checks cover caller schemas, protected reopen/wrong-key byte preservation,
backup-flag readback and path/key rejection. A Foundation path-alias regression
was reproduced and corrected using POSIX canonical paths before backup changes.
No Android or React Native runtime support is claimed from desktop evidence.

Observed host checkpoint: `cargo test --workspace --offline --locked` passed372
Rust tests across37 result records. The two subsequent facade receipt/index
regressions are included in a separate final21-test facade pass; these totals
are not disjoint. Workspace all-target Clippy, root and browser formatting,
WASM all-feature Clippy, traceability (0violations) and current RTM passed.
Detailed RED/GREEN qualifications and original log hashes are preserved in
[facade TDD evidence](phase-seven-facade-tdd.json) and
[core seam evidence](phase-seven-core-seam-tdd.json). The external Rust package
identities in all three new/updated locks remain a subset of the approved Phase6
graph ([activation audit](phase-seven-dependency-activation-audit.json)).

The later [Claude facade checkpoint](phase-seven-facade-review.md) identified
batch-preflight over-rejection and a terminal-prefix boundary hypothesis. Both
now have actual public reproductions and host fixes; all 28 facade tests pass.
The [resolution evidence](phase-seven-reply-budget-resolution.json) preserves
the RED/GREEN logs and incomplete-run qualifications. The separately completed
374-test full workspace run predates these fixes. The matching rebuilt JavaScript packages passed the final real memory/OPFS
reply-budget cases. The [current checkpoint](phase-seven-web.md) and separately
retained [historical report](phase-seven-web-pre-review.md) distinguish these
artifacts; final cross-platform review remains required.

Claude Opus's [entropy review](phase-seven-android-entropy-review.md) passed
before Android provider activation; its scope is source/fault/syscall checks,
not the final Phase7 review. Subsequent native-lane evidence reports an actual
isolated16KiB Android emulator provider/C ABI pass; Kotlin/RN support is still
unclaimed pending actual consumer execution; ADR-0035 now records graph approval.

## Open gates

Current continuation checkpoints (each has its own source cut and limits):

- [Native C/JNI foundation](phase-seven-native-foundation/README.md) and
  [focused Claude lifecycle review](phase-seven-native-lifecycle-review.md)
  cover the checked binary-key boundary, readonly preflight and shared release
  foundation. The review excludes the full mobile SDK and benchmark gates.
- [JavaScript external-receipt correction](phase-seven-web-receipt-wake.md)
  records84 Node tests, installed memory/walkthrough/guide checks and36 protected
  browser groups across reload, plus9 security and1 initializer-retry cases.
  DEF-0015 is fixed at69f43d7. The older83-test report remains historical.
- [Swift public testing/lifecycle checkpoint](phase-seven-swift-public-testing/README.md)
  records44 passing tests, separate strict formatting exit0 and independent
  public macOS writer/reader processes. DEF-0017/0018 fixes and helpers are
  committed16c15b8/cf3f03a. The older35-test/lint-failed cut is preserved.
  The later [current-source iOS run](phase-seven-swift-ios-current/README.md)
  passes a clean outside-consumer build and independent writer/reader on the
  owned x86_64 iOS simulator. Root rechecked all40 recorded source hashes and
  the copied runtime summary/commands/stdout against the original capture.
  Arm64 simulator/device slices remain build-only; the earlier timeout causes
  remain unknown.
- [Kotlin's final local artifact](phase-seven-kotlin-final.md) supersedes the
  frozen development handoff:50 full-D8 tests,8 full-versus-split controls and
  the independent23-group APK matrix pass on the final source cut. Its AAR is
  `f2e8a16f…`; root rehashed all five final package/certificate files,28 source
  inputs,12 packaging-tool inputs and five native provenance inputs without
  mismatch. The installed-app base-directory alias failure and its regression
  are DEF-0024. Runtime covers x86_64/API37/16KiB pages; arm64 remains build-only.
  The [independent Android SDK/package review](phase-seven-android-review.json)
  passes against the exact artifact/source hashes and50-test/23-case evidence;
  RN Android consumption remains a separate gate. Git storage must also preserve
  upstream notice and raw evidence bytes: DEF-0027 is corrected and reviewed in
  5b7ac5d, and the matching SDK milestone is committed1352608. All225 staged
  milestone blobs are byte-exact; preserved vendor/raw formatting is reported
  separately from clean authored-file whitespace checks.
- [Native cold/resource measurements](phase-seven-benchmarks/cold-resources.md)
  are complete for their frozen Rust-facade classes. The [warm attempt](phase-seven-benchmarks/full-20261024-warm-interrupted/README.md)
  was explicitly interrupted and failed:145 completed captures are unchanged,
  the next10k reset worker's partial samples were unavailable, and owned cleanup
  was verified. No full warm baseline or numerical budget is accepted.
  Browser correctness/calibration is distinct from performance measurements.
  Reviewed between-worker admission now precedes the next full measurement run.
  The later [functional benchmark checkpoint](phase-seven-benchmarks/functional-20261025-benchmark-lane/README.md)
  records75 JavaScript/75 Python checks and17 installed-package browser steps;
  its observations are contaminated or measurement-disabled, with no accepted
  baseline. A bounded10k reset diagnostic did not finish its first sample within
  1200 seconds on a heavily loaded host. The [read-only core investigation](phase-seven-core-scaling/README.md)
  traces quadratic work across fixed-size bootstrap pages under the existing
  full-ledger atomic contracts, plus redundant constant-factor copying/parsing.
  This is source-derived analysis, with no profiling, runtime forecast or
  production change. Broader contract or storage-format changes are unapproved.
  Reviewed page permit transport416fffb precedes its implementation; accepted
  measurements still require a coordinated quiet window. The operator selected
  that window after RN Android is complete; no accepted baseline run is admitted
  before that handback.
  A later [five-worker native point pilot](phase-seven-benchmarks/pilot-20261025-native-memory-c00/README.md)
  completed with exact one-use admission and unchanged sources/binary. It is
  explicitly contaminated, with no accepted summary/baseline/budget. Root
  checked all23 integrity entries and all12 primary capture copies; sanitized
  environment views carry explicit derivation hashes and preserve local originals.
  Its [later functional checkpoint](phase-seven-benchmarks/permit-transport-20261025/README.md)
  retains the N01–N08 RED/GREEN sequence and actual contaminated launcher run.
  Root rehashed all41 lane files with no mismatch. The [focused Claude review](phase-seven-permit-review.json)
  completed using a compatible Opus fallback after the Opus5.5 tool-version
  error. Its waiter transition issue DEF-0023 is now reproduced and corrected:
  root reran90 Python/79 JavaScript checks and rehashed all41 files successfully.
  The [focused closure review](phase-seven-permit-resolution-review.json) passes;
  [resolution evidence](phase-seven-benchmarks/def-0023-resolution-20261025/README.md)
  keeps the original failures and contaminated integration separate. The final
  independent matching-artifact review remains open.
- The packed portable RN declaration check reproduced a DOM-only AbortSignal
  dependency in the shared host declaration. DEF-0026 preparationabdf25c precedes
  the [declaration-only correction](phase-seven-portable-declarations/README.md)
  committed82cffb8. Strict packed ES2022 host/testing/RN and DOM probes pass;
  the unchanged runtime suite passes84 tests. Root compares all105 packed files:
  only client/index.d.ts differs, with all100 non-declaration files unchanged.
  The new client tarball is `fb710c70…`; earlier runtime packets and footprints
  keep their recorded package identities until explicit input alignment.
  The [exact Kotlin compiler proposal](phase-seven-kotlin-compiler-addendum/README.md)
  is frozen at5e771f2. Root rehashes all123 index entries and reruns177 explicit
  checks under optimized Python without changing the packet. Initial review
  corrections and embedded risks are recorded. The [final independent review](phase-seven-kotlin-compiler-final-review.json)
  is ready for an operator risk decision with no blocker; the [decision checkpoint](phase-seven-kotlin-compiler-decision-status.json)
  pins the exact committed packet and remaining restrictions. The operator's
  [subsequent approval](phase-seven-kotlin-compiler-approval.json) authorizes
  exact build input use and requests security fixes. Bounded RN Android graph
  validation and separate security triage are assigned; compilation remains
  held for the reviewed security handback. These
  gates do not supersede the unchanged
  core/runtime dependency pins or the accepted restricted-build policy.
- The [exact Android plugin addendum](phase-seven-plugin-addendum/README.md)
  is reviewed and explicitly approved at8a87389, freezing snapshot03048bb.
  It adds six build coordinates and embedded Gson with exact hashes and the
  original restricted build policy; app/SDK runtime pins are unchanged.
  The separate lazy formatter remains excluded. Activation still needs actual
  selected-graph/hash checks and the final official consumer execution.

- ADR-0035 records the operator's explicit exact-bundle/restricted-build approval
  and command-ID clarification. All 86 original review files were verified
  against dc5469e before the [approval record](phase-seven-dependencies/approval.json)
  was committed. Activation still requires archive and actual selected-graph
  verification; the disclosed 47 build-only advisory matches are not patched.
- Extend the approved TC-0016 exact-ID scenarios to the remaining mobile
  consumers. Real WASM memory and protected OPFS already pass all16 original
  walkthrough groups with the explicitly approved two exact IDs.
- Complete shared facade/host async, entity/passive-merge and lifecycle matrix;
  real protected browser byte-fault and restart controls; final broad regressions.
- [RN iOS Hermes execution](phase-seven-rn-ios/README.md) passes51 cases plus
  independent protected writer/reader on the owned x86_64 simulator. Public
  factory forwarding corrections are DEF-0025. Debug prebuilt containers were
  used; no arm64/device or release runtime claim follows. RN Android execution,
  final package compatibility snapshots and final cross-review remain open.
- [Native packaging and hermetic Pod installation](phase-seven-native-packaging/README.md)
  record the uniform XCFramework basename correction and a fresh local-source
  CocoaPods install with socket networking denied. Root reran the four
  XCFramework checks successfully. All76 podspecs and the three prebuilt trees
  match the reviewed inputs; the lock changes only its Podfile checksum.
  Earlier CDN metadata contact and the failed preserved-Pods layout attempt
  remain visible. DNS over system IPC and Xcode's external build service are
  outside the demonstrated socket-denial boundary. RN compilation/runtime
  remains open; installation alone is not SDK acceptance.
- Optimized measured TC-0037 baselines, reviewed derived numeric budgets and
  failing regression controls; the [historical point-query pilot](phase-seven-benchmarks/README.md) is
  measured, while the full baseline and budgets remain open.
- Final Claude Opus cross-review, licenses/source manifest, traceability,
  developer guides and completion report on matching artifacts.

The operator requested more implementation delegation to conserve Codex quota.
[Three Opus5.5 implementation lanes](phase-seven-opus-delegation.json) now own
benchmarks, Kotlin packaging/APK, and Swift/native/RN respectively, with explicit
file boundaries, handoff hashes and acceptance checks. Codex predecessors are
frozen. The separately configured Opus5.5 final reviewer has no implementation
ownership. Root retains integration, selective Git and acceptance decisions.
Workspace formatting, all-target Clippy, core/memory WASM build and trace/RTM
checks pass in this continuation. The [new broad workspace run](phase-seven-workspace-verification/README.md)
passes381 tests with unchanged source hashes; platform/package gates remain open.

Existing formatting/CI maintenance and `.agents/` work are preserved separately.
No user stores were converted and no package was published, pushed or tagged.
