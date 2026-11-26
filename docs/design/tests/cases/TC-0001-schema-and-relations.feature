# id: TC-0001
# verifies: REQ-0001, REQ-0002
# doc: TC-0001-schema-and-relations.md

@REQ-0001
Feature: Declaring a schema and its relations
  As a developer integrating hoyadb
  I want to declare tables, typed columns and real relationships
  So that my data model is hoyadb's own and relations are first-class

  Background:
    Given a database opened against the in-memory engine
    And the walkthrough schema declaring "customers", "orders", "order_items", "products" and "order_notes"

  Scenario: A declared table exists with its typed columns and an implicit id
    When I insert an order with status "pending" and total_cents 4200
    Then the stored record has status "pending"
    And the stored record has total_cents 4200
    And the stored record has a non-empty "id" field that I did not declare

  Scenario: An undeclared table is rejected, not created implicitly
    When I query the table "invoices"
    Then the operation is rejected
    And no table "invoices" exists in the database

  @REQ-0002
  Scenario: A to-one relation is navigable without writing a join
    Given a customer "cust_123" in country "FR"
    And an order belonging to customer "cust_123"
    When I navigate from that order to its customer
    Then I receive a query object rather than an already-materialized record
    And resolving it yields the customer "cust_123"

  @REQ-0002
  Scenario: A has-many relation is navigable from the other side
    Given a customer "cust_123" with two orders
    When I navigate from that customer to its orders
    Then resolving it yields exactly 2 orders

  @REQ-0002
  Scenario: A many-to-many relation is navigable from both sides
    Given a schema declaring orders and products joined through "order_products"
    And an order linked to two products
    When I navigate from that order to its products
    Then resolving it yields exactly 2 products
    And navigating from one of those products to its orders yields that order
