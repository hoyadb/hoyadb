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
tags: [sdk, bindings, mobile, dependency-approval]
---

# ADR-0035: General SDK bindings and restricted mobile builds

## Context

The operator requested full platform SDK with recommended policies and periodic
Claude cross-review. RFD-0006 and API-0011 define one semantic Rust core,
owned host scopes and passive hooks, protected language bindings, local packages
and measured baselines. Exact dependencies and TC-0016's historical ID conflict
were held as separate explicit decisions.

## Decision

Core and memory remain dependency-free. Platform activation is limited to the
reviewed RN/React/CLI, Kotlin, native provider and exact transitive graph recorded
by current locks and toolchain controls. New components require separate review;
cache presence does not authorize activation. RFD-0006 defines SDK ownership and
packaging boundaries. Build-only inputs are not runtime dependencies.

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

Activate only that exact supplement with the original restricted build policy.
App/SDK/core runtime pins remain unchanged; the separately disclosed ktfmt0.54
formatter is excluded. Existing explicit JDK selection and disabled downloads,
verified bytes, isolated trusted offline builds and unknown-component stops
remain binding. This approval does not establish mobile/runtime/platform SDK success.
