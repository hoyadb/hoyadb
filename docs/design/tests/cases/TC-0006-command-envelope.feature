# id: TC-0006
# verifies: REQ-0012, REQ-0013, REQ-0014
# doc: TC-0006-command-envelope.md

@REQ-0012
Feature: What travels when a command is pushed
  As a developer integrating hoyadb
  I want the backend to receive the action and its inputs, not my local rows
  So that the backend re-runs the intent authoritatively

  Background:
    Given a database opened against the in-memory engine at schema version 3
    And a fake transport capturing the envelopes it is given
    And the walkthrough schema

  Scenario: The envelope carries the command name, args and context
    When I execute "add-order-v1" for customer "cust_123" with 1 item
    And the queue is pushed
    Then the captured envelope has command "add-order-v1"
    And the captured envelope's args contain customerId "cust_123"
    And the captured envelope has a non-empty context identity

  Scenario: The envelope does NOT carry the local row mutations
    When I execute "add-order-v1" whose handler inserted 1 order and 2 order_items
    And the queue is pushed
    Then the captured envelope contains no orders rows
    And the captured envelope contains no order_items rows

  @REQ-0013
  Scenario: hoyadb attaches a time-sortable idempotency key without app code doing so
    When I execute "add-order-v1" for customer "cust_123" with 1 item
    And the queue is pushed
    Then the captured envelope has a non-empty idempotencyKey
    And that key was generated with no network access

  @REQ-0013
  Scenario: Two distinct commands get distinct keys
    When I execute "add-order-v1" twice with identical arguments
    And the queue is pushed
    Then the two captured envelopes have different idempotencyKey values

  @REQ-0014
  Scenario: The envelope declares the client's schema version
    When I execute "add-order-v1" for customer "cust_123" with 1 item
    And the queue is pushed
    Then the captured envelope has schemaVersion 3

  @REQ-0014
  Scenario: An entity-diff payload also declares the schema version
    When I update an order_notes record's body
    And the queue is pushed
    Then the captured payload has schemaVersion 3
    And the captured payload has table "order_notes" and op "update"

  @REQ-0014
  Scenario: A command's name carries its own version suffix
    Given commands "add-order-v1" and "add-order-v2" are both registered
    When I execute "add-order-v2" for customer "cust_123" with 1 item
    And the queue is pushed
    Then the captured envelope has command "add-order-v2"
    And no envelope with command "add-order-v1" is produced
