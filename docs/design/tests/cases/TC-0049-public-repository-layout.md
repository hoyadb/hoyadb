---
id: TC-0049
title: Public checkout validates independently and moved packages preserve behavior
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-30
updated: 2026-10-30
parents: [TP-0001]
verifies: [SPEC-0045, REQ-0048, NFR-0005]
tags: [distribution, tooling, compatibility]
---

# TC-0049: Public repository layout

```gherkin
Feature: Independently usable public source tree
  Scenario: LAYOUT01 isolated traceability checks
    Given only the public traceability tools and fixture artifacts in a temporary directory
    When the checker runs without a private repository
    Then valid artifacts pass and dangling links and missing fields fail
  Scenario: LAYOUT02 RTM generation and stale detection
    Given the same isolated public tools and valid linked artifacts
    When the RTM is generated and checked
    Then it passes and a modified matrix fails without being overwritten
  Scenario: LAYOUT03 public contributor commands
    Given the source tree without private submodule access
    When contributor instructions and CI jobs are inspected and executed
    Then required checks use public tools and no private checkout is fetched
  Scenario: LAYOUT04 preserve runtime and package behavior
    Given the moved Rust crates and npm packages
    When existing tests and independent packed-consumer checks execute
    Then unchanged package names and APIs pass the same behavioral assertions
  Scenario: LAYOUT05 preserve existing work and historical evidence
    Given the saved initial working-tree snapshot
    When the final source diff is reviewed
    Then existing user edits survive the moves
    And accepted ADR bodies, notices and historical evidence retain their bytes
    And publication stays disabled and the local private checkout remains intact
```
