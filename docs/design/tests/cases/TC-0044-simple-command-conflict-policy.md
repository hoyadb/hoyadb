---
id: TC-0044
title: Stop or discard preserves current data and truthful recovery status
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-27
updated: 2026-11-10
parents: [TP-0001]
verifies: [SPEC-0040, REQ-0017, REQ-0043, DEF-0032]
regression_for: [DEF-0032]
tags: [conflicts, recovery, proposed, compatibility]
---

# TC-0044: Simple conflict policy

The operator approved the reviewed Stop/Discard design and its fail-closed
older-build behavior on 2026-10-27, noting that the library is unreleased.
Implementation and verification remain pending. Task-owned memory and SQLite
fixtures only; no execution is claimed. New symbols or decoder failures are not
behavioral RED.

```gherkin
Feature: Configure two understandable reactions
  Scenario: CP01 default and reopen are Stop
    Given no conflict policy is configured
    Then a direct-edit inverse mismatch returns RollbackConflict without a change
    And after configuring Discard and reopening the policy is Stop again

  Scenario: CP02 exact command override wins
    Given a Discard default and a Stop override for edit-v1
    Then edit-v1 stops while another eligible command keeps current data
    And the opposite default and override combination behaves symmetrically
    And lookup uses the exact recorded command name and unknown command overrides are accepted and inert
    And malformed values names or oversized configuration leave the previous policy intact

  Scenario: CP03 normal outcomes and non-conflict errors keep their meaning
    Then accepted repeated edits and waiting same-row commands follow SPEC-0038
    And an unchanged rejected row rolls back normally under either policy
    And network Busy Closed capacity size and protocol errors are never discarded

Feature: Keep local data without claiming an undo
  Scenario: CP04 rejected command followed by a direct edit
    Given a processed rejection and a later direct edit
    When rollback runs under Discard
    Then every row stays byte-identical and the record becomes discarded
    And rolled_back is false the Rejected outcome is retained and the RD blocker is released
    And resolving and unit methods share success errors and report preflight

  Scenario: CP05 correction keeps data and valid mappings
    Given a valid correction whose own inverse proof mismatches
    When its policy is Discard
    Then mappings apply and all plan writes are skipped atomically
    And status remains Applied or Corrected and reconciled with no convergence claim
    And KeptCurrent has no discarded keys no unmapped IDs and the exact skipped count

  Scenario: CP06 manual unsent discard and dependent closure
    Given an unsent producer and transitive eligible dependents
    When discard is called regardless of configured policy
    Then all eligible records retire together and all current rows remain
    And the report lists exactly newly discarded keys and unmapped created IDs
    And subsequent new references to unmapped local IDs return UnknownLocalReference
    And already rolled-back or discarded records are not relabelled
    And each newly discarded Queued processed Rejected or never-sent unresolved Cancelled record has state Discarded reconciled true and rolled_back false
    And only those flags state reference_paths and dependencies change while args created undo frozen attempts due_ms outcome and mappings equal their pre-action values
    And reference_paths and dependencies are empty and normal ledger validation succeeds

  Scenario: CP06b never-sent Cancelled recovery can be retired
    Given cancel kept recovery data after an inverse conflict on an unsent command
    Then explicit discard keeps the rows and retires that Cancelled root
    And a Cancelled root or descendant with frozen bytes and no stored outcome is refused
    And a processed rejection left Cancelled by a conflicted cancel keeps its stored Rejected outcome and stays discardable

  Scenario: CP07 in-flight work cannot be unsent
    Given a Sent Acknowledged or Cancelled-after-send descendant
    Then the entire discard is refused without any change
    And a Sent Acknowledged Applied Corrected or Cancelled-after-send root returns Protocol
    And public root cases cover those states without changing rows or ledger
    And strict ledger validation rejects a frozen descendant retaining an unresolved created-ID dependency
    And a pure defensive closure-check case refuses that inadmissible descendant without claiming it is publicly reachable

  Scenario: CP08 later recorded work remains protected
    Given a live record outside the dependent closure wrote an affected row
    Then discard refuses and the current rows and all records remain unchanged
    And resolving the later work first permits an otherwise eligible action

  Scenario: CP08b correction conflicts also protect later commands
    Given a later queued command wrote the earlier correction's affected row
    Then Discard refuses and changes nothing
    And the X1 cancel-correct-reissue path still resolves both intents

  Scenario: CP09 entity records and metadata are never bypassed
    Given an entity descriptor or an undo row on an entity-managed table or metadata_undo
    Then the entire action refuses without changing rows timestamps or recovery

  Scenario: CP10 stale guards and invalid mappings never trigger policy
    Given a stale expectation protected unrelated row unused guard or invalid mapping
    Then the original SPEC-0039 error occurs under either policy
    And no write or correction is silently skipped
    And a correction-discard still commits every caller expectation as a storage RowGuard
    And an intervening guard race returns RollbackConflict without committing any change and never re-enters policy

Feature: Commit truth and reports together
  Scenario: CP11 bounded report precedes persistence
    Given an eligible action whose encoded result exceeds the existing frame limit
    Then LimitExceeded leaves rows mappings model records and notifications unchanged
    And unit-returning methods have the identical result preflight

  Scenario: CP12 restart and transaction failure
    Then a successful SQLite discard survives restart with exact status and data
    And a real transaction failure leaves the prior ledger and rows after reopen
    And no partial dependent closure or status notification is published

  Scenario: CP13 SDK reports and lifecycle
    Then public and managed operations forward KeptCurrent to onResolution once
    And a failing callback cannot mask the committed result
    And onResolution remains installed and each failing invocation reports once as callback CallbackFailure with token null through onBackgroundError on every SDK
    And those reports are suppressed after close or reset and sink failures are contained without invoking startSync onError
    And Kotlin model sink and decoder tests remain source-only and not-run while JVM and Android execution are held
    And failed configuration or close during configuration disposes the new owner
    And reconfiguration during an admitted handler returns Busy without changing policy

  Scenario: CP14 fail-closed downgrade and opt-out
    Given a real old native artifact built from the exact policy base immediately before Discarded with its source lock toolchain command host and artifact hashes recorded
    And the new build creates closed task-owned protected stores of the same schema and format with and without discarded history
    When the old artifact opens the store without discarded history
    Then that positive control succeeds or the refusal probe is invalid
    When the old artifact opens the discarded store
    Then it refuses with its actual error kind recorded and the positive control proves this is not a format failure
    And database and WAL content hashes are unchanged with any SHM appearance or change inventoried separately
    And the new binary reopens it with truthful status
    And setting Stop never pretends that existing discarded history has disappeared
    And trimming has no time-based downgrade guarantee
  Scenario: CP15 discarded records have explicit operation semantics
    Then rollback cancel reconcile and request_redelivery return Protocol
    And an identical stored Rejected outcome is a no-op
    And a conflicting outcome or any outcome for a never-sent discard returns Protocol
    And pending reconciliation excludes it while forget and trim use non-live rules
    And receipt-only None is a byte-identical no-op on a sent discard and Protocol on a never-sent discard
    And discarding retains a sent Rejected outcome without a new Outcome event while an unsent discard has status only
    And created entries undo and frozen bytes remain until the record itself is removed
    And the special never-sent Cancelled history cleanup never clears those fields on a Discarded record

  Scenario: CP15a discarded producer retention is independent of dependent history
    Given an unsent producer P creates a local ID x and a later unsent dependent D references x
    And history is 1 with P older than D and the pre-action ledger is valid
    When discard P retires the eligible closure atomically
    Then trimming drops P and keeps D with its args created undo frozen attempts due_ms and outcome preserved and reference_paths and dependencies empty
    And all current rows are byte-identical and persistence and ledger validation succeed
    And strict SQLite reopen through a fresh connection reads the same retained history and rows
    And the memory case reopens its owner and is labelled as owner reopen only
    And the same results hold for a processed Rejected P with an unsent dependent when configured rollback uses Discard

  Scenario: CP15b forgetting a discarded producer leaves valid history
    Given discarded P and D are both retained with enough history capacity
    Then forgetting P succeeds while D remains byte-identical and the ledger stays valid after reopen
    And forgetting D subsequently succeeds
    And this holds for both unsent P and processed Rejected P discarded by configured rollback
    And new references to P's unmapped local ID return UnknownLocalReference with no row or ledger change both while P is retained and after P is forgotten or trimmed
    And a later insert without reset receives a fresh local ID different from P's removed ID

  Scenario: CP15c mapped references and validation keep their existing meaning
    Given a valid mapped producer and an eligible dependent whose args already contain the mapped remote ID
    When the dependent is discarded
    Then its remote argument and the producer's mapping stay unchanged and removing the non-live producer leaves valid retained history after reopen
    And a non-Discarded record with a dangling local reference still fails normal ledger validation
    And a Discarded record with nonempty reference_paths or dependencies fails its legal-state validation
    And removing either bookkeeping-clearing step is a killed mutation of CP15a

  Scenario: CP15d dependent-only discard does not block a later producer mapping
    Given live producer P and unsent dependent D write separate rows and D references P's unmapped local ID
    When only D is manually discarded before P is confirmed with a valid mapping
    Then D's bookkeeping is cleared by the discard and the discard itself keeps every current row
    And later confirmation of P succeeds without reactivating relabelling or dispatching D
    And the ordinary global mapping rewrites current row references and D's historical undo identities and Reference values exactly as before
    And D's args frozen bytes and every other field outside that existing undo rewrite stay unchanged after confirmation
    And the ledger validates and strict fresh-connection SQLite reopen preserves the exact result

  Scenario: CP15e dependent-only discard stays outside a later producer rollback
    Given live producer P and unsent dependent D write separate rows and D references P's unmapped local ID
    And D was manually discarded and its exact record and kept row were captured
    When P is processed as Rejected and its ordinary rollback completes
    Then the closure excludes D and P's created row is removed normally
    And D's row and entire Discarded record stay byte-identical with no inverse or relabelling of D
    And its Reference may still name P's removed row under the existing Reference typing rule
    And persistence ledger validation and fresh-connection SQLite reopen succeed
    And the memory cases use labelled owner reopen only

  Scenario: CP16 cancel ignores configured conflict policy
    Given Discard is configured and cancel encounters an inverse mismatch
    Then cancel retains its existing Cancelled recovery behavior
    And no record is automatically discarded
    And rollback of a Cancelled root whose inverse mismatches returns RollbackConflict under either policy
```
