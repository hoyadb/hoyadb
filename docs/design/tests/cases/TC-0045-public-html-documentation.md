---
id: TC-0045
title: Public documentation builds and examples execute
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-27
updated: 2026-10-27
parents: [TP-0001]
verifies: [SPEC-0041, UX-0003, REQ-0047]
tags: [documentation, public-html, executable-examples]
---

# TC-0045: Public documentation builds and examples execute

```gherkin
Feature: Read and reproduce public documentation
  Scenario: DOC01 complete offline site
    Given the committed library source and public documentation sources
    When the documented build command finishes
    Then getting started, concepts, practical guides and public reference exist
    And every local page, asset and fragment link resolves
    And the site loads without a CDN or telemetry
  Scenario: DOC02 examples are executable source
    Given each displayed executable code block names a checked-in example
    When the documented example verification command runs
    Then every example executes against the actual library and passes assertions
    And an invalid example fails verification rather than being skipped
    And every code block is labelled executed example or source-only reference
    And no code block lacks its source and verification link
    And no held JVM execution or user store is touched
  Scenario: DOC03 source and reference stay honest
    Given the current public crate exports and SDK declarations
    When reference pages are built
    Then current public exports have generated reference entries
    And planned Discarded policy is labelled unimplemented
    And every page shows source commit, build date and "Unreleased · local packages"
    And verification records source hashes, commands and actual outcomes
    And a guide symbol missing from generated reference fails with a path diagnostic
    And the verification page distinguishes runtime checks from source review
  Scenario: DOC04 accessible reading and search
    Given a generated page
    Then disk-open search uses an inline index without fetch or XHR
    And a recorded browser check covers 320 CSS pixels and keyboard focus
    And automated structural checks do not claim full WCAG conformance
    And a skip link, labelled navigation, one main and a language are present
    And keyboard focus is visible and content reflows at 320 CSS pixels
    When the search input has no matching page
    Then it displays "No pages found. Try another term."
  Scenario: DOC05 repeatable tooling and negative controls
    Given malformed local links, missing example source or a missing reference symbol
    When build verification runs
    Then it fails with a path-specific diagnostic
    And generated code is escaped so it cannot become HTML
```
