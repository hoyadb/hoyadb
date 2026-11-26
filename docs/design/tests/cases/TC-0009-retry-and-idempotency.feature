# id: TC-0009
# verifies: REQ-0020, REQ-0013, THR-0003
# doc: TC-0009-retry-and-idempotency.md

@REQ-0020
Feature: Retrying unacknowledged writes
  As a developer integrating hoyadb
  I want retries that a backend can recognize as retries
  So that a lost response does not become a duplicate order

  Background:
    Given a database opened against the in-memory engine
    And the walkthrough schema
    And a fake transport whose responses I control

  @REQ-0013 @THR-0003
  Scenario: A retry resends the identical envelope with the same key
    Given I executed "add-order-v1" and its push received no response
    When the write is retried
    Then the resent envelope has the same idempotencyKey as the original
    And no new idempotency key was generated
    And the resent envelope is otherwise byte-identical to the original

  Scenario: Retries fire automatically when a new command is issued
    Given a write is sitting unacknowledged
    When I execute a new command
    Then the unacknowledged write is attempted again

  Scenario: Retries can be requested explicitly
    Given a write is sitting unacknowledged
    When the developer calls the retry API
    Then the unacknowledged write is attempted again

  Scenario: Retries back off exponentially with jitter
    Given a write whose push keeps failing
    When it is retried several times
    Then each successive delay is longer than the previous one
    And the delays are not a fixed interval
    And the delays vary between two clients retrying the same write

  Scenario: Only still-unacknowledged writes are resent
    Given a pushed array of 3 writes where 2 were acknowledged and 1 was not
    When the retry fires
    Then only the unacknowledged write is resent
    And the 2 acknowledged writes are not resent
