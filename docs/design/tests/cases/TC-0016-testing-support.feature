# id: TC-0016
# verifies: REQ-0034
# doc: TC-0016-testing-support.md

@REQ-0034
Feature: Testing an app built on hoyadb
  As a developer writing tests for my app
  I want hermetic, deterministic hoyadb behaviour
  So that my tests exercise real behaviour without a disk or a network

  Scenario: The in-memory engine touches no filesystem or platform storage
    When I open a database against the in-memory engine
    And I write and read an order
    Then no filesystem path was created
    And no platform storage API was called

  Scenario: Behaviour above the storage interface is identical to a real engine
    Given the walkthrough schema
    When I run schema declaration, a relation navigation, a subscription and a command handler against the in-memory engine
    Then each behaves identically to the same operation against an on-disk engine

  Scenario: The fake transport implements the same contract as a real one
    Given a fake transport with a handler for "add-order-v1"
    When I execute "add-order-v1" and the queue is pushed
    Then the fake's push was called with an envelope array
    And it returned an array of wire outcomes
    And hoyadb treated the result exactly as it would a real transport's

  Scenario: The fake attaches the correlation field automatically
    Given a fake transport whose handler returns only business fields
    When I execute "add-order-v1" and the queue is pushed
    Then the outcome hoyadb received carries that envelope's idempotencyKey
    And the handler author did not have to attach it

  Scenario: A rejection is simulated by the handler's return value
    Given a fake transport whose handler for "add-order-v1" returns status "rejected"
    When I execute "add-order-v1" and the queue is pushed
    Then the observed outcome status is "rejected"

  Scenario: A dropped ack is simulated by the push rejecting
    Given a fake transport whose push rejects
    When I execute "add-order-v1" and the queue is pushed
    Then the command remains in state "sent"
    And it is eligible for retry

  Scenario: An injected clock makes a handler's timestamp deterministic
    Given a database opened with a clock fixed at "2026-09-15T12:00:00Z"
    When I execute "add-order-v1"
    Then the created order's placed_at is exactly "2026-09-15T12:00:00Z"

  # Historical wording retained; explicitly superseded by ADR-0035 on2026-10-24.
  @historical_superseded_by_ADR_0035
  Scenario: An injected id generator makes local ids deterministic
    Given a database opened with an id generator returning "test-fixed-id-001"
    When I execute "add-order-v1"
    Then the created order's local id is "test-fixed-id-001"

  Scenario: Approved command id injection preserves exact local recovery identity
    Given a database opened with an id generator returning "local:test-fixed-id-001"
    When I execute "add-order-v1"
    Then the created order's local id is exactly "local:test-fixed-id-001"
    And the surviving command record remains valid for recovery

  Scenario: Approved ordinary CRUD id injection retains the original exact value
    Given an unconfigured database opened with an id generator returning "test-fixed-id-001"
    When I insert an order through ordinary CRUD
    Then the created order's local id is exactly "test-fixed-id-001"
