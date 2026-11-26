# id: TC-0008
# verifies: REQ-0018, REQ-0019, NFR-0003
# doc: TC-0008-queue-durability-and-dependents.md

@REQ-0018
Feature: Surviving being offline and being killed
  As a developer integrating hoyadb
  I want queued writes to survive an app kill
  So that a user's offline write is never silently lost

  Background:
    Given a database opened against a transaction-capable engine
    And the walkthrough schema

  Scenario: A write moves through the queue states
    When I execute "add-order-v1"
    Then the queued write's state is "queued"
    When the queue is pushed with no response
    Then the queued write's state is "sent"
    When the backend acknowledges it
    Then the write is discarded from the queue

  @NFR-0003
  Scenario: The state transition and the local mutation commit together
    When I execute "add-order-v1" and the process is killed immediately after the transaction commits
    And I reopen the database
    Then either both the order and its queued command are present
    And never the order without its queued command

  @NFR-0003
  Scenario: A kill before the transaction commits leaves neither
    When I execute "add-order-v1" and the process is killed before the transaction commits
    And I reopen the database
    Then no order from that command is present
    And no queued command for it is present

  @REQ-0018
  Scenario: A restart recovers the last known state rather than losing the write
    Given I executed "add-order-v1" and the queue was pushed with no response
    When the process is killed and I reopen the database
    Then the queued write is recovered in state "sent"
    And it is still eligible for retry

  @NFR-0003
  Scenario: A non-transactional engine claims no atomicity
    Given a database opened against an engine without transaction support
    When I execute "add-order-v1"
    Then the write's state is tracked best-effort
    And hoyadb reports no atomicity guarantee for that engine

  @REQ-0019
  Scenario: A dependent write is held back until the parent id resolves
    When I execute "add-order-v1" and then "add-order-note-v1" referencing its local id
    Then the note command's state is "queued"
    And the note command is never transmitted while the parent id is unresolved

  @REQ-0019
  Scenario: The local id is swapped on the authoritative outcome, not on acknowledgement
    Given I executed "add-order-v1" and then "add-order-note-v1" referencing its local id
    When the backend acknowledges the order without an outcome
    Then the order's id is still the local id
    And the note command is still "queued"
    When the backend's outcome is "applied" with id "order_456"
    Then the order's id is "order_456"
    And the note command's orderId argument is corrected to "order_456"
    And the note command becomes eligible to send
