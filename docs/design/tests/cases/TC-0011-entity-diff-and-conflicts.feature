# id: TC-0011
# verifies: REQ-0023, REQ-0024, REQ-0025
# doc: TC-0011-entity-diff-and-conflicts.md

@REQ-0023
Feature: The plain write path and how its conflicts resolve
  As a developer integrating hoyadb
  I want plain table writes without designing a command
  So that tables with no domain logic stay low-ceremony but not less secure

  Background:
    Given a database opened against the in-memory engine
    And the walkthrough schema
    And a fake transport capturing the payloads it is given

  Scenario: An ordinary table mutation generates an entity-diff payload
    Given an order_notes record "note_789"
    When I update that record's body to "Customer requested gift wrap"
    And the queue is pushed
    Then the captured payload has table "order_notes", op "update" and recordId "note_789"
    And I never constructed that payload by hand

  Scenario: The payload carries only the fields actually written
    Given an order_notes record "note_789" with a body and an author field
    When I update only its body
    And the queue is pushed
    Then the captured payload's changes contain only "body"
    And the captured payload's changes do not contain "author"

  Scenario: An entity-diff carries no idempotency key
    When I update an order_notes record and the queue is pushed
    Then the captured payload has no idempotencyKey field
    And its response is correlated by recordId instead

  Scenario: An entity-diff goes through the same queue as a command
    Given every transport call fails
    When I update an order_notes record
    Then the write is persisted in the queue with state "queued"
    And it is retried the same way a command is

  Scenario: An entity-diff gets the same authorization bar as a command
    Given the fake transport will reject the write with reason "not authorized"
    When I update an order_notes record and the queue is pushed
    Then the observed outcome status is "rejected"
    And the rejection reason is "not authorized"

  @REQ-0024
  Scenario: Per-field last-write-wins preserves both sides' distinct fields
    Given two offline clients each update a different field of order_notes "note_789"
    When both writes are resolved
    Then client A's field change survives
    And client B's field change survives
    And neither client's untouched fields were discarded

  @REQ-0024
  Scenario: A delete beats a later update to the same record
    Given client A deletes order_notes "note_789"
    And client B afterwards updates that record's body
    When both writes are resolved
    Then the record remains deleted

  @REQ-0024
  Scenario: A table may override the default resolution
    Given the order_notes table declares its own resolution logic
    When two conflicting writes to that table are resolved
    Then the table's own logic is applied instead of per-field last-write-wins

  @REQ-0025
  Scenario: The client attaches a timestamp but breaks no tie itself
    When I update an order_notes record and the queue is pushed
    Then the captured payload has a clientTimestamp
    And hoyadb performed no local tie-break between competing writes
