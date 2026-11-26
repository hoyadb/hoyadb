# id: TC-0012
# verifies: REQ-0015, REQ-0028, REQ-0032, THR-0001, THR-0007, THR-0008
# doc: TC-0012-boundary-identity-and-egress.md

@REQ-0015
Feature: The boundary hoyadb does not cross
  As a developer integrating hoyadb
  I want hoyadb to carry identity and nothing else
  So that authentication, the network and telemetry stay mine

  Background:
    Given a database opened against the in-memory engine
    And the walkthrough schema
    And a trusted-context extension point returning identity "identity-abc"
    And all outbound network access from hoyadb's own code is monitored

  @THR-0001
  Scenario: The trusted context is forwarded unmodified
    When I execute "add-order-v1" and the queue is pushed
    Then the captured envelope's context identity is exactly "identity-abc"
    And hoyadb did not inspect or validate that value

  @THR-0001 @THR-0008
  Scenario: A command handler cannot reassign the identity
    Given a command handler that attempts to assign ctx.identity
    When I execute that command
    Then the assignment is rejected
    And the envelope's context identity is still "identity-abc"

  Scenario: hoyadb implements no authentication of its own
    When I open a database with no trusted-context extension point configured
    Then hoyadb does not attempt to obtain an identity itself
    And hoyadb performs no authentication handshake

  @REQ-0028
  Scenario: hoyadb opens no network connection
    When I execute commands, update records, and push and pull the queue
    Then hoyadb's own code opened 0 network connections
    And every send was performed by the developer-supplied transport

  @REQ-0028
  Scenario: The transport is the only egress path
    Given a transport that records every call it receives
    When the queue is pushed
    Then the transport's push was called with the envelope array
    And hoyadb made no request of its own alongside it

  @REQ-0032 @THR-0007
  Scenario: No telemetry leaves the device
    When I execute several commands and read the queue status
    Then hoyadb sent 0 telemetry payloads to any external destination
    And hoyadb sent 0 diagnostics to any external destination

  @REQ-0032
  Scenario: Command contents are retained only in local state
    When I execute "add-order-v1" with customer "cust_123"
    Then the command's args are present in hoyadb's local queue state
    And those args were not written to any destination outside the local store
