---
id: SPEC-0044
title: Measured coverage and mutation testing with honest scope
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-29
updated: 2026-10-29
parents: [ARC-0001]
satisfies: [REQ-0034, NFR-0005, NFR-0006]
depends_on: [ADR-0036, API-0004, API-0005, API-0011, API-0012]
tags: [testing, coverage, mutation, compatibility]
---

# SPEC-0044: Test quality and mutation testing

Origin: operator request of 2026-10-29 to audit code/tests against documentation,
review public method names, improve meaningful coverage and add mutation testing,
with an independent Claude Opus 5.5 challenge. That request authorizes this
implementation; it does not authorize committing, publishing or changing public
contracts. Preserve the checkout's existing uncommitted work.

## Intended behavior

- Measure native workspace Rust line/region/function coverage using the pinned
  compiler and cargo-llvm-cov. Preserve file counts, uncovered locations, source
  hashes, tool versions, command and revision. Rust branch coverage is unknown
  when the pinned stable toolchain cannot instrument it. Regions are not branches.
  LLVM file-level counts include inline unit-test code; label this scope rather
  than claiming a production-only percentage.
- Report components separately; native workspace results do not establish WASM,
  native wrapper, JavaScript, tooling or real mobile execution coverage. Inventory
  those remaining measurement gaps. Do not add coverage suppression attributes,
  drop first-party source or weaken assertions to improve a number.
- Use pinned cargo-mutants to mutate actual production Rust in isolated source
  copies, executing dependent workspace tests. A full run is the default scope;
  focused runs are explicitly labelled and list their selection. Keep baseline
  tests enabled. Missing tools, baseline failure, survivors and timeouts must
  prevent a successful gate. Unviable mutants remain distinct from caught ones.
- Add behavioral tests against documented public contracts, guided by uncovered
  paths and surviving mutants. Demonstrate failures against the mutant or
  implementation defect, then passing tests. Test observable results and errors,
  atomicity and persistence rather than merely executing methods.
- Audit public names against the actual language contracts. Preserve intentional
  Rust snake_case versus host camelCase and documented reserved-word escapes.
  Do not silently rename published methods. Record unresolved inconsistencies
  and distinguish source checks, compilation and runtime verification.
- Keep reproducible quality commands and retain raw reports as local/CI artifacts.
  Do not claim a whole-project score from a selected module, a timeout, a stale
  report, a missing component or a partial run.

## Independent-review corrections

- Exercise bounded JSON parsing in a supervised child process with exact value
  and error assertions. A small finite corpus must finish within 10 seconds;
  the parent kills and reaps a stuck child and fails an assertion. This tests
  termination without changing production parsing or accepting mutation timeouts.
  The original unit tests remain in ordinary workspace regression execution.
- Encrypted conformance wrappers forward native ownership, readiness and the
  validated query path. Assert capability parity and actual validated-path use;
  retain existing conservative and false-capability adapter tests.
- Capture measurement command, target, profile, compiler, tool version and source
  hashes automatically. Reject floor comparisons across measurement identities.
  Preserve the measured macOS/dev floor; Linux needs its own measured baseline.
  A missing baseline must be explicit and cannot count as a passing floor check.
- Emit a structured failure artifact for rejected mutation evidence, with source
  identity and tool exit status; retain raw outcomes. Do not mask tool failures.
- Do not advertise an unbounded full-workspace mutation job as a practical CI
  gate. Keep full discovery available and select critical modules explicitly;
  broader runs require measured budgets and complete selected-outcome validation.

The operator also chose fresh default browser runtime creation after post-ready
worker retirement, with all existing handles kept closed. TC-0048 QUALITY10 and
DEF-0043 cover that additive recovery behavior; no cross-realm replay is allowed.

## Acceptance and rollback

TC-0048 defines the executable cases. Quality report validation rejects missing
components, invalid counts and mismatched source digests. The mutation entry
point propagates tool failure and rejects uncaught/time-limited experiments.
The delivery report records actual scope, all survivors and unknown metrics;
this change records measured scoped floors, not an invented numerical threshold. ADR-0036
remains unchanged. Remove the added tooling/CI jobs/tests to revert; no store,
schema, wire protocol or runtime dependency changes are needed.

Full platform execution and measuring every language may require existing
platform-specific build gates; unexecuted work remains explicitly open.
