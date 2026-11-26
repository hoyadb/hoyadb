---
id: DEF-0065
title: Pulled delete of an unseen identity blocks a pending create's mapping
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-10
updated: 2026-11-10
parents: [API-0009]
affects: [API-0009, API-0006, REQ-0017]
regression_for: [TC-0034]
tags: [sync, pull, review, regression]
---

# DEF-0065: Pulled delete of an unseen identity blocks a pending create's mapping

## Symptom

A pulled `Delete` for a remote identity the device had never seen was not treated as protected: only insertion counted for unmapped local-ID producers. The page committed an authoritative tombstone at that identity. A later `reconcile` mapping the pending create to it was refused as an occupied target on every attempt, and cancel, rollback, discard and forget all refused the Applied record, leaving recovery reset as the only exit.

## Acceptance

A page that would give an absent row its first tombstone in a table with an unmapped live producer returns `PullConflict` and commits nothing, on memory, SQLite and encrypted SQLite.

## Root cause and fix

`PullPlanner::advance` counts an absent row gaining its first tombstone as insertion for unmapped producers, per API-0009 section 4 (lost-outcome mapping collisions). `command::pull::tests::e14_pulled_delete_of_unseen_identity_cannot_occupy_a_pending_mapping_target` failed with `Ok(())` before the change. The shared `pull_tombstone_mapping_target` conformance case runs on both adapters and in the encrypted suite; it was written after the fix and was not observed failing there.

## Follow-up

A fully rolled-back, reconciled sent cancellation no longer triggers the insertion clause of pull protection, since it has no local row and no future mapping; its own created identity stays protected until `forget` (`e14_rolled_back_sent_cancellation_does_not_block_pulled_insertion`, observed failing with `PullConflict` first, on core and memory). Pulling into a schema-declared table outside the configured entity tables was reviewed and is intended behavior (`docs/guides/sync-implementation.md`); it is not a defect.
