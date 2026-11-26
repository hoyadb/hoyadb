---
id: TC-0037
title: Measured protected consumer baseline precedes numerical regression budgets
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [TP-0001]
verifies: [REQ-0046, SPEC-0033, OBJ-0004, NFR-0001, NFR-0004]
tags: [sdk, performance, baseline]
---

# TC-0037: Measured protected consumer baseline precedes numerical regression budgets

No latency/memory/disk number has been measured by this draft. Dataset sizes and
sample counts below define an experiment, not a performance target. ADR0022's
baseline-first policy stays binding; concrete NFR-0001/OBJ-0004 targets follow actual
reviewed results rather than analogy or an arbitrary percentage.

## Fixed proposed protocol

Ordinary optimized release artifact, actual supplied synthetic key and approved
cipher profile. Same caller schema fixtures across memory, protected native,
protected browser and supported language packages. Separate platform/runtime/
device/simulator/engine classes; do not pool them into one baseline.

Datasets100/1000/10000 typed rows, including a related graph; queue0/10/100;
equivalent and distinct observer fanout1/10/100. Existing500-capacity stress runs
unchanged as a separate workload, not a proxy for single-operation latency.
Seed/content/result IDs/checksums fixed; generated data is synthetic. Record every
input shape and expected result before timing. Functional correctness asserts
outside timing where possible, never remove security/validation for benchmark.

Warm cases:5 warmups then30 measured iterations in each of5 independent runs.
Cold open:10 new-process/worker executions per class. Large stress separately
declares finite sample count and receives no misleading small-call percentile.
Record raw nanosecond/millisecond samples with timer resolution, median and
nearest-rank empirical p95,min/max/count.30 samples is a coarse empirical p95,
not a high-confidence population tail or p99. Retain setup/timeout/contaminated
samples with explicit reason; predeclare exclusion criteria, rerun rather than
quietly deleting slow data. Record power/thermal/background activity.

Measure SDK call→decoded return for point/selective/sorted-related query,
atomic batch, optimistic command with await/reentry, entity patch, non-noop pull
page/checkpoint and explicit reset. Separate core adapter-only time when used
for comparison. Cold create/reopen/upgrade includes key/KDF/validation cost;
state whether OS cache is warm, do not call process restart “cold disk.”

Reactivity: write invocation→actual callback and durable-return→callback, bytes
and observer fanout, membership/columns/count separate.16ms count throttle is
existing policy, not a new latency budget. Boundary: input/output bytes and
end-to-end encode/call/decode timing; do not subtract unrelated noisy runs and
label that exact FFI cost. Retry timing uses injected clocks for correctness and
separate actual wake-lateness samples for host scheduling.

Memory: native process peak resident bytes plus repeated open/observe/close
retained delta with named tool; browser JS heap and WASM linear memory separately
where actually measurable. Do not call WASM memory size total process RSS.
Footprint: complete owned encrypted main/journal/WAL after declared checkpoint/
close state, OPFS namespace bytes, compressed/uncompressed package assets and
architecture slices. Metadata JSON length is not disk use. Missing profiler
capabilities stay unknown, not zero.

## Acceptance

```gherkin
Feature: Performance claims are reproducible measurements
  Scenario: P01 frozen workload and protected artifact are identified
    Given a release package artifact with source dependency and generated hashes
    When the named baseline command creates its deterministic datasets
    Then runtime records actual OS CPU memory ABI device runtime cipher and build profile
    And exact result IDs counts event states and checksums match the predefined workload
    And protected storage stays protected with production validation and authentication

  Scenario: P02 API callback and boundary samples have explicit endpoints
    Given the declared workload sizes observer fanouts and timers
    When the5 runs and finite sample protocol execute
    Then all raw samples repeats timer resolution and percentile calculation are retained
    And warm cold process adapter SDK and callback endpoints are labeled separately
    And the native reactive primary value is explicitly invocation to last caller callback
    And any corrected analysis preserves original bytes hashes and every captured sample
    And failures exclusions contention and timeouts remain visible

  Scenario: P03 memory storage and package metrics report actual quantities
    Given the final supported artifacts and owned stores
    When native memory browser memory complete store and package sizes are measured
    Then bytes state tools and missing capabilities are explicit
    And no JSON length memory-engine or plaintext figure is claimed as encrypted disk performance
    And cleanup removes only task-owned stores and child processes

  Scenario: P04 summarizer correctness is independent of production implementation
    Given known sample arrays empty malformed mixed-unit and mixed-profile inputs
    When the stdlib summary tool calculates median and nearest-rank p95
    Then exact expected summaries match and invalid or mixed cases reject
    And zero fabricated-clock measurements count as performance evidence

  Scenario: P05 numeric budgets follow observed repeatability
    Given actual baseline run distributions and same-class repeated measurements
    When the operator reviews their observed noise envelope and central values
    Then OBJ-0004 NFR-0001 receive concrete Past Meter Scale Must and Target values
    And no arbitrary absolute SLO or guessed regression percentage is introduced
    And target is hold or improve the accepted measured baseline

  Scenario: P06 regression checks fail without silently changing their baseline
    Given the reviewed numerical budget table and raw-data hashes
    When an actual repeat run is compared under the same declared class
    Then the command enforces the accepted measurement-derived bounds
    When a synthetic regressed result is supplied to the gate
    Then it fails without updating baseline files or disabling the workload

  Scenario: P07 functional and security gates still describe measured artifacts
    Given the baseline source artifact runtime and security manifests
    When relevant full conformance security and package consumer gates are checked
    Then all applicable checks pass on matching sources and artifacts
    And no encryption integrity lifecycle or correctness assertion was removed for speed
    And every unexecuted required target or metric remains an open acceptance gate
```

Initial budget proposal after measurements: central run-level statistic as Target,
observed repeated upper envelope as candidate Must, with a paired same-machine
control rerun for exceedance. Review actual repeatability before selecting the
exact rule; this text is not an already-adopted numeric tolerance. Final budget
table is a required follow-up artifact, not a TODO hidden by phase completion.
