---
id: TC-0043
title: Cross-row corrections require exact caller expectations
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-27
parents: [TP-0001]
verifies: [SPEC-0039, REQ-0043, DEF-0038]
regression_for: [DEF-0038]
tags: [purpose-audit, corrections, atomicity, regression]
---

# TC-0043: Cross-row correction guards

Proposed cases before implementation. Synthetic task-owned stores only. Core
cases run on memory and SQLite; facade and SDK cases use their production codecs.
No execution is claimed here. GW01 and GW02 are behavioral RED on existing
operations; a missing symbol or import is never RED. Other new-operation cases
run after the additive entry point exists and verify its required semantics.

```gherkin
Feature: Authoritative effects cannot overwrite unseen unrelated edits
  Scenario: GW01 unguarded Put cannot lose a later edit outside the command
    Given command A writes r and a later direct local write edits s
    And A receives Corrected
    When ordinary reconcile supplies a Put for existing s without a guard
    Then it returns RollbackConflict and preserves all rows mappings ledger and events
    And this fails on unchanged code because reconciliation overwrites s and returns Ok

  Scenario: GW02 unguarded Remove has the same protection
    Given GW01 with a later local edit made by another command instead
    When the correction Removes s without a guard
    Then it returns RollbackConflict and s and both recovery records remain unchanged

  Scenario: GW03 an explicit exact expected image permits the server effect
    Given s has public fields x and A's optimistic rows still match their proof
    When reconcile_guarded supplies expected x and a Put for s
    Then the correction and A's mapping and reconciliation commit atomically
    And expected x also permits a guarded Remove in a separate fixture
    And omitted optional fields and explicit Null are compared as distinct public maps

  Scenario: GW04 an intervening local edit invalidates a supplied guard
    Given the application captured expected x for s
    And a later write changes s before the guarded plan is applied
    Then reconciliation returns RollbackConflict without changing any row or record
    And it never refreshes the guard and retries automatically

  Scenario: GW05 absent-row and ordered-write controls
    Given a plan inserts an absent unrelated s or removes an absent s
    Then an absent-row expectation is required and matches only while s is absent
    And an expected-absence guard conflicts if s exists
    And multiple ordered writes to s use one original before-image
    And a later unrecorded remove cannot be resurrected by an unguarded insert
    And a no-op absent Remove uses its guard and retains the storage absence guard through commit
    And on both adapters a Put identical to the current row retains its expectation without a mutation or data notification

  Scenario: GW06 exact codec and guard validation
    Given duplicate unused malformed unknown-table wrong-type or reserved-field guards
    Then each is rejected with the specified InvalidCommand or existing decode error
    And invalid guard structure takes precedence over a separate missing or stale expectation
    And no mapping data ledger or event change occurs

  Scenario: GW07 identity rewriting does not misclassify owned rows
    Given A creates local:x and maps it to srv
    Then a correction to srv remains inside A's write set
    And a guard for unrelated s compares its original reference values before the rename
    And failure preserves both original identities and every reference
    And a guard supplied for local:x or its mapped closure-owned target srv is unused and returns InvalidCommand

  Scenario: GW08 failed storage apply and restart
    Given a valid guarded plan and a real adapter transaction failure
    Then data mappings ledger and events remain at their prior state
    And reopening SQLite shows the same state
    And a successful guarded plan survives restart with its exact rows

  Scenario: GW09 existing reconciliation controls remain exact
    Then SPEC-0038 no-write confirmations and row-causal dispatch are unchanged
    And nonempty command-closure and entity correction proofs are not relaxed
    And facade and SDK fields preserve finite floats integers null and references

  Scenario: GW10 an expectation cannot overwrite another live command
    Given a live command E outside A's closure writes or removes s
    When A's plan writes s with a matching value or absence guard
    Then it returns RollbackConflict and preserves E and its optimistic result
    And E's later confirmation cannot silently accept an overwritten value

  Scenario: GW11 ordinary cross-row writes cannot bypass entity metadata
    Given s is in an entity-managed table outside A's closure
    When A supplies a non-entity Put or Remove for s with an exact guard
    Then it returns RollbackConflict and preserves the row and comparison metadata
    And existing typed entity correction controls remain unchanged

  Scenario: GW12 typed cross-row corrections cannot bypass protection either
    Given a later entity write E changes s outside A's closure
    When A supplies a typed Entity Update correction to s
    Then it returns RollbackConflict and preserves E's row and metadata
    And typed Entity Create and Delete corrections outside the closure each return RollbackConflict
    And each refusal preserves all rows comparison metadata recovery records and notifications
    And a matching ordinary row guard does not permit the typed cross-row write
  Scenario: GW12b typed corrections to non-entity-configured tables are also protected
    Given an entity ledger exists and declared table t is not entity-configured
    And a command closure does not own the target row in t
    When its correction supplies a typed Entity Update with an exact guard
    Then it returns RollbackConflict with all rows metadata ledger and notifications unchanged
    And a separate typed Entity Create for an absent target with an absence guard has the same result
    And both adapters cover the cases
    And removing only the typed outside-closure refusal makes these cases fail

  Scenario: GW13 storage supports read-only expectation guards atomically
    Given a valid SyncPlan with a guard on an application row and no mutation for that row
    Then a matching expected image commits and a mismatching image returns the existing guard conflict
    And an expected-absence guard on an absent managed row commits without creating a row
    And a mismatch preserves all application rows model recovery records and events
    And a guard for an unknown table is rejected even when expected is absent
    And duplicate guards and unguarded mutations remain invalid
    And Reset still requires guards and removals for exactly the whole actual inventory
    And the contract unit tests and both adapter conformance suites cover these cases
    And a reserved-table guard is rejected even when expected is absent

  Scenario: GW14 no-op guards close an Unproven adapter race
    Given an Unproven wrapper changes a guarded row between the planner snapshot and atomic apply
    When the guarded plan has an equal-value Put or an absent-row no-op Remove
    Then the stale expectation returns RollbackConflict and commits nothing
    And a core planner that drops a mutation while retaining its derived guard returns Protocol
    And only explicit caller expectations may add guards without mutations

  Scenario: GW15 SDK guard helpers translate losslessly
    Given an open JavaScript Swift or Kotlin SDK with a scripted core channel
    When reconcileGuarded receives one existing-row guard with an omitted optional field and an explicit Null cell and one absence guard
    Then exactly one reconcile_guarded operation carries key, the plan encoded as reconcile encodes it and guards in caller order
    And the expected maps keep the omitted field absent, the Null cell present and absence as null
    And mutating the caller's guard or plan after the call starts does not change the sent frame
    And core RollbackConflict, InvalidCommand, Protocol and Busy are returned unchanged with no retry and no reconcile call
    And on success observation delivery is scheduled and a started managed driver is woken, as reconcile does
    And on failure neither is scheduled
    # Compile or public-signature seams are not behavioral RED; assert operation and result.

  Scenario: GW16 SDK guards against the real core
    Given the corrected Rust framework or WASM rebuilt into task-owned fixtures
    When GW01, GW03 and GW04 run through each executed SDK's public helpers after pendingReconciliations
    Then the unguarded plan conflicts, the exact expectation commits and a stale expectation conflicts with rows unchanged
    And a managed reconcile callback returning the unguarded plan reports one sync failure for that key per pass and leaves the outcome pending
    And a later unrelated pending entry is reconciled in the same pass on both one-page and multi-page runs
    And the conflicted key is not retried or mapped again within that pass and skipping it alone schedules no wake or timer
    And callback failures and non-RollbackConflict core errors retain their existing pass failure behavior
    And row-causal work blocked by the unresolved key stays withheld
    # JavaScript/WASM and Swift macOS only; Kotlin and Android stay static while held.

  Scenario: GW16b managed conflict skips do not self-loop
    Given a scripted driver with no outgoing envelopes, no due deadline and only pending X
    When X's core reconcile returns RollbackConflict
    Then X has exactly one callback, one reconcile and one sync failure report
    And the driver finishes with no additional prepare_push, pass, wake or timer
    When instead pending X conflicts and unrelated pending Y succeeds once
    Then the existing success wake in JavaScript Swift and Kotlin causes exactly one further pass
    And X's callback, reconcile and report each occur twice total, once per pass, with no activity after that second pass
    # deterministic scripted scheduling controls; Kotlin source-only while held

  Scenario: GW17 only the exact storage guard conflict is translated
    Given a guarded reconciliation reaches atomic storage apply
    When storage returns a non-guard SchemaConflict such as command model changed before commit
    Then the result remains Storage SchemaConflict with no data ledger metadata or notification change
    And the exact API-0007 GUARD_CONFLICT alone maps to RollbackConflict for reconcile
    And a mutation translating every SchemaConflict fails this case

  Scenario: GW18 global-owner fixtures are isolated under the default test runner
    Given the guard-race cases use Unproven adapters sharing one process-global coordinator
    When GW05 GW14 and GW17 run together with the normal parallel test harness
    Then every case reaches its own guard assertions without another case causing Busy during setup
    And a test-only shared poison-tolerant mutex isolates each entire case including teardown
    And the deliberate two-handle race inside GW14 and all state and notification assertions are unchanged
    And ten focused default-runner repeats pass on each adapter without thread-count flags
    And every other multi-case Unproven test binary is listed as serialized or single-global-case and any missing isolation is fixed
    # RED: concurrent guard-race setup unwraps Busy; no production coordination change

```
