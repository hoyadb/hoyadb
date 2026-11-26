---
id: TC-0048
title: Coverage evidence and mutation gates expose real test gaps
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-29
updated: 2026-10-29
parents: [TP-0001]
verifies: [SPEC-0044, REQ-0034, NFR-0005, NFR-0006]
regression_for: [DEF-0041, DEF-0042, DEF-0043]
tags: [coverage, mutation, public-api]
---

# TC-0048: Test quality and mutation testing

```gherkin
Feature: Trust test-quality evidence
  Scenario: QUALITY01 validate measured coverage
    Given LLVM coverage of every native workspace production source
    When the report is summarized
    Then each component has its own covered and total counts and uncovered files
    And zero or absent branch instrumentation is unknown rather than 100 percent
    And absent components, duplicate paths, impossible counts and stale sources reject
  Scenario: QUALITY02 preserve mutation outcome meaning
    Given isolated production mutations and the unchanged baseline tests
    When a mutation run finishes
    Then caught, missed, timeout and unviable counts are distinct
    And a survivor, timeout, missing outcome or baseline failure fails the gate
    And a focused run never claims full workspace mutation coverage
  Scenario: QUALITY03 assert public behavior
    Given the documented public method contracts
    When boundary and error cases execute against the real implementation
    Then exact results and typed failures are asserted
    And a mutation of those decisions causes the tests to fail
  Scenario: QUALITY04 preserve names and compatibility
    Given actual Rust and platform public declarations and their contracts
    When their names are audited
    Then documented language differences remain compatible
    And unresolved mismatches and unexecuted platform checks are recorded
  Scenario: QUALITY05 retry a failed browser runtime bootstrap
    Given concurrent default runtime calls share a pending worker
    When that worker fails before ready
    Then the next default call starts a fresh worker and can succeed
    And explicit-worker failures cannot replace or clear a successful default
  Scenario: QUALITY06 bound portable jitter sampling
    Given samples at either side of the rejection boundary
    When the runtime requests jitter
    Then accepted samples produce exactly 0 through 2500 basis points
    And rejected samples are retried at most 16 entropy calls before BackendFailure
  Scenario: QUALITY07 bounded parser execution remains observable
    Given a finite numeric and Unicode corpus with exact expected results
    When the parser executes in a supervised child process
    Then every value and rejection matches its expected result
    And a nonterminating child is killed and reaped and fails an assertion
  Scenario: QUALITY08 encrypted conformance preserves native capabilities
    Given the encrypted native SQLite conformance fixture
    When command ownership and validated queries execute
    Then ownership and readiness match the concrete adapter
    And queries use the concrete validated path and return the expected rows
  Scenario: QUALITY09 comparable evidence and failure artifacts
    Given measurement metadata and a recorded coverage baseline
    When a floor is compared across different targets or profiles
    Then the comparison fails with an explicit reason
    And a rejected mutation run retains structured counts source hashes and tool exit status
  Scenario: QUALITY10 recover new opens after browser worker retirement
    Given a successful default browser runtime with an active request
    When its worker crashes and later default opens are requested
    Then the active request fails and old handles remain closed
    And concurrent new opens share one fresh worker and runtime
    And late old-worker events and explicit-worker failures cannot clear the replacement
```
