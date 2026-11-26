# id: TC-0015
# verifies: REQ-0030, REQ-0031, THR-0002
# doc: TC-0015-at-rest-protection.md

@REQ-0030 @THR-0002
Feature: Protecting the local store at rest
  As a developer integrating hoyadb
  I want the local file unreadable outside my app's runtime
  So that a stolen device or extracted backup does not expose synced data

  Background:
    Given a storage engine providing an encrypted-storage capability
    And the walkthrough schema

  Scenario: The store is encrypted using the engine's own capability
    When I open a database supplying encryption key material
    And I write an order with status "pending"
    Then the raw file on disk does not contain the plaintext "pending"
    And the encryption was performed by the concrete engine adapter

  Scenario: Encryption is invisible above the storage interface
    Given an encrypted database
    When I query orders where "status" = "pending"
    Then the query behaves identically to an unencrypted database
    And no layer above the storage interface referenced encryption

  Scenario: hoyadb generates and stores no key of its own
    When I open a database supplying encryption key material
    Then hoyadb generated 0 encryption keys
    And hoyadb persisted the key in 0 locations
    And the key material came solely from the configuration entry I supplied

  Scenario: Opening without the correct key fails rather than falling back
    Given an encrypted database created with one key
    When I open it supplying a different key
    Then the open fails
    And hoyadb does not fall back to unencrypted access

  @REQ-0031
  Scenario: The local store is excluded from platform backups by default
    When I open a database without configuring backup behaviour
    Then the local store is marked excluded from platform device and app backups

  @REQ-0031
  Scenario: An integrator can opt back into platform backups
    When I configure excludeFromBackups false
    And I open the database
    Then the local store is not marked excluded from platform backups
