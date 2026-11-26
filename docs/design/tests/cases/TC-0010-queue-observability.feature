# id: TC-0010
# verifies: REQ-0021, REQ-0022, THR-0006
# doc: TC-0010-queue-observability.md

@REQ-0021
Feature: Seeing what the queue is doing
  As a developer debugging a sync problem in production
  I want per-command state from one call
  So that "blocked on its parent" and "just queued" are distinguishable

  Background:
    Given a database opened against the in-memory engine
    And the walkthrough schema
    And a fake transport whose responses I control

  Scenario: One call returns both aggregate counts and per-command entries
    Given 3 queued writes and 1 sending write
    When I read the queue status
    Then the status reports pending 3 and sending 1
    And the status includes a per-command array
    And no second, separately-named method was required

  Scenario: A dependency-blocked command is distinguishable from an ordinary queued one
    Given I executed "add-order-v1" and then "add-order-note-v1" referencing its local id
    When I read the queue status
    Then the note command's entry has state "queued"
    And the note command's entry has blockedOn set to the parent's local id
    And the order command's entry has no blockedOn field

  Scenario: blockedOn reports the local id the app already holds
    Given a command blocked on a parent whose local id is "local:a1b2c3"
    When I read the queue status
    Then that entry's blockedOn is "local:a1b2c3"
    And that entry's blockedOn is not the parent's idempotency key

  Scenario: A recently-resolved command is still visible after leaving the queue
    Given a command that was acknowledged and rejected
    When I read the queue status
    Then its entry appears with state "rejected"
    And it is no longer counted in pending

  Scenario: Queue status is observable live
    Given I am observing the queue status
    When a new command is executed
    Then the observer is notified with the updated status

  @REQ-0022
  Scenario: A repeatedly rejected command is never auto-cancelled
    Given a command that has been rejected 10 times
    When I read the queue status
    Then its entry is still present with state "rejected"
    And hoyadb did not cancel it automatically

  @REQ-0022
  Scenario: Explicit cancellation rolls back and cascades
    Given I executed "add-order-v1" and then "add-order-note-v1" referencing its local id
    When I cancel the order command by its idempotency key
    Then the order command leaves the retry queue
    And its local optimistic mutation is rolled back
    And the dependent note command is rolled back too

  @REQ-0022
  Scenario: Cancelling a sent command is best-effort only
    Given a command in state "sent" that the backend may already have applied
    When I cancel it
    Then hoyadb rolls back the local mutation
    And hoyadb makes no claim that the backend will not apply it

  @THR-0006
  Scenario: The backlog threshold bounds the queue
    Given the pending-command threshold is configured to 500
    And 500 writes are already pending
    When I execute another command
    Then the write is rejected locally
    And the queue does not grow beyond the threshold
