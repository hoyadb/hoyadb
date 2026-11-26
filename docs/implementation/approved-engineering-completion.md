# Approved engineering follow-up — 2026-10-31

Status: the approved conflict-policy, compatibility and SPEC-0037 R9–R10 work
is implemented, cross-reviewed and verified within the scopes recorded below.
R11 and the listed decisions and platform gates remain open; SPEC-0047 is not
claimed complete in full.

## Scope and preservation

The operator requested all remaining fixable work after the
[2026-10-30 remediation](review-remediation.md). HEAD at this follow-up's start
was `035bd0f180ca73c017813f03c454d4ad85936927`; the existing working tree and index
contained substantial prior work. The exact baseline, 3,289 source files, Git
status and tracked/index diffs were captured under
`/tmp/hoyadb-followup-20261031/` before this task's edits. Task-only changes are
derived against that capture, not attributed wholesale from the dirty HEAD diff.
No commit, push, publication or user-store conversion is part of this work.

The initial remaining-work assessment needed two corrections. ADR-0035 already
approved the exact prefixed command ID and unprefixed ordinary CRUD ID; DEF-0057
corrects stale active wording while preserving the accepted decision and
historical question. SPEC-0040's Stop/Discard behavior was missing in the core,
facade and all SDKs, not just Kotlin. That approved foundation was completed
before rewriting the same planners for SPEC-0037 Phase 3.

## Conflict-policy checkpoint

The core/facade implementation adds owner-local Stop/Discard configuration,
truthful Discarded recovery records, explicit discard and resolving operations.
Policy is consulted only at the permitted inverse-proof failure after the
existing protection, mapping, guard and C4 checks. The complete report is bounded
before commit, including unit-returning calls. Normal history trimming may
remove a just-discarded record in that commit; the report describes its transition.
Memory and SQLite share the same planner and refusal tests.

JavaScript now configures a private acquired owner before publishing it, retains
an unpublished handle after failed cleanup for explicit close retry, and uses
resolving operations from existing unit APIs and managed sync. Reports are
immutable, exact-key decoded, and preserve full-u64 decimal strings. KeptCurrent
invokes `onResolution` once in the current epoch; exceptions and rejected
thenables become redacted callback facts without changing the commit or removing
the callback. Application and React Native hosts use this shared implementation.

TDD evidence includes `js-policy-red.log` (10 behavioral failures),
`js-policy-normalization-red.log` (explicit null map must not become an empty
map), and `js-policy-shape-red.log` (malformed policy must not acquire an owner).
The old-WASM unsupported-operation run is retained as an integration prerequisite
failure, not counted as behavioral RED. Fresh ordinary WASM inputs/artifacts are
bound in `wasm-policy-manifest.json`; its source hashes matched before and after
the confirming Cargo/bindgen invocation.

Fable's final core review matched all 62 frozen source inputs and found no
remaining actionable defect. Its history-trim, unsigned-wire legend and
defensive descendant-test notes are addressed in the active contracts and
tests. The exact-current native downgrade probe
(`policy-downgrade-final/report.json`) shows the earlier binary opening the
control store, refusing Discarded records with `BackendFailure`, and leaving
the main-file hash unchanged with no WAL/SHM present before or after. The new
binary reopens the store with the retained row and Discarded state.

Fable also reviewed JavaScript and the compatibility checker, ran 40 focused
host cases and 20 checker cases, and requested an additional real-WASM
Busy-policy control. That control passes. The final JavaScript workspace run
passed 207 client and 11 testing cases; the React Native host run passed 10.
Four policy cases pass against the actual WASM build. The final package build,
package-tree check and independent packed consumer all exited zero; the
consumer exercises TypeScript declarations and actual WASM/core memory.
These results are retained in the `js-workspace-policy-final.log`,
`rn-host-policy.log`, `js-policy-wasm-busy.log` and
`js-package-*-final.log` files. Fable's final JavaScript re-review matched the
13-input freeze and actual WASM bytes, reran 38 focused cases, and found no
defects.

