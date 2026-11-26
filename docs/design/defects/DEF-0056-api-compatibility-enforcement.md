---
id: DEF-0056
title: Native Swift and React Native API snapshots lack enforcement
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-31
updated: 2026-10-31
parents: [SPEC-0047]
affects: [API-0011, NFR-0006]
regression_for: [TC-0051]
tags: [compatibility, tooling, regression]
---

# DEF-0056: API snapshots lack enforcement

API-0011 requires compatibility snapshots before release readiness. The current
native/Swift/RN snapshot README labels the existing files inert baselines, and
no enforcing consumer was found in current CI, tests or SDK tooling. A fresh
Swift source/digester probe also found that comparison can exit zero despite
reported breakages, so exit status alone cannot establish compatibility.

Add dependency-free, fail-closed tooling and meaningful regression tests for
reviewed source surfaces, fresh Swift API diagnostics and actual native exports.
Keep historical evidence unchanged. Explicitly distinguish approved additive
surfaces and conservative drift checks from a semantic compatibility claim.
Do not accept a new baseline merely to conceal a removed API or failing tool.

Exact diagnostics, RED/GREEN commands, source/artifact binding and any required
source compatibility correction will be recorded with the implementation.

## Implementation and regression evidence (2026-10-31)

`tools/release/compatibility.py` now checks retained historical evidence,
reviewed current source drift, freshly compiled Swift API dumps and actual
native-library exports. It rejects tool failures, empty or malformed evidence,
missing/added/changed inputs, changed inputs during capture and unexpected
exports. Candidate generation writes a separate unreviewed file; it cannot
overwrite retained or reviewed baselines. Historical snapshot bytes are not
updated. The source manifest's final cut awaits the in-progress SPEC-0040 SDK
surface review; this section does not claim that pending check passed.

The first meaningful RED was the installed Swift digester reporting
`API breakage:` with exit zero. The exit-status-only helper accepted that result,
and the regression failed with `CompatibilityError not raised`. The correction
checks diagnostic content and rejects unrecognized or empty output. A separate
exact allowance records the already-approved SPEC-0036 enum addition;
SPEC-0036 explicitly describes its source break for exhaustive switches. It
does not allow the removed historical Database initializer. The fresh source
probe still reported that removal before its compatibility repair.

The portable regression command is
`python3 -m unittest discover -s tools/release -p 'test_*.py'` (20 tests passed at
this implementation checkpoint). The suite includes temporary source fixtures,
source/artifact mutation during capture, immutable historical inputs, malformed
digester dumps, diagnostic exceptions and missing/extra exports. A second
behavioral RED reproduced acceptance of malformed symbol-only `nm` output;
another reproduced rejection of LLVM bitcode's valid unknown-address notation.
Both now pass. Earlier 19-test runs also passed under Python `-O`; the final
suite and source/API/artifact runs must be repeated after the final SDK cut.

Actual probes are saved under `/tmp/hoyadb-followup-20261031/`: the fresh Swift
report records source hashes and commands; the prior-task Android JNI library's
export check passed, proving only that supplied artifact's exports. Apple and
Android SDK `nm` tools failed on LLVM 22 bitcode in the Rust archive and were
correctly rejected despite partial symbol output. A compatible Rust-distributed
inspector is required. No build-only/export check certifies platform execution,
binary ABI or correspondence between an old library and changed source.

Final source/API verification, initializer correction and Fable review remain
to be appended; the defect retains draft status.

## Source/API completion addendum (2026-10-31)

This supersedes the pending source/API items above. The current manifest was
written only after Claude Fable 5.1 accepted the JS/API surface review and its
declaration hashes were checked against that reviewed cut. It records the
API-0012 RN additions and SPEC-0040 conflict/report/status additions explicitly.
The real `sources` command passes. Root CI and CONTRIBUTING invoke that gate and
the release-tool regression suite.

Final independent inspection found that stripping Android symbol versions could
hide a changed/default-removed version, and an undefined symbol could be counted
as an export. Two added regression cases produced four failing assertions;
the fixed checker requires exact default `@@HOYADB_1` versions and rejects
undefined symbols. All 22 tooling tests now pass normally and under Python
`-O`. The actual current macOS archive and the prior-task Android JNI artifact
pass their export checks with a compatible LLVM inspector. Their distinct
artifact scope is recorded; Android export success alone remains an old-artifact
claim, not a current-source Android execution claim.

The old four-parameter Swift Database initializer is restored as an overload;
the prior SPEC-0036 five-parameter form also remains. A fresh compile/digester
run now passes against the unchanged historical dumps. Its only exact accepted
diagnostics are `HoyaError.subscriptionTerminated` (SPEC-0036's explicit
source-breaking addition) and `QueueEntry.State.discarded` (SPEC-0040's explicitly
permitted API changes and required SDK enum/status update). The latter approval
is distinct from the stored-state downgrade consequence. No initializer
removal or other diagnostic is allowed. Eight retained historical snapshot
files were compared with the task-start copy and are byte-identical.

Evidence: `/tmp/hoyadb-followup-20261031/compat-swift-final/report.json`
(`passed: true`, fresh source/dump hashes and all commands),
`compat-sources-green.json`, `compat-tests-final.log`,
`compat-tests-optimized.log`, `compat-version-red.log`,
`compat-apple-current-exports.json`, `compat-android-exports.json` and
`compat-historical-unchanged.json`. Swift's controlled host/guard/background
suite passed 27 tests with warnings treated as errors; strict Swift formatting
and the scoped whitespace check passed. Current native Swift execution and
the complete Apple package build remain separately tracked in the task report.

## Reviewed checkpoint and Apple artifact verification (2026-10-31)

Claude Fable 5.1's final compatibility review found no defects in the exact
final checker, tests, accepted-change list and reviewed source manifest. It
independently reran all 22 tests and the source gate successfully; the response
is retained in `/tmp/hoyadb-followup-20261031/fable-js-compat-final-review.md`.
This supersedes the earlier pending review notes.

At the frozen SPEC-0040 checkpoint, the full Swift suite passed 102 tests with
warnings treated as errors against the independently pinned current native
archive. The standard Apple builder then produced all four target builds; its
macOS archive is byte-identical to the one executed by those tests. All three
packaged slices passed the actual native export check. The ordinary artifact
checker passed all five cases, including duplicate-package Swift import and
complete notice closure, against the independently retained compilation parent.

The build initially refused stale provider evidence, as required. Fresh actual
per-architecture symbol inspection justified updating only the current provider
bindings. The old source index is retained byte-exact, while the new source
parent, inspection commands/tool hashes, imports and capture script are in
[approved engineering evidence](../../implementation/approved-engineering-evidence/README.md).
The unmodified finalizer passed after that evidence refresh. Historical API
snapshots, license texts and old runtime captures remain unchanged. Later core
work needs its own corresponding runtime/artifact verification; this checkpoint
does not certify later source, iOS execution or publication.
