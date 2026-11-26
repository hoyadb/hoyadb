---
id: DEF-0051
title: Quality evidence includes generated and excluded crate sources
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-30
updated: 2026-10-31
parents: [SPEC-0046]
affects: [SPEC-0044, NFR-0006]
regression_for: [TC-0050]
tags: [review, regression]
---

# DEF-0051: Quality evidence includes generated and excluded crate sources

## Symptom

The 2026-10-30 review recorded REL-02: Broad crates globs include standalone bridge build output and excluded bridge sources in purported root-workspace coverage and mutation evidence.

## Acceptance

Scope snapshots and coverage inventories to actual root-workspace members and their first-party inputs, excluding generated outputs while detecting real source additions and drift.

## Root cause and fix

The former `crates/*/**/*.rs` and manifest globs included the excluded native/web
bridges after the public layout move, including their generated Cargo output.
`tests/tooling/quality.py` now validates the explicit four-member root workspace
for both coverage and mutation snapshots. It collects each member's manifest,
package-root Rust and `src`, `tests`, `examples` and `benches` Rust, plus shared
conformance sources and root measurement inputs. Coverage's uninstrumented
production inventory uses the same member source roots.

## Regression and verification

TC-0050 FIX08 is implemented in `tests/tooling/test_quality.py`: generated output
inside an excluded bridge or workspace member does not change evidence; excluded
bridge source/manifests do not enter coverage inventory; actual workspace
production, build and test inputs still invalidate stale evidence. The real
mutation CLI accepts unrelated bridge output and rejects added, changed and
removed workspace source.

