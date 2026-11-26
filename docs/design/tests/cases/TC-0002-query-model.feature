# id: TC-0002
# verifies: REQ-0003, REQ-0004, REQ-0005
# doc: TC-0002-query-model.md

@REQ-0003
Feature: Querying the local store
  As a developer integrating hoyadb
  I want a small, predictable predicate surface
  So that I know exactly what a query chain supports

  Background:
    Given a database opened against the in-memory engine
    And the walkthrough schema

  Scenario Outline: Each comparison operator filters as specified
    Given orders with total_cents 500, 1000 and 4200
    When I query orders where "total_cents" <operator> <value>
    Then the result contains exactly <count> orders

    Examples:
      | operator | value | count |
      | "="      | 1000  | 1     |
      | "!="     | 1000  | 2     |
      | "<"      | 1000  | 1     |
      | "<="     | 1000  | 2     |
      | ">"      | 1000  | 1     |
      | ">="     | 1000  | 2     |

  Scenario: The "in" operator matches any of several values
    Given orders with status "pending", "shipped" and "cancelled"
    When I query orders where "status" is in ["pending", "shipped"]
    Then the result contains exactly 2 orders

  Scenario: Multiple predicates combine as implicit AND
    Given an order with status "pending" and total_cents 4200
    And an order with status "pending" and total_cents 500
    When I query orders where "status" = "pending" and "total_cents" >= 1000
    Then the result contains exactly 1 order
    And that order has total_cents 4200

  @REQ-0004
  Scenario: A dot-path filters on a to-one relation's field
    Given a customer "cust_fr" in country "FR" with 1 order
    And a customer "cust_de" in country "DE" with 1 order
    When I query orders where "customer.country" = "FR"
    Then the result contains exactly 1 order
    And that order belongs to customer "cust_fr"

  @REQ-0004
  Scenario: Traversing a has-many relation in a predicate is rejected
    When I query orders where "items.quantity" > 5
    Then the query is rejected

  @REQ-0005
  Scenario: Results are ordered and capped
    Given 25 orders for customer "cust_123" with increasing placed_at values
    When I query orders where "customer_id" = "cust_123" ordered by "placed_at" descending limited to 20
    Then the result contains exactly 20 orders
    And the first result has the most recent placed_at
