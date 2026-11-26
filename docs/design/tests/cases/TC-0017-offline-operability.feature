# id: TC-0017
# verifies: NFR-0002
# doc: TC-0017-offline-operability.md

@NFR-0002
Feature: Working with no network at all
  As a user of an app built on hoyadb
  I want the app to keep working offline
  So that connectivity is never a precondition for using it

  Background:
    Given a database opened against the in-memory engine
    And a transport whose every push and pull rejects with a network failure
    And the walkthrough schema

  Scenario: Declaring a schema succeeds offline
    When I declare the walkthrough schema
    Then the declaration succeeds
    And no network call was attempted by hoyadb

  Scenario: Applying a migration succeeds offline
    Given a local database at schema version 1 and an authored chain to version 2
    When I open the database with a declared schema at version 2
    Then the migration is applied locally
    And the operation did not require connectivity

  Scenario: A one-shot query succeeds offline
    Given 3 orders with status "pending"
    When I fetch orders where "status" = "pending"
    Then the result contains exactly 3 orders

  Scenario: A live subscription delivers and updates offline
    Given I am observing orders where "status" = "pending"
    When a new order with status "pending" is created
    Then the subscription is notified with the updated result

  Scenario: A command applies optimistically offline
    When I execute "add-order-v1" for customer "cust_123" with 2 items
    Then the order and its items exist in the local store
    And the command is queued rather than failed

  Scenario: An entity-diff write applies offline
    Given an order_notes record "note_789"
    When I update that record's body
    Then the change is visible in the local store
    And the write is queued rather than failed

  Scenario: Queue introspection succeeds offline
    Given several queued writes
    When I read the queue status
    Then the status is returned
    And it reports the queued writes

  Scenario: Zero documented local operations require connectivity
    When I perform every operation in NFR-0002's measured set
    Then 0 of them failed
    And 0 of them required a successful network call to complete
