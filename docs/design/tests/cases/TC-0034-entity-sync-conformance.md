---
id: TC-0034
title: Entity patches scoped bootstrap and explicit reset compose durably
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [TP-0001]
verifies: [SPEC-0031, REQ-0023, REQ-0024, REQ-0025, REQ-0026, REQ-0027, REQ-0029, REQ-0044, THR-0018]
tags: [phase-five, conformance, entity-diff, bootstrap, reset]
---

# TC-0034: Phase 5 acceptance

RFD-0004 policies are operator-approved and adopted by ADR-0033. API-0009 fixed
exact types/signatures and fixture entry points before test code. These scenarios
are the acceptance floor; the execution report below records their actual proof.

```gherkin
Feature: Automatic entity patches share durable command recovery
  Scenario: E01 configured ordinary mutations generate exact patches
    Given a configured entity table and a trusted context provider
    When ordinary create partial update replacement and delete operations commit
    Then generated envelopes have only explicitly written application fields
    And replacement explicitly clears omitted previously present optional fields
    And each carries schema version identity timestamp table operation and record ID
    And no envelope has idempotencyKey or undo images
    And unconfigured tables and command-scope writes generate no extra diffs

  Scenario: E02 admission or transaction failure loses no local or recovery state
    Given missing invalid context or exhausted common queue capacity
    When an entity mutation is attempted
    Then its row ledger counter and comparison metadata remain unchanged
    And the same preservation holds on an injected transaction commit failure

  Scenario: E03 mixed arrays correlate independently without position guessing
    Given one command two edits to one entity and equal record IDs in two tables
    When mixed push responses arrive reordered duplicated or partially missing
    Then immutable writeId table and recordId select exactly the entity write
    And command idempotency keys still select commands
    And ambiguous foreign or conflicting terminal responses reject unchanged

  Scenario: E04 entity retries and separate receipt survive restart
    Given entity diffs persisted before transport dispatch
    When transport fails or receipt arrives without an outcome and the store reopens
    Then unacknowledged retries reuse byte-identical frozen envelopes
    And acknowledged diffs never resend and still recover pending outcomes
    And status and outcome observations identify each diff without exposing payloads

  Scenario: E05 entity dependencies map only declared references
    Given command and entity producers with multiple transitive local-ID dependents
    When authoritative mapping commits
    Then only Reference cells record identities and declared command paths rewrite
    And eligible diffs release in common execution order
    And unknown retired references collisions and interrupted mapping preserve state
    And entity creates from different store namespaces never share a wire record ID
    And an identity-preserving entity mapping resolves its producer
    And stable entity references remain usable after that producer history is evicted

  Scenario: E06 entity rejection inverse and cancellation preserve unrelated edits
    Given queued sent acknowledged rejected and dependent entity writes
    When explicit rollback or cancellation is requested
    Then the existing reverse closure and guarded inverse rules apply
    And conflicting independent edits survive with recovery retained
    And successful inverse restores optimistic comparison metadata with its rows
    And sent cancellation is durable and late outcomes cannot mutate local data

  Scenario: E06 typed corrections commit backend-selected comparison clocks
    Given an entity outcome whose selected values and timestamps differ from optimistic state
    When reconciliation uses typed Entity changes with an identity-preserving mapping
    Then only supplied fields removals and exact authoritative clocks change atomically
    And untouched fields keep their values and comparison clocks
    And supplied backend timestamps override superseded newer optimistic timestamps
    And generic Put or Remove for an entity-origin outcome rejects unchanged
    And an invalid later correction or independent row or metadata edit rejects the whole plan
    And a genuinely renamed old identity rejects while its final identity succeeds
    And an optimistic tombstone may be corrected only by a complete valid row
    And an authoritative tombstone cannot be resurrected
    And subsequent older pull changes cannot overwrite the corrected clocks
    And no secondary outbound diff is generated


Feature: Field resolution preserves authoritative boundaries
  Scenario: E07 distinct field edits and clocks merge exactly
    Given two offline clients modifying distinct fields of one record
    When authoritative resolution and local application complete
    Then both modified fields survive without overwriting untouched fields
    And field comparison state survives a reopen

  Scenario: E08 deletion precedence and missing-row rules are explicit
    Given an authoritative tombstone followed by a later update or create
    When the default resolver is used
    Then the record remains deleted without resurrection
    And an update to an unknown nondeleted row requires a complete valid row
    And tombstone counts remain visible until explicit reset

  Scenario: E09 equal-time winners come only from the server
    Given unequal values with equal field timestamps
    When the client has no authoritative winner
    Then resolution reports AuthorityRequired without choosing a client or device order
    When the backend fixture confirms its first-arrival winner
    Then clients converge to that value in either transport response order

  Scenario: E10 table overrides are isolated validated and bounded
    Given one custom merge table and one default table
    When unequal-time values conflict
    Then only the configured table uses its custom merge
    And invalid oversized or failed hook output commits nothing
    And a hook cannot choose an unresolved equal-time winner or resurrect a tombstone

Feature: Bounded scoped pull and bootstrap recover atomically
  Scenario: E11 opaque checkpoints and membership stay transport-owned
    Given a fresh or previously paged database
    When a pull starts
    Then its checkpoint is null or the exact previous opaque token
    And observing or unsubscribing UI queries does not change the pull request
    And only backend-returned records are applied without client membership inference

  Scenario: E12 page data checkpoint clocks and progress are one commit
    Given a multi-change page and existing queue state
    When validation storage or final commit fails
    Then all rows metadata queue checkpoint and progress remain unchanged
    And no data or progress callback reports a partial page
    When the same page succeeds
    Then all effects become visible together through ordinary observations

  Scenario: E13 bootstrap pages expose partial data and resume after interruption
    Given a first page marked incomplete
    When it commits and native process or browser page restarts
    Then progress identifies partial data and the next request resumes its checkpoint
    When the final page commits
    Then readiness becomes true and all page data is present exactly once
    And an empty page is valid but an incomplete unchanged continuation rejects

  Scenario: E14 suspended pulls compose with local handlers and optimistic recovery
    Given a pull awaiting transport while a handler is suspended
    When its page attempts to apply
    Then it returns Busy and the host retains the response until admission is idle
    And application uses latest local rows rather than an old captured snapshot
    And intervening committed data or comparison revisions make the page StalePull
    And changing a live recovery-protected row returns PullConflict for the whole page
    And insertion with an unresolved producer in that table also returns PullConflict
    And a pending deletion that hides a backend update still returns PullConflict
    And an optimistic field clock that hides an older backend value still returns PullConflict
    And these failures retain the prior checkpoint and progress without events
    And cancelling the local write and refetching can still apply the backend change
    And an obsolete change under a confirmed authoritative tombstone remains a page no-op
    And after outbound resolution a fresh request refetches the unchanged checkpoint
    And only its newly fetched page can commit without data loss
    And cached stale response bytes cannot be rebound to that fresh ticket

  Scenario: E15 malformed oversized foreign and stale pages reject unchanged
    Given bounded pages and one outstanding checked pull ticket
    When a page has invalid schema clocks bounds owner scope or generation
    Then no rows checkpoint or progress change
    And concurrent duplicate or already-consumed pull tickets reject
    And mismatched-scope reopen preserves the original store
    And a valid near-limit page uploads through bounded checked foreign chunks
    And oversized assembly bad offsets or aborted uploads commit no partial page

Feature: Explicit reset is total and cannot replay prior-generation work
  Scenario: E16 default reset preserves all live recovery kinds
    Given queued sent acknowledged inverse-conflict or sent-cancellation recovery
    When PreserveRecovery reset is invoked
    Then reset rejects without changing data recovery progress or subscriptions
    And any active or queued handler makes reset Busy without aborting it

  Scenario: E17 explicit discard reset atomically clears all local state
    Given application rows queue history comparison metadata and partial bootstrap
    When DiscardRecovery reset commits
    Then every application table is empty with its declared schema retained
    And recovery checkpoint comparison state and progress reset together
    And fresh writes never reuse old queue or local record identities
    And reset makes no claim to reverse already-performed backend effects

  Scenario: E18 reset success and failure define exact lifetime boundaries
    Given active data status outcome and progress subscriptions and an awaited pull
    When reset fails at commit
    Then all tokens pending deliveries and data remain usable unchanged
    When reset succeeds
    Then old cached frames subscriptions and transport replies are inert
    And old chunk buffers and old transport completion cannot affect the new sync pump
    And fresh subscriptions see empty data and a fresh pull uses null
    And resolved authoritative deletion tombstones do not prevent default reset

  Scenario: E19 store switching revocation and migration preserve explicit policy
    Given one scoped store and an independently opened second file
    When switching files or backend membership narrows
    Then the original file remains untouched and no automatic purge occurs
    And revoked queued writes still send and surface backend rejection
    And schema migration with recovery or Phase 5 comparison state rejects unchanged
    And ordinary mismatched configuration rejects unchanged
    And explicit reconfiguration succeeds only after reset with no rows or recovery
    And local-only rows after reset prevent reconfiguration
    And a failed reconfiguration preserves its pull token and context provider
    And successful reconfiguration invalidates the prior scope's pull token

  Scenario: E20 real hosts and prior phases retain one production implementation
    Given actual Swift C ABI and main-thread browser worker transports
    When entity push pull suspended handlers restart reset and callbacks execute
    Then shared production Rust and real SQLite or OPFS perform every durable effect
    And no shadow queue server-auth substitute or library network call is introduced
    And Phase 0 through Phase 4 regression gates still pass
```

Mapping: original TC-0011 -> E01/E03/E04/E07–E10; original TC-0013 ->
E11/E13/E16–E19; mixed TC-0014 -> E03/E04; original offline entity write -> E01.
All applicable shared cases execute on memory/native SQLite/real OPFS. E12/E17/
E18 require real SQLite transaction faults; E04/E13/E17 require independent
native processes and browser reloads, not only in-memory reopen. E14/E18/E20
require actual host awaits, callback and cached-frame lifetime checks.

Acceptance report must list exact executed tests/counts and any failed or
unverified boundary. Executed evidence and qualifications are in
[the Phase 5 report](../../../implementation/phase-five-entity-sync.md).
