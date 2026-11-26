---
id: TC-0047
title: Binding quickstarts preserve and color verified source by language
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-28
updated: 2026-10-28
parents: [TP-0001]
verifies: [SPEC-0043, SPEC-0041, UX-0003, REQ-0047]
tags: [documentation, syntax-highlighting, bindings]
---

# TC-0047: Language-aware quickstarts

```gherkin
Feature: Read and reproduce each binding's getting started
  Scenario: LANG01 color source without changing it
    Given JavaScript, TypeScript, TSX, Rust, Swift, Kotlin, C, JSON and shell source
    When the renderer highlights a full example or included region
    Then keyword, string, comment and number tokens have language-aware classes
    And stripping markup recovers exactly the original source
    And script-like text, raw strings, triple strings and Unicode remain escaped data
    And regex versus division, nested templates/comments, lifetimes versus chars and comment-like strings preserve text
    And single-line string tokens stop at the end of the line
    And an unsupported language stays escaped plain text
  Scenario: LANG02 expose language and accessible colors
    Given executable and source-only blocks and generated declaration pages
    Then each block has its actual language label and language attribute
    And every syntax color has at least 4.5 to 1 contrast on the code background
    And the disk-open site requires no external highlighting asset or fetch
    And all quickstarts reflow at 320 pixels with selectable scrollable code
  Scenario: LANG03 find every binding entry
    Given the getting-started picker and navigation
    Then JavaScript, TypeScript, Rust, Swift, Kotlin, React Native and C each have an entry
    And each explains setup, schema, owner opening, query, observer, handler, sync and close
    And every named method exists in the current binding reference
    And local handlers and backend handlers are explained separately
  Scenario: LANG04 prove runnable examples and state native limits
    When the documentation verifier runs
    Then Rust, JavaScript, emitted TypeScript, Swift and C examples execute real library calls
    And shared React Native application logic executes against the real core
    And Kotlin and native bootstrap snippets carry explicit source-only labels if execution is held
    And source-checked means included checked-in source with symbols validated against current declarations
    And uncompiled snippets say "Source-checked · not compiled or executed" with the reason
    And native outputs stay in task-owned ignored locations without replacing packaged artifacts
    And source and artifact hashes identify each native build
    And a compile or setup failure never counts as a behavioral RED
  Scenario: LANG05 reject false verification
    Given an unknown snippet, missing language, source-only block claiming execution or absent binding symbol
    When the documentation build runs
    Then it fails with a source-specific diagnostic
    And highlighting never makes a previously invalid or unverified block valid
    And all previous example, package, link and fragment checks remain enabled
```

```gherkin
  Scenario: LANG06 teach through focused tested snippets
    Given the tutorial and guide pages
    When the documentation is built
    Then application code is included through named regions rather than whole-file test dumps
    And each snippet has adjacent prose explaining its purpose or result
    And verification-only Rust assertions and JavaScript checks stay outside displayed regions
    And every snippet links to its complete tested source and verification result
    And all examples retain their executable checks and completion markers
    And source text is dedented consistently without losing its internal indentation
```

Readability review also checks that result comments have adjacent executable
assertions, hidden example helpers are introduced, snippets contain whole
statements with balanced delimiters, and source-order departures are explained.

```gherkin
  Scenario: LANG07 understand and run Rust Todo commands
    Given create-task-v1 declares a required string title
    When the application supplies a title and calls its named local handler
    Then business normalization determines the stored title
    And the committed receipt has a command key and the handler's chosen JSON result
    And a blank title is rejected without a row or queue commit
    When complete-task-v1 is invoked with the task_id argument
    Then its handler updates the task and inserts an activity through one request
    And one finish commits both writes and one additional queue record
    When the second write fails after the update was staged
    Then abort preserves the prior task, activity rows and queue state
    And the same intent then completes successfully with a new request
    And task_id is declared as a reference path
    And completing before the first sync waits for creation and sends the mapped task_id afterwards
```
