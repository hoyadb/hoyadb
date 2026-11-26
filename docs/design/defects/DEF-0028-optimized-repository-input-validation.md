---
id: DEF-0028
title: Optimized Python skips build repository input validation and metadata collection
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-25
updated: 2026-10-25
parents: [SPEC-0033]
affects: [SPEC-0033]
regression_for: [TC-0036]
tags: [phase-seven, build-tooling, integrity, python]
---

# DEF-0028: Optimized Python skips build repository input validation and metadata collection

## Observed behavior

During approved compiler-input activation preparation, running the repository
assembler target/phase-seven-rn/android/prepare-repo.py with Python -O produced
an apparently successful local repository with only34 metadata files. The valid
explicit-check rebuild contains633 metadata files and241 binaries. The invalid
run is preserved as repo-INVALID-O-run and was not used for any graph or build.
The author reported the issue and corrected the experimental script before this
formal defect and permanent regression were written; do not claim retroactive
documentation-first or RED-first implementation for that experimental correction.

The exact script bytes used for the34/633 incident were not preserved. The
available prepare-repo-v1.py.txt is an earlier pre-addendum script with the same
assert/setdefault mechanism, not that incident's exact source. Its SHA256 is
f76c2a5c04518d10da1f2bc1f0325b55ec66ba42575c09a953523eb70e651d6f.
Any replay must record its own input/output counts: it demonstrates the
mechanism, not reproduction of the34/633 totals.

## Cause and required behavior

The earlier assembler put validation in assert statements, including metadata
collection through provenance.setdefault inside an assert expression. Python -O
removes those statements, losing both validation and required state changes.
An optimized invocation must enforce the same exact approved hashes, conflict
checks and complete metadata inventory as an ordinary invocation. A changed or
missing input must reject without certifying the output as complete.

The permanent activation assembler is offline-only: no network subprocess or
implicit download fallback. Missing input is a local integrity rejection.
Approved acquisition, if needed, remains a separate explicit preparation step.
Do not use a network failure as the missing-input regression result.

## Minimal tool boundary and test isolation

Put the reusable assembler and tests under
packages/react-native/tools/android/prepare_repo.py and test_prepare_repo.py.
This tools directory is outside the package.json files whitelist, so build
verification utilities are not shipped in the RN SDK or its Gradle sourceDir.
It takes explicit --inputs, --out and --record paths; import has no side effects
and no behavior depends on the tool's directory depth. The input plan records
the complete expected relative output paths, input source references, kinds,
sizes and cryptographic hashes plus metadata provenance identities. It is
provided by the trusted activation controller after checking the frozen
approval records; a caller-supplied plan is not itself dependency approval.

Expose the same repository verification routine to graph/build preparation.
Write a success record only after copying and rechecking the exact expected
path set and hashes. The named consumer is the RN Android build controller,
currently target/phase-seven-rn/android/build.py: before invoking Gradle it must
verify that success record against the expected trusted plan and actual files.
Directory existence, counts alone or a self-declared completion flag cannot
authorize consumption. Incomplete output, unexpected files, changed bytes,
missing marker or a marker for another plan reject.

Use a small synthetic fixture with fake binary/metadata bytes and declared
hashes for permanent unit regressions. No actual JAR, ignored cache, Gradle,
compiler or network is needed. The real633/241 assembly is separate integration
evidence against approved input identities. Input/output plan validation must
reject absolute or escaping output paths; no test writes to shared inputs.

## Named permanent regression cases before final integration

- R01: Given a complete small synthetic input plan, when the actual assembler runs
  normally and under -O in separate fresh owned fixture roots, then both
  inventories contain the same expected binaries and complete metadata with
  identical bytes and byte-identical logical inventories/success records.
  An explicitly hash-identified replay of the earlier v1 script may reproduce
  the assert side-effect mechanism separately; do not label it the exact
  historical incident. Observe meaningful RED against the new actual tool's
  compiling stub before implementing its boundary.
- R02: Given a corrupted local binary or metadata input, when each invocation
  runs, then it fails for the expected integrity reason before any success
  record. Observe old -O validation bypass where reproducible; do not accept
  fixture setup, missing-module or network failures as that RED.
- R02b: Given a missing local binary or metadata input, when both modes run,
  then local validation rejects before a success record, without attempting a
  network operation. Check this independently from corrupted-input rejection.
- R03: Given conflicting metadata provenance identities for the same metadata
  path, when both modes run, then neither
  produces a successful repository. Collection must occur independently of
  validation expressions and cannot be skipped by optimization. This covers
  the provenance-map conflict lost by setdefault, separately from a duplicate
  destination with conflicting bytes, which must also reject.
- R04: Given a rejected or incomplete run, when graph/build preparation tries
  to consume its output, then it rejects the incomplete inventory rather than
  treating directory existence as certification. Exercise the named controller
  verification seam with missing/stale/tampered records and file mutations;
  rejection happens before any Gradle subprocess. Preserve failure evidence.

The shared verification routine has permanent repo-owned regression coverage.
Until the controller is also repo-owned, its invocation ordering before Gradle
is separate integration evidence; do not claim the permanent unit suite proves
that untracked controller's ordering.

Use a permanent repo-owned regression driven by the actual assembler source;
no substitute test-only assembler. Freeze source hashes and expected input/output
inventories. Tests must be offline, load no JAR/compiler, and modify no approved
shared input or cache. Temporary outputs are isolated and cleaned; the real
invalid-run evidence remains intact. TDD evidence distinguishes reproduction
against the saved pre-correction script from the later experimental fix.

The final repo-owned tool location and exact commands must be recorded with the
regression. No new dependency, runtime policy, compiler activation or user-store
change belongs in this correction. Rollback removes the unpublished tool
correction and derived outputs, preserving frozen inputs and failure evidence.

## Implementation verification

The permanent assembler and verifier now use explicit checks under both Python
modes. Root reran16 synthetic tests successfully with -I and with -I -O, then
verified the real874-file repository (241 binaries and633 metadata) with the
optimized tool. The independently rehashed source and evidence identities,
stub RED, disclosed harness/test corrections and separate v1 mechanism replay
are recorded in the [implementation evidence](../../implementation/phase-seven-rn-def-0028/README.md)
and [root verification](../../implementation/phase-seven-rn-def-0028-root-verification.json).
Recorded controller failures reject with zero launched steps; controller
ordering remains integration evidence as specified above. This closes the
assembler correction, not RN Android compilation, the security remediation or
the two final Phase7 review rounds.
