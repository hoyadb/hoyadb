# id: TC-0004
# verifies: REQ-0007, REQ-0008, REQ-0009
# doc: TC-0004-subscriptions.md

@REQ-0007
Feature: Reading once and observing live
  As a developer integrating hoyadb
  I want query results that update live at a predictable granularity
  So that I never re-filter a raw change feed myself

  Background:
    Given a database opened against the in-memory engine
    And the walkthrough schema
    And 3 orders with status "pending"

  Scenario: The same query supports a one-shot read and a live subscription
    When I fetch orders where "status" = "pending"
    Then the result contains exactly 3 orders
    When I observe orders where "status" = "pending"
    Then the callback receives 3 orders

  Scenario: Unsubscribing stops further notifications
    Given I am observing orders where "status" = "pending"
    When I unsubscribe
    And a new order with status "pending" is created
    Then the callback is not invoked again

  @REQ-0008
  Scenario: A record entering the set re-emits
    Given I am observing orders where "status" = "pending"
    When a new order with status "pending" is created
    Then the callback is invoked with 4 orders

  @REQ-0008
  Scenario: A record leaving the set re-emits
    Given I am observing orders where "status" = "pending"
    When an existing pending order's status changes to "shipped"
    Then the callback is invoked with 2 orders

  @REQ-0008
  Scenario: An unrelated field edit on a matched record does NOT re-emit
    Given I am observing orders where "status" = "pending"
    When an already-matched order's "notes" field changes to "Gift wrap"
    Then the callback is not invoked again

  @REQ-0009
  Scenario: Count-only observation emits a number, not records
    Given I am observing the count of orders where "status" = "pending"
    When a new order with status "pending" is created
    Then the count callback receives 4
    And no order records are materialized for that callback

  @REQ-0009
  Scenario: Per-field observation re-emits where the default would not
    Given I am observing orders where "status" = "pending" with columns ["total_cents"]
    When an already-matched order's "total_cents" changes from 4200 to 3800
    Then the callback is invoked again
    When that same order's "notes" field changes
    Then the callback is not invoked again