The Swift policy checkpoint passed 102 native host tests with warnings as
errors against archive `b92e3f27…20fa1`; 66 native inputs and the Swift input
inventory are bound in `swift-host-current/verification.json`. Its first native
run exposed a new fixture's missing empty reconciliation after a rejected
receipt; the fixture was corrected to follow the contract, with the failed run
retained. The Kotlin checkpoint reproduced 12 behavioral failures before its
fix, then passed 17 controlled-runtime groups and four actual JNI policy groups
on the owned API-37 x86_64/16-KiB emulator. Fable found no actionable defect in
either SDK's frozen policy implementation. Later admission-fixture corrections
and package execution receive separate source-bound evidence.

## Compatibility checkpoint

DEF-0056 adds a public source-drift gate, fresh Swift module/API comparison, and
actual native export checks. Historical snapshots remain immutable. Explicitly
approved changes are recorded separately; tool failure, empty/malformed evidence
or an unapproved digester diagnostic fails the check even if the digester exits
zero. The historical Swift initializer is restored as an overload. The source
gate passed against the reviewed current manifest (`compat-sources-root.json`).
The source gate is included in GitLab's existing design job; this is a local
configuration change, not evidence that GitLab executed it.

Fresh Apple compilation completed all four supported target architectures with
67 unchanged source inputs. The notice finalizer correctly refused the old
archive-bound provider evidence. Actual new per-architecture symbol inspection
then refreshed only the current provider bindings; the old index and fresh
derivation are retained in [the evidence directory](approved-engineering-evidence/README.md).
The unmodified finalizer, five ordinary artifact checks and all three packaged
slice export gates passed. This closes the prior missing-packaging limitation
for the retained policy checkpoint. It does not certify later source changes.

## Facade forwarding correction

DEF-0058 records an additional omission found while preparing the adapter work:
the facade Engine dropped the validated query proof through the trait default.
Two public-facade regressions first failed on excess memory visits and SQLite
sorting. Forwarding the existing proof to the concrete adapter makes both pass
at N=10 and N=1000; all 21 existing general facade cases also pass. This changes
neither query results nor the raw-query validation path. Fable reviewed the
forwarding change and both regressions without an actionable finding. Final
platform evidence is recorded separately below.

## Phase 3 preparation

Both the internal prerequisite review and Claude Fable 5.1 reviewed SPEC-0037's
R9–R11 cases before conversion. TC-0041 now specifies the dual-fault entry points,
test-only first-image/cache/epoch observations, same-adapter counter comparisons,
static proof-site negative control, stricter stale-session proof and marker
roundtrips. TC-0044 joins the unchanged-behavior baseline. Counter completion and
SI-Y06 characterization preceded LedgerTxn conversion. At that checkpoint,
full delta persistence, format 3 and the offline converter were later gates.

The immutable policy checkpoint completed `cargo test --workspace --release
--offline --locked`: 736 tests passed, none failed or ignored, including the
actual S05/S07 process-interruption recovery cases. All 3,296 captured inputs
were unchanged before and after execution. This is the pre-conversion behavior
pin in `workspace-policy-baseline.json`, not a claim about later runtime edits.

The first Phase 3 packet completed scoped packet/snapshot/nested-undo clone
instrumentation and six validation-order characterizations. Its existing exact
counter controls, Clippy and formatting pass; four compiled behavioral REDs per
adapter establish missing first-image, epoch, cache and bounded-copy behavior.
Fable cleared that packet for R9 conversion. Separate Engine regressions also
reproduce 64 versus 5,014 parsed packets for the same patch at M=10 and M=1,000;
they provide the negative control for the later facade-forwarding verification.

The R10 test-only packet has four passing legacy controls and ten compiled REDs
for missing format 3. Fable found its acceptance cases adequate. The R11 probe
proves unchanged source inventories and no WAL/SHM access for the specified
read-only opener. Wrong-key classification with existing WAL sidecars requires
a post-refusal whole-file SQLCipher integrity check. Its resource cost and the
explicit exit-code classification remain a pending user decision; no converter
implementation or contract relaxation is claimed.

The delta proof and provided conservative adapter method now have behavioral
RED/GREEN evidence. Fable's review found an overly permissive entity revision
rule and reconfiguration branch; four new cases failed before correction and
the resulting 16-case validator suite passes. The validator requires the exact
revision increment and refuses configuration changes, which retain the full
inventory path. Both adapters pass six genuine-proof race/fallback cases,
including a well-formed untouched edit between the default method's full load
and apply. External compile-fail examples preserve proof opacity.

