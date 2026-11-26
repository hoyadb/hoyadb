---
id: ADR-0035
title: General SDK bindings and explicitly approved restricted mobile builds
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [REQ-0046, REQ-0034, ARC-0002]
depends_on: [RFD-0006, SPEC-0033, API-0011, TC-0036, TC-0037, THR-0020]
tags: [phase-seven, bindings, mobile, dependency-approval]
---

# ADR-0035: General SDK bindings and restricted mobile builds

## Context

The operator requested full Phase 7 with recommended policies and periodic
Claude cross-review. RFD-0006 and API-0011 define one semantic Rust core,
owned host scopes and passive hooks, protected language bindings, local packages
and measured baselines. Exact dependencies and TC-0016's historical ID conflict
were held as separate explicit decisions.

On 2026-10-24 the operator answered "Approve the exact bundle and restricted
build policy (Recommended)" and "Approve the prefix clarification (Recommended)".
The approved packet is the exact review snapshot committed at dc5469e. Its
[approval record](../../implementation/phase-seven-dependencies/approval.json)
preserves each reviewed file's SHA256 and the restrictions.

## Decision

Adopt RFD-0006's recommended general facade/SDK ownership and packaging policies.
Activate only the reviewed RN 0.86.3 / React 19.2.3 / CLI 20.2.0, Kotlin 2.1.20,
patched Okio 3.4.0 and exact locked transitives and six hashed iOS containers in
the [dependency packet](../../implementation/phase-seven-dependencies/README.md).
Core and memory remain dependency-free. Newly selected components outside that
packet require separate review/approval; existing cache presence is not approval.

Accept the disclosed build-only risk: 47 unique advisory matches, including
3 critical, remain in the unchanged build-tool graph. Runtime scans had no known
matches at review time. This does not establish exploitability or unreachability.
Use task-owned trusted projects/inputs and isolated Gradle homes, no remote or
shared/untrusted build caches, prefer `--no-build-cache`, verify exact archive
bytes before activation and build offline after verified inputs are available.
The six approved iOS containers have no implicit source-build fallback.
Review actual build commands before executing them. No arbitrary plugin/project,
external publication or operation on user stores is authorized.

Explicitly supersede TC-0016's historical unprefixed command-created ID example:
the command case must inject and return exactly `local:test-fixed-id-001`;
ordinary unconfigured CRUD must still inject and return exactly
`test-fixed-id-001`. Retain the historical wording with an explicit supersession
notice. Do not relax the core's surviving-command `local:` recovery guard or
replace exact-ID assertions with prefix-only checks.

## Consequences and verification

Approval clears activation and ID-policy gates; it proves no runtime or phase
completion. Actual independent consumer, compatibility, archive, Android 16 KiB,
security, async lifetime and complete performance gates remain TC-0036/0037.
Final Claude cross-review remains required. Device builds and simulator/emulator
execution must be labelled separately. Numerical budgets still require review
of observed repeatability, rather than a guessed threshold.

Rollback is a code/package revert with original stores and keys preserved.
No format conversion, reset, migration, publication, push or tag is implied.

## Supplemental exact build-plugin approval

On2026-10-24 the operator explicitly answered "Approve exact build-only addendum
(Recommended)" for snapshot03048bb. The [approval record](../../implementation/phase-seven-plugin-addendum/approval.json)
freezes its reviewed bytes: six additional Maven coordinates (four JARs and two
marker POMs) plus Foojay's embedded Gson2.13.1. The [packet](../../implementation/phase-seven-plugin-addendum/README.md)
retains scope, independent Claude consistency review, exact archive hashes,
notices, advisory results and publisher-attestation limitations.

Activate only that exact supplement with the original restricted build policy.
App/SDK/core runtime pins remain unchanged; the separately disclosed ktfmt0.54
formatter is excluded. Existing explicit JDK selection and disabled downloads,
verified bytes, isolated trusted offline builds and unknown-component stops
remain binding. This approval does not establish mobile/runtime/Phase7 success.
