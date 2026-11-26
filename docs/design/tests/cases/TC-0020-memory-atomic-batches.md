---
id: TC-0020
title: Ordered memory batches commit completely or preserve all tables
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-21
updated: 2026-10-21
parents: [TP-0001]
verifies: [REQ-0034, NFR-0003]
tags: [test, memory, transactions]
---

# TC-0020: Memory batch cases

Named Rust tests in the existing memory contract suite exercise SPEC-0020 and
API-0002 through `TransactionalStorage`; compare complete owned rows across
tables before/after. Use ordered state transitions and error partitions. This
is partial memory atomicity evidence, not durable queue/engine parity testing.

```gherkin
@REQ-0034 @NFR-0003
Feature: Optional atomic memory write batches
Scenario: T01 mixed cross-table commit
  Given two tables containing distinct existing rows
  When a batch inserts, replaces and removes rows across both tables
  Then all complete expected rows are visible and removed rows are absent
Scenario: T02 intra-batch ordering
  Given an existing empty table
  When a batch inserts, replaces, removes and reinserts the same ID
  Then every operation succeeds and only the final complete row remains
Scenario: T03 duplicate rolls back all tables
  Given two populated tables
  When a batch writes valid changes then attempts a duplicate insert then another mutation
  Then the original RowAlreadyExists is returned and both tables remain unchanged
  And the remaining mutation has no visible effect
Scenario: T04 replacement after removal fails atomically
  Given an existing row
  When a batch removes it then attempts to replace it
  Then RowNotFound carries the original table and ID and the original row remains
Scenario: T05 each mutation rejects an unknown table
  Given a populated table and an unknown table
  When each mutation kind targets the unknown table after a valid candidate change
  Then TableNotFound carries that table and the populated table is unchanged
Scenario: T06 first error identity
  Given a batch containing multiple different potential typed errors
  When the batch executes
  Then the first error in input order is returned with its original table and ID
  And all original data remains unchanged
Scenario: T07 empty batch and absent-row removal
  Given existing unrelated rows
  When an empty batch and a batch removing a missing ID from a known table execute
  Then both succeed and all unrelated complete rows remain unchanged
```

Execution: all seven cases failed at the expected missing batch implementation
while the nine existing CRUD cases passed. After implementation all 16 pass,
along with formatting, Clippy and the WASM library build. See
`docs/implementation/phase-one-foundation.md` for the mechanism and limits.
