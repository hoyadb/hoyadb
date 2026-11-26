# id: TC-0014
# verifies: REQ-0029
# doc: TC-0014-batched-push.md

@REQ-0029
Feature: Pushing writes as an array
  As a developer implementing a transport
  I want one wire shape and independent per-write outcomes
  So that there is only one path that has to stay correct

  Background:
    Given a database opened against the in-memory engine
    And the walkthrough schema
    And a fake transport whose responses I control

  Scenario: A single eligible write is still pushed as an array of one
    Given exactly 1 write is eligible to send
    When the queue is pushed
    Then the transport's push received an array of length 1
    And no alternative single-write call shape was used

  Scenario: Several eligible writes are pushed as one array in local execution order
    Given 3 independent writes are eligible to send, executed in a known order
    When the queue is pushed
    Then the transport's push received an array of length 3
    And the entries appear in the same order they were locally executed

  Scenario: A command and an entity-diff can share one array
    Given 1 command and 1 entity-diff write are both eligible
    When the queue is pushed
    Then the transport's push received an array of length 2
    And one entry carries a "command" field
    And the other entry carries "table" and "op" fields

  Scenario: Each entry resolves independently — no cross-write rollback
    Given an array of 2 commands
    And the backend will apply the first and reject the second
    When the queue is pushed
    Then the first command's outcome status is "applied"
    And the second command's outcome status is "rejected"
    And the first command was not rolled back

  Scenario: Outcomes correlate by idempotency key, not array position
    Given an array of 2 commands with keys "01J...Q3" and "01J...Q4"
    And the backend returns their outcomes in the reverse order
    When the queue is pushed
    Then the outcome carrying key "01J...Q3" is applied to the first command
    And the outcome carrying key "01J...Q4" is applied to the second command

  Scenario: An entity-diff outcome correlates by record id
    Given an array containing an entity-diff write for recordId "note_789"
    When the backend returns an outcome carrying recordId "note_789"
    Then that outcome is applied to that entity-diff write

  Scenario: A missing outcome leaves its write retryable
    Given an array of 2 writes
    And the backend returns an outcome for only the first
    When the queue is pushed
    Then the second write's state is still "sent"
    And the second write is still eligible for retry