The first bounded command/entity planner checkpoint passes all four SI-Y01
memory cases. Engine now forwards the proof to the concrete adapters, and both
Engine patch-scaling cases pass at M=10 and M=1,000 against 177 unchanged inputs
(`engine-delta-first-green.json`). At that checkpoint reconcile, pull and merge
conversion was still in progress; those early results do not establish their
bounds. The actual proof-site source
check and its 20 negative-control tests pass locally, and the same commands are
included in CI's quality-tooling job. GitLab execution is still unverified.

Removing only Engine's delta forward in a separate captured tree makes both
cases fail again (112 versus 7,042 parsed packets and 90 versus 6,030 SQLite
writes). Fable verified that one-file/seven-line negative control and the two
177-input manifests, and approved the forwarding change for this checkpoint.
The compiled planner snapshot is bound in the manifest. Both forwarding cases
also pass in the final integrated workspace.

DEF-0060 closes a build-evidence omission found during final rebuild preparation.
The Apple builder now captures root Cargo configuration, conventional build
scripts, source resources and its own bytes. Five fixture groups reproduce
eight old assertion failures and now pass with the four existing packaging
groups. Fable independently reran all nine groups and approved the stated
five-crate inventory scope; external inputs still require separate capture.
No historical artifact certificate was rewritten or recertified by these tests.

The integrated R9 planner checkpoint now passes 248 focused cases plus Clippy,
formatting and the proof-site audit on 179 unchanged inputs. The separate
pull/merge checkpoint passes 85 cases. `phase3-core-r9-packet.json` and
`r9-pull-final-work/packet.json` bind those sources and results. Independent
read-only review found no actionable issue in the first-image, prior/staged,
commit and format-2 rank boundaries. Fable's final core and pull/merge reviews
found no confirmed semantic defect. Two initial core observations were withdrawn
after checking the existing nested-undo counter and large-operand query path;
the corrections are appended to the retained review rather than replacing it.
The final candidate-copy counter supplement passes 226 cases with Clippy and
formatting on 179 unchanged inputs (`r9-candidate-green-work/packet.json`).
Its three behavioral REDs identify actual internal row copies; the public
`merge_input` boundary remains uncounted. Fable found no actionable issue.

Fable's cancellation review also exposed DEF-0061: reactive publication moves
all groups into a local vector across an await, so cancelling a suspended
post-commit read drops subscriptions. The existing SI-Y01 test proves committed
owner/store state, not subscription preservation. Three focused regressions now
pass after a publication guard restores subscriptions on cancellation, retains
already queued delivery, and invalidates unfinished count work. The 26 affected
cases, scoped Clippy, formatting and traceability pass against the 180-input
`reactive-cancellation/final-source.json` checkpoint. Fable reviewed the guard
and fault placement without an actionable finding. Its pre-persist Pending is
snapshot acquisition, not ledger rollback
across an await; ledger staging and persistence are synchronous. The epoch
omission mutant's first case fails its epoch assertion, while its second case
fails the premature-Ready publication barrier. The original source/log records
remain unchanged and these distinctions are retained in
`ledger-cancellation-review-addendum.md`.

The first integrated R9 `cargo test --workspace --offline --locked` stopped at
the SQLite terminal-correction fault fixture. DEF-0062 records why its old
sync-field INSERT trigger no longer fires when a delta only deletes a trimmed
command. The repaired trigger targets that deletion and requires the mapped
application row to exist first, proving rollback after writes. Both the original
bundled case and the focused regression pass; exact error, queue, epoch, retry
and strict-reopen assertions remain. Fable accepted the repair. All 27 SQLite
integration targets that the initial failure prevented from running also pass
in a separate explicit-target run. These results are retained as a failed full
gate plus scoped follow-up evidence. The subsequent combined workspace gate
passed as recorded below.

## Format-3 implementation

The reviewed R10 change is now imported. New entity ledgers select format 3;
existing format-2 ledgers retain their encoding through all supported lifecycle
operations. Identity metadata packets encode a fixed-width table byte-length
prefix followed by table and record-id bytes in lowercase hex. Validation checks
the raw bound, alphabet, length and payload agreement without allocating decoded
bytes. Full and delta storage plans reject implicit marker changes. Format-3
packet updates touch only changed identities; the legacy ordinal suffix path
and its exact assertions remain covered by independently seeded format-2 data.

