---
id: TC-0046
title: Original-style JavaScript application workflow uses the real core
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-28
updated: 2026-10-28
parents: [TP-0001]
verifies: [SPEC-0042, API-0012, REQ-0046, REQ-0047]
tags: [javascript, application-api, regression]
---

# TC-0046: JavaScript application workflow

```gherkin
Feature: Use the friendly binding without replacing core behavior
  Scenario: APP01 schema builders and original query syntax
    Given numeric-version tables, optional fields and all relation kinds
    When plain values and symbolic predicates are used
    Then declarations encode to existing DTOs and real core queries return flat rows
    And per-column Number/bigint and Date/bigint policies remain fixed at 2^53 boundaries
    And signed 64-bit integers, timestamps, null and absence remain exact
    And a stored value outside its column policy fails the whole query with a typed error
  Scenario: APP02 simple open and secure persistence
    Given browser or Node imports with no manually constructed Facade
    When openDatabase uses memory or protected filename options
    Then it opens the real adapter with one shared runtime
    And a missing protected key rejects before any storage open
    And the advanced tagged host API remains unchanged even with a numeric schema version
  Scenario: APP03 observer lifetime
    Given a query returning plain rows
    When rows change, or unsubscribe happens before registration completes
    Then ready resolves after early disposal or rejects with the typed registration error
    And callbacks see committed plain snapshots only
    And unsubscribe is idempotent and no later callback runs after close
  Scenario: APP04 named scoped handler
    Given one definition bound explicitly to each of two databases
    When execute awaits reads and writes, or throws after a staged write
    Then result is the handler value with local IDs after one atomic commit
    And a throw restores data and queue with no notification
    And scopes cannot execute nested commands or change captured identity
    And outer writes during an admitted handler return Busy
  Scenario: APP05 dependent writes and authority
    Given a command references a producer through declared reference paths
    When push and explicit mapped reconciliation finish
    Then the dependent waits, then sends with the mapped remote reference
    And transport failures preserve frozen retry work and reopen recovery
    And retained outcomes reconcile after reopen before any command definition is bound
    And app pending exposes persisted kind and created IDs without changing the tagged pending DTO
    And an entity create with an empty app mapping receives its exact identity mapping after reopen
    And partial omissions fill missing entity-created IDs only, preserving every supplied mapping
    And command mappings are never filled automatically
    And the projection returns only kind and created, refusing unknown/non-pending keys with Protocol
    And undeclared ID-like strings are not guessed as references
  Scenario: APP06 records and entity opt-in
    Given explicit entity tables and an unconfigured local table
    When a record handle performs partial update or remove
    Then configured writes use entity queue and others retain local semantics
    And lazy relations and field/method name collisions are handled correctly
    And an interleaved write cannot be overwritten by partial update
  Scenario: APP07 object transport and fake backend
    Given plain command and entity outcome handlers
    When transport responses pass through core codecs
    Then exact correlation and large integers survive
    And callback, malformed input and protected data never bypass validation
  Scenario: APP08 automatic pull lifecycle
    Given configured entities and transport.pull
    When sync starts, retries or its owner closes while a response is pending
    Then one pull runs at a time using the retained checkpoint
    And stop, close and reset stop timers and suppress stale publication
    And Busy is skipped without reporting while real failures use the background sink
    And one conflicting background reconciliation is reported once while later records still reconcile
    And close/reset await cancelled in-flight push and pull work
    And advanced tagged startSync remains push-only
  Scenario: APP09 teach the complete binding workflow
    Given the generated getting-started page
    Then its first code shows public binding imports and database opening
    And schema, query, observer, command, local handler and backend are explained in order
    And code regions come from the same executed application file
    And missing, duplicate or nested snippet regions fail the build
    And all previous examples, package types and local links still pass
```
