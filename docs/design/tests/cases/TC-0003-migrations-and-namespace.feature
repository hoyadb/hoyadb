# id: TC-0003
# verifies: REQ-0006, REQ-0033, NFR-0005
# doc: TC-0003-migrations-and-namespace.md

@REQ-0006
Feature: Schema migration and reserved naming
  As a developer shipping a new version of my app
  I want a predictable migration outcome
  So that I never get a partial upgrade or a guessed migration

  Scenario: An authored chain applies in order and preserves data
    Given a local database at schema version 1 containing 3 orders
    And migrations authored for 1 to 2 and 2 to 3
    When I open the database with a declared schema at version 3
    Then both migration steps are applied in order
    And the local database is at schema version 3
    And the 3 orders are still present

  Scenario: A missing chain resets the local database rather than guessing
    Given a local database at schema version 1 containing 3 orders
    And no migration chain reaching version 4
    When I open the database with a declared schema at version 4
    Then the local database is recreated at schema version 4
    And the local database contains 0 orders
    And no partial-upgrade state remains

  Scenario: A migration step is never inferred
    Given a local database at schema version 1
    And no migration chain reaching version 2
    When I open the database with a declared schema at version 2
    Then no inferred migration step is applied
    And the local database is recreated at schema version 2

  @REQ-0033 @NFR-0005
  Scenario: A schema declaring a reserved name is rejected
    When I declare a schema containing a table named within hoyadb's reserved namespace
    Then the schema declaration is rejected
    And hoyadb's own bookkeeping state is unchanged

  @REQ-0033
  Scenario: hoyadb's own bookkeeping does not collide with an application table
    Given the walkthrough schema
    When I inspect the tables hoyadb maintains for its own bookkeeping
    Then every one of them is named within the reserved namespace
    And none of them collides with an application-declared table name