The codec/owner/storage packet passed 477 focused cases across its unchanged
source captures, with scoped Clippy and formatting. Behavioral REDs precede
the codec, bounded-work and marker-retention changes; the storage flip tests
first establish valid same-format controls. Fable's complete runtime/API review
found no actionable defect. The ten-file import used exact before/after hashes,
and all 187 integrated-workspace inputs matched the working tree at import.
The later two-attribute DEF-0063 correction is bounded separately below.
Evidence is `r10-codec-work/final-packet.json`, `fable-r10-runtime-review.md`
and `r10-import-result.json` under the task evidence directory.

API-0009 documents the approved `EntityLedger.ledger_format: u8` representation,
restricted to 2 or 3. The C/JS/RN source compatibility gate does not cover Rust
recovery structs; its retained manifest remains unchanged and passing. The Rust
addition was reviewed explicitly against SPEC-0037. No online conversion API or
store converter is introduced by this packet. Fresh platform results are
recorded below; older platform checkpoints above remain historical.

The first ordinary WASM rebuild exposed DEF-0063: two observation helpers were
compiled without their test/counter-only callers. A production build with
warnings denied reproduced the dead-code failure. The correction adds exactly
two conditional-compilation attributes; it passes the ordinary core/memory
WASM build with warnings denied, all 219 core cases, Clippy and formatting.
Fable found no issue in the caller/configuration boundary. The final 187-input
runtime map is `r10-final-runtime-source.json`; the only difference from the
integrated workspace snapshot is this two-line compile-time correction. The
workspace's test configurations already enable both helpers, so their behavior
is unchanged. Platform attempts before that correction are retained separately
and final artifacts are rebuilt against the refreshed source.

## Verification and remaining work

The integrated Rust pipeline passed all five commands: workspace tests,
workspace formatting, workspace all-target Clippy with warnings denied, ordinary
core/memory WASM and SQLite WASM library builds. The workspace has 882 passing
tests, none failed or ignored, across 89 outer Cargo suites. An intentionally
failing supervised child is retained in the JSON resource-limit test log; its
parent test passes. All 187 inputs stayed unchanged during this pipeline.
`r10-integrated-checks/packet.json` binds the commands, logs, source maps and 85
test executables. Its only difference from final runtime source is the two
DEF-0063 attributes described above; all 219 core checks and the production
WASM build were repeated on that final source. No full-workspace rerun after
those two attributes is claimed. Both integrated WASM builds retain the
pre-correction dead-code warning in their logs; the warning-denied production
proof is the separate final-source build. Fable independently recounted the
workspace log and verified all source, evidence and executable hashes without
an actionable finding.

The final ordinary web build, actual Node/WASM bridge, 207 client tests,
11 testing-package tests, ten React Native host tests, package build/check and
independent offline packed consumer passed. The consumer ran three strict
TypeScript checks, including the no-DOM surface, and actual WASM execution.
All 5,359 captured inputs (511 repository and 4,848 dependency files) stayed
unchanged through all eight stages; both pack-time manifests match all 76
source inputs and their shipped artifacts. Fable verified this linkage and
the recorded stage results without findings. This establishes ordinary WASM
and Node behavior, not mobile or browser OPFS execution. The earlier attempt's
npm-cache permission failure remains recorded; the unchanged workflow passed
with a new task-owned cache, without a permission or dependency change.

Final Android verification freshly built both native ABIs against the final
77-input runtime capture. All 103 Kotlin cases across 24 entrypoints passed
using the actual final x86 JNI on the owned API-37 emulator with 16-KiB pages.
Production AAR packaging passed its unchanged source, notice, ELF, export and
archive gates. An outside-checkout consumer built from that AAR passed all 25
groups, including a separate-process restart, and uninstalled its own app.
The 53 test and 30 production SDK input maps and native inputs stayed unchanged.
The standalone JNI and packaged JNI have distinct recorded link outputs;
each runtime run is bound to the artifact it actually used. ARM64 is build/ELF
evidence only. This is manual SDK/AAR/APK evidence, not an AGP or React Native
consumer certification. The complete record is `kotlin-r10/verification.md`.
Fable checked all 125 packet artifacts, the device hash/push trace, independent
case totals and consumer linkage without an actionable finding.

