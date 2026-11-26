---
id: TC-0039
title: Corrected clock samples recover retries without changing identity
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-26
parents: [TP-0001]
verifies: [SPEC-0035, REQ-0018, REQ-0043, DEF-0036]
regression_for: [DEF-0036]
tags: [purpose-audit, retry, clock, boundary, restart]
---

# TC-0039: Retry clock recovery

State-transition and boundary tests with injected clocks; fixtures are synthetic,
offline and task-owned. Public-owner cases run on memory and SQLite. Counts are
not a coverage percentage or a mobile runtime result. These cases cover
DEF-0036 CL01-CL03 only; acknowledged final-outcome recovery CL04 stays open.

```gherkin
Feature: Retry scheduling recovers while command identity stays monotonic
  Scenario: CS01 a corrected forward jump retains a bounded future deadline
    Given raw sample r and the maximum configured jittered delay C
    And an offline command sent using valid forward sample F equal to r plus 10 times C
    When prepare_push receives a corrected raw sample
    Then no premature envelope is emitted
    And next_due_ms is no later than raw sample plus the maximum configured jittered delay
    And the command key attempts state and frozen envelope are unchanged
    And this deadline assertion fails on the unchanged implementation

  Scenario: CS02 increasing corrected samples do not extend the capped deadline
    Given the capped Sent record from CS01
    When corrected samples progress before the capped deadline and then reach it
    Then earlier samples emit no envelope and retain the same deadline
    And the due sample emits exactly the original frozen envelope
    And the attempt count advances exactly once

  Scenario: CS03 restart retains the cap and its recovery behavior
    Given a cap persisted by prepare_push without sending an envelope
    When the owner closes and reopens with the same stored options
    Then the exact capped deadline and key remain
    And retry at that deadline uses identical frozen bytes

  Scenario: CS04 ordinary forward and small backward samples stay exact
    Given the existing c16 backoff jitter and restart fixture
    When all its original samples and assertions run
    Then every original deadline jitter invalid-input and frozen-byte assertion is unchanged
    And a deadline inside the recovery ceiling is never shortened

  Scenario: CS05 timestamp and cap arithmetic reject or saturate safely
    Given supported boundary timestamps and large valid retry options
    When the pure planner computes a recovery ceiling and a new send deadline
    Then ceiling arithmetic never wraps or panics
    And an unrepresentable new deadline yields the existing LimitExceeded error
    And the original ledger remains unchanged on failure
    And out-of-range timestamps and jitter remain rejected

  Scenario: CS06 receipt and outcomes are not silently rearmed
    Given Queued Sent Acknowledged and terminal control records
    When a corrected sample causes a Sent deadline cap
    Then only Sent recovery deadlines are capped
    And Acknowledged and terminal records are not resent
    And Queued work uses the raw sample with its unchanged backoff policy

  Scenario: CS07 failed durable cap preserves all prior bytes
    Given a real owner whose cap plan reaches an injected commit failure
    When prepare_push fails
    Then the application data model identity ledger and original deadline remain unchanged
    And reopening shows no partial cap or attempt change

  Scenario: CS08 a far-future identity floor does not become the scheduling sample
    Given header.last_ms remains above the corrected raw sample
    When a capped retry or a new Queued command is sent
    Then its deadline is computed from the raw sample
    And the header identity floor never decreases
    And later command keys remain unique and monotonic

  Scenario: CS09 a backward sample no longer uses the identity floor as elapsed time
    Given a Sent record with raw sample r below due_ms and due_ms at or below header.last_ms
    And its deadline is inside r plus C
    When prepare_push receives r
    Then the deadline is unchanged and no envelope is sent
    And a later sample equal to due_ms sends the exact original envelope
    And the unchanged floor-based planner first fails the no-envelope assertion
```

CS01 is the initial behavioral RED. CS04 and invalid-input controls are
characterization checks and must not be relabelled as newly failing behavior.
Record exact source hashes for the RED and GREEN cuts. No network, actual user
store or held compiler/runtime operation is needed.
