# id: TC-0013
# verifies: REQ-0026, REQ-0027
# doc: TC-0013-scoping-and-reset.md

@REQ-0026
Feature: What a client holds, and giving it up
  As a developer integrating hoyadb
  I want a data footprint that follows identity, not screens
  So that what a client holds is predictable

  Background:
    Given a database opened against the in-memory engine
    And the walkthrough schema
    And a fake transport whose pull responses I control

  Scenario: A pull carries the opaque checkpoint back unchanged
    Given the last pull returned checkpoint "chkpt-124"
    When the next pull is issued
    Then the transport's pull was called with since "chkpt-124"
    And hoyadb did not parse or interpret that token

  Scenario: A first pull has no checkpoint
    Given no previous pull has occurred
    When a pull is issued
    Then the transport's pull was called with a null since value

  Scenario: The client holds what the backend returned, nothing inferred
    Given the backend returns changes for 3 orders within this client's membership
    When the pull is applied
    Then the local store contains exactly those 3 orders
    And hoyadb requested no data beyond what the backend exposed

  Scenario: Scoping does not follow UI state
    Given I am observing orders where "customer.country" = "FR"
    When I unsubscribe from that query
    And a pull is issued
    Then the data the client holds is unchanged
    And the pull request is identical to one issued while the subscription was active

  @REQ-0027
  Scenario: The reset operation is total
    Given the local store contains 3 orders and 2 order_notes
    When the developer invokes the reset operation
    Then the local store contains 0 orders
    And the local store contains 0 order_notes

  @REQ-0027
  Scenario: Reopening against a different local store leaves the original untouched
    Given the local store "workspace-a.hoyadb" contains 3 orders
    When the developer reopens against "workspace-b.hoyadb"
    Then the active store contains 0 orders

  @REQ-0027
  Scenario: hoyadb never purges revoked data on its own
    Given the local store contains 3 orders synced under a membership that has since narrowed
    When the backend recomputes membership to exclude them
    And the developer does NOT invoke the reset operation
    Then the 3 orders are still present in the local store
    And hoyadb did not detect the membership change
    And hoyadb did not purge anything automatically

  @REQ-0027
  Scenario: A queued write against revoked data is sent and rejected by the backend
    Given a write queued offline against data this client may no longer write
    And the backend will reject it with reason "not authorized"
    When connectivity returns and the queue is pushed
    Then the write was transmitted normally
    And the observed outcome status is "rejected"
    And hoyadb did not inspect the write against membership before sending it