The unchanged public application browser runner also passed against the final
ordinary WASM in installed Chrome 156. It executes the Todo tutorial in a real
worker, including rollback, sync and ID mapping, then checks missing-key
rejection and caller-key encrypted OPFS create/write/close/reopen. All 112 copied
inputs and the final 187 runtime inputs remained unchanged. The first attempt
failed the runner's five-second Chrome startup assertion before page execution;
a separate diagnostic reached readiness after 6.32 seconds, and the unchanged
retry passed. Neither assertions nor timeouts changed. This is one macOS Chrome
checkpoint, not cross-browser, reload, tamper or concurrent-worker certification.
Those broader suites require a fresh test-controls artifact and were not run.

Fresh Apple compilation built macOS x86_64, iOS arm64 and both simulator
architectures. The first normal build correctly rejected stale provider
evidence; fresh per-architecture inspection and retained provenance allowed the
unchanged finalizer and all five artifact checks to pass. All three slice export
checks and the fresh Swift API comparison passed. The refreshed normal builder
also completed successfully. Its reordered XCFramework metadata changed the
parent/package record, while native archives stayed byte-identical; the first
runtime-tested package and final package are retained separately.

The actual packaged Swift SDK passed 102 tests with warnings treated as errors.
An isolated public consumer compiled and passed independent writer and reader
processes against a new protected fixture. The native bridge passed nine tests,
Clippy and formatting. The real old/new native-dylib probe confirms that the old
binary opens its control store, rejects Discarded recovery without modifying
the store, and the final new binary reopens the retained row and state.
Restricted attempts that failed Swift cache access or backup-policy application
remain recorded; unchanged code passed under normal host access, with no
protection check disabled. These runs establish x86_64 macOS execution; iOS and
simulator slices are build evidence only, with no arm64 macOS claim.
The downgrade probe uses the separately built native dylib, not an XCFramework
member. The first runtime package's provider pointer referred to the then-current
generated parent; use the retained derivation and final package's immutable
parent pointer for later provenance checks. Fable verified Apple and browser
source/artifact linkage and the recorded results without actionable findings.

Compact final records and implementation reviews are retained in
[the R9/R10 evidence index](approved-engineering-evidence/final-r9-r10/README.md).
The [Apple record](approved-engineering-evidence/apple-r9-r10-verification-20261031.json)
binds both packaged checkpoints and the captured source/tool inventories;
[its explanation](approved-engineering-evidence/README.md) preserves the earlier
policy checkpoint separately. Raw outputs, failed attempts, mutants and full
dependency inventories remain in the task evidence directories named above.

Public tooling passes 9 packaging, 52 quality/proof-tool, 13 traceability and
22 compatibility tests.
The source compatibility gate, proof-site audit over 40 inputs and formatting
checks pass. Final traceability reports zero violations, the requirements matrix
is up to date, and `git diff --check` passes. The preservation audit confirms
unchanged HEAD, staged contents,
38 accepted ADRs, historical API snapshots and all baseline implementation
evidence. It does not claim raw index-byte preservation because no raw index
baseline was captured. All 187 final runtime inputs remain unchanged. The owned
Android emulator was stopped after identity/start-record verification; browser
profiles, servers and the consumer installation were cleaned up.

Remaining decisions and execution limits:

- R11's offline converter is now implemented as explicit native tooling after
  the operator answered "Allow the full-file check" (DECISIONS.md, 2026-10-31):
  a keyed WAL refusal is classified on the same retained read-only connection
  with SQLCipher's whole-file integrity check, whose reads and error-row memory
  grow with file size. Its source-bound RED/GREEN evidence, remaining limits and
  the implementer's own review are in
  [the converter report](offline-ledger-converter.md). No user store was
  converted; existing format-2 stores remain in their original format until an
  operator runs the tool against a closed store with a verified backup.
- An unexpected factory startup failure after owner acquisition, combined with
  failed cleanup, still lacks a caller-visible cleanup handle. Direct Database
  instances retain retryable cleanup. Changing the factory API or adding an
  automatic retry policy requires a decision; neither is invented here.
- The full React Native Android consumer remains gated by the recorded security,
  native-lifetime assurance, advisory/baseline and HEAD-pack decisions. Host,
  Codegen and Java checks do not substitute for that execution.
- License, security-reporting channel, release policy, registry ownership,
  supported-target claims and remote Apple delivery remain publication decisions.
  Packages remain unpublished. GitLab configuration was checked locally but not
  executed remotely. Build-only architectures are not device certifications.
