# id: TC-0007
# verifies: REQ-0016, REQ-0017
# doc: TC-0007-outcomes-and-rollback.md

@REQ-0016
Feature: Observing a command's outcome
  As a developer integrating hoyadb
  I want one transport-agnostic outcome and a working rollback
  So that my recovery code never branches on a transport-specific code

  Background:
    Given a database opened against the in-memory engine
    And the walkthrough schema
    And a fake transport whose responses I control

  Scenario Outline: The outcome is surfaced as a transport-agnostic status
    Given the fake transport will respond to "add-order-v1" with "<status>"
    When I execute "add-order-v1" and the queue is pushed
    Then the observed outcome status is "<status>"
    And the observed outcome carries no HTTP status code
    And the observed outcome carries no correlation field

    Examples:
      | status    |
      | applied   |
      | corrected |
      | rejected  |

  Scenario: Acknowledgement is distinct from acceptance
    Given the fake transport will acknowledge "add-order-v1" and then reject it
    When I execute "add-order-v1" and the queue is pushed
    Then the command was acknowledged
    And the command was discarded from the retry queue
    And the final observed outcome status is "rejected"

  Scenario: No acknowledgement leaves the command queued for retry
    Given the fake transport's push rejects with a network failure
    When I execute "add-order-v1" and the queue is pushed
    Then the command state is "sent"
    And the command is still eligible for retry

  @REQ-0017
  Scenario: A correction reaches the app through its ordinary subscription
    Given I am observing orders where "status" = "pending"
    And the fake transport will respond to "add-order-v1" with "corrected" and total_cents 3800
    When I execute "add-order-v1" and the queue is pushed
    Then the subscription is notified of the corrected order
    And no separate error-only channel was used

  @REQ-0017
  Scenario: A rejection rolls back the optimistic mutation
    Given the fake transport will respond to "add-order-v1" with "rejected" and reason "out of stock"
    When I execute "add-order-v1" and the queue is pushed
    And I invoke the rollback hoyadb exposes
    Then no order created by that command remains in the local store
    And no order_items created by that command remain in the local store

  @REQ-0017
  Scenario: Rolling back a rejected parent cascades to a blocked dependent
    Given I executed "add-order-v1" and then "add-order-note-v1" referencing its local id
    And the fake transport will respond to "add-order-v1" with "rejected"
    When the queue is pushed and I invoke the rollback
    Then the order is rolled back
    And the dependent note command is rolled back too
    And the note is not left referencing an id that will never resolve
