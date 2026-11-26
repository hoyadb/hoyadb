# id: TC-0005
# verifies: REQ-0010, REQ-0011
# doc: TC-0005-command-execution.md

@REQ-0010
Feature: Executing a command locally
  As a developer integrating hoyadb
  I want a command to apply instantly and atomically against the local store
  So that my app feels immediate without ever exposing partial state

  Background:
    Given a database opened against the in-memory engine
    And the walkthrough schema
    And a command "add-order-v1" whose handler reads the customer's prior order count and then inserts an order and its items

  Scenario: A command applies locally and immediately
    When I execute "add-order-v1" for customer "cust_123" with 2 items
    Then an order exists in the local store
    And 2 order_items exist in the local store
    And the returned order has a local id
    And the command is queued for sync

  Scenario: A command applies with no network connection
    Given every transport call fails
    When I execute "add-order-v1" for customer "cust_123" with 2 items
    Then the order still exists in the local store
    And the command remains queued

  @REQ-0011
  Scenario: A handler reads its own earlier writes
    When I execute "add-order-v1" for customer "cust_123" with 2 items
    Then the handler's order_items inserts reference the order id created earlier in the same handler

  @REQ-0011
  Scenario: A handler's awaited local read sees current local state
    Given customer "cust_123" already has 1 order
    When I execute "add-order-v1" for customer "cust_123" with 1 item
    Then the handler's prior-order-count read returns 1
    And the created order has "discounted" false

  @REQ-0011
  Scenario: Nothing outside the handler observes partial state
    Given I am observing orders where "status" = "pending"
    When I execute "add-order-v1" whose handler inserts an order and then 2 items
    Then the subscription is notified once, after the handler returns
    And at the moment of notification the order and both items are all present

  @REQ-0011
  Scenario: Two command handlers never interleave
    When I execute "add-order-v1" and "add-order-v1" back to back without awaiting the first
    Then the second handler does not begin until the first handler has returned
    And the second handler's first read sees all of the first handler's writes
