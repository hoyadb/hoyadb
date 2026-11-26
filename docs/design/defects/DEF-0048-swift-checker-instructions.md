---
id: DEF-0048
title: Swift artifact instructions omit the required trust anchor
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-30
updated: 2026-10-30
parents: [SPEC-0046]
affects: [API-0011]
regression_for: [TC-0050]
tags: [review, regression]
---

# DEF-0048: Swift artifact instructions omit the required trust anchor

## Symptom

The 2026-10-30 review recorded SDK-5: The documented artifact check command omits --expected-parent-sha256 and fails the independent compilation-parent check.

## Acceptance

Document a complete workflow that supplies an independently trusted compilation-parent hash, preserving the trust gate and honest artifact freshness limits.

## Root cause and fix

`bindings/swift/README.md` now captures the compilation-parent digest from a
successful trusted local build's adjacent `Artifacts/native-build.json`, passes
it through `--expected-parent-sha256`, and shows the packed-copy invocation. It
requires the captured digest to be kept outside the distributed framework and
explicitly rejects using the packed claims themselves as a trust anchor.

The old checker invocation was rerun and failed at the missing required digest.
The corrected invocation passed the required digest's format check and ran all
five checks on the existing ignored framework: four passed, while notice closure
correctly failed `missing-packaging` because it has no `packaging.json`. This
failure occurs before the parent-hash comparison, so the real-artifact digest
match was not executed. The derivation was checked against the finalizer's
verbatim parent copy and the checker; synthetic Apple tests cover correct and
incorrect anchors. An end-to-end current-artifact pass still requires fresh
provider and packaging evidence.
No checker or trust gate was weakened; no Apple artifact certification is claimed.
The guide explains that a build's `stale-provider-evidence` failure requires fresh
reviewed evidence before proceeding. Status remains draft.

