---
id: TC-0033
title: Rust formatting is consistent and preserves existing behavior
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-23
updated: 2026-10-23
parents: [TP-0001]
verifies: [SPEC-0030, NFR-0005]
tags: [tooling, formatting, rust]
---

# TC-0033: Formatting acceptance

```gherkin
Feature: Consistent Rust source layout
  Scenario: F01 small constructs have readable multiline layouts
    Given compact struct literals, named enum variants and conditional blocks
    When the pinned stable rustfmt formats them using the root configuration
    Then the output matches tests/tooling/formatting.sh's expected layout

  Scenario: F02 every Rust package uses the same formatting rules
    Given the workspace and four excluded spike manifests
    When cargo fmt is checked for each manifest
    Then all checks pass without unstable-option warnings
    And declarations and methods have blank-line separators on diff review

  Scenario: F03 existing behavior remains intact
    Given the formatting-only source changes
    When workspace and host spike tests and applicable Clippy checks run
    Then the existing checks pass
    And existing Rust changes match formatter output plus blank-line separators
```

F01 runs with `sh tests/tooling/formatting.sh`. F02 uses the commands in the
root README. F03 is a regression boundary, not a new full-product acceptance
or browser/Swift runtime claim.
