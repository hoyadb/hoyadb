---
id: SPEC-0046
title: Repair confirmed SDK lifecycle and public validation defects
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-30
updated: 2026-10-30
parents: [ARC-0001]
satisfies: [REQ-0048, NFR-0006]
depends_on: [API-0011, API-0012, SPEC-0044, SPEC-0045]
tags: [review, regression]
---

# SPEC-0046: Review remediation

## Scope and authorization

On 2026-10-30 the operator requested fixes for the full project review, a
cross-review by Claude Fable 5.1, and an explicit report of remaining work.
Restore the existing API-0011/API-0012 lifecycle and retained-work contracts,
repair clean-source CI and quality/traceability validation, and correct active
guides. DEF-0044 through DEF-0053 record the ten reviewed findings.

The existing DEF-0040 Kotlin background-error gap is also in scope: implement
the already-declared API-0011 sink and its source, epoch, redaction and containment
behavior, with meaningful behavioral tests on a task-owned Android runtime.
TC-0040 and TC-0044 remain its authoritative cases; host compilation alone does
not establish Android reporting parity.

Use existing dependencies and scheduling mechanisms. Preserve public API shapes,
storage formats, security gates, user changes, accepted ADR bodies and historical
evidence. This work does not select publication policy or certify unexecuted
platforms. Existing scaling work under DEF-0034 remains subject to its documented
compatibility gates; do not silently migrate stores or weaken limits.

## Acceptance and verification

TC-0050 maps each finding to acceptance conditions. For behavior changes,
add a regression and observe its expected failure before implementation. Verify
real JavaScript/WASM integration, deterministic native host lifecycle cases,
public Python tools, source formatting, Rust tests/lint/build and traceability.
Keep host checks distinct from actual device and browser execution.

Compare final changes against the saved pre-fix working tree, cross-review
with Claude Fable 5.1, resolve supported findings, and report unresolved decisions
and verification limits. No commit, push or publication is authorized.

Actual outcomes and remaining scope are recorded in the
[implementation report](../../implementation/review-remediation.md).

## Rollback

Revert only remediation changes against the pre-fix snapshot. Restore
scheduler/tooling behavior and corresponding guides together. Do not revert
pre-existing layout changes or rewrite historical verification artifacts.

