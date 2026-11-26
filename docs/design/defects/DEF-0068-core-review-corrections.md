---
id: DEF-0068
title: Scope self-relation queries, publication suspension, same-migration backfill and late begin bound
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-10
updated: 2026-11-10
parents: [SPEC-0037]
affects: [API-0004, API-0006, API-0009]
regression_for: [TC-0041, TC-0026, TC-0032, TC-0034]
tags: [core, review, regression]
---

# DEF-0068: Scope self-relation queries, publication suspension, same-migration backfill and late begin bound

## Symptom

Four independent core defects from the 2026-11-10 review. (1) An in-scope query over a ToOne relation targeting its own table failed with `RowAlreadyExists` or `overlay row disappeared`. (2) Pull ticket and merge session retirement ran after the publication await, so an abandoned future left `begin_pull` Busy after a durable commit. (3) `Backfill` could not target the implicit key of a ToOne added by `SetRelations` in the same migration. (4) `begin` validated arguments standalone, so a record one container level too deep ran its handler and failed only at `finish`.

## Acceptance

Scope queries over a self relation equal the committed store; a committed pull or merge retires its ticket before publication can suspend; a same-migration backfill reaches the implicit key with unchanged atomic endpoint validation; `begin` applies the record bound `finish` enforces.

## Root cause and fix

`overlay_query` inserts candidates before relation targets and filters target-only rows. `apply_transaction_then` retires the ticket or session directly after the synchronous commit; reactive publication skipped by an abandoned future is not addressed. `Backfill` resolves the implicit key of an already declared ToOne without early materialization. `begin` and `finish` share one record constructor and `begin` runs the send-size check.

The existing `c01_c06_c08_public_owner_contract` conformance case pinned the late failure (600 KB arguments passing `begin`); it now expects `LimitExceeded` from `begin` and keeps a finish-time case with a 1.2 MB staged row. Regression cases: `ov_self_relation_*`, `si_o02_self_relation_scope_queries_equal_the_committed_store`, `e12_committed_pull_retires_its_ticket_before_publication_can_suspend`, `e12_committed_merge_retires_its_session_before_publication_can_suspend`, `review_m01_backfill_reaches_implicit_key_of_same_migration_to_one`, `c05_begin_applies_the_record_depth_bound_finish_enforces`.
