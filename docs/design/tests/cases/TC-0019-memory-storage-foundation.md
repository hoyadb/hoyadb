---
id: TC-0019
title: Initial memory storage obeys typed asynchronous CRUD
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-21
updated: 2026-10-21
parents: [TP-0001]
verifies: [REQ-0034]
tags: [test, foundation, memory, storage]
---

# TC-0019: Memory storage foundation

Implement named Rust tests against API-0002, awaiting the actual trait methods
without adding an async runtime dependency. Use equivalent partitions for known
and unknown tables, existing/missing IDs and duplicate/new inserts; boundary
values for signed integers. Test comments map these cases and identify partial
TC-0016 first-scenario coverage; tests do not monitor filesystem/platform calls.

```gherkin
@REQ-0034
Scenario: M01 typed roundtrip
  Given an empty table and rows with Unicode strings, both booleans and signed 64-bit extrema
  When each row is inserted and read asynchronously
  Then each complete row equals its input exactly
@REQ-0034
Scenario: M02 owned reads
  Given a stored row with multiple fields
  When an owned read result is modified or discarded
  Then a later read returns the original unchanged row
@REQ-0034
Scenario: M03 table and instance isolation
  Given two tables and two independently constructed memory adapters
  When the same ID receives distinct rows in each table and adapter
  Then each read returns only its own complete row
@REQ-0034
Scenario: M04 duplicate rejection preserves data
  Given an existing row
  When insertion of a different row with the same ID is attempted
  Then RowAlreadyExists carries the table and ID and the original row is unchanged
@REQ-0034
Scenario: M05 unknown table errors
  Given no table named missing
  When insert get replace and remove are attempted against missing
  Then each returns TableNotFound carrying missing and no table is created
@REQ-0034
Scenario: M06 missing point read and replacement
  Given an existing empty table
  When a missing ID is read and then replaced
  Then get returns None and replace returns RowNotFound carrying the table and ID
  And a later read still returns None
@REQ-0034
Scenario: M07 complete replacement
  Given a row with two fields
  When it is replaced by a row of the same ID containing one changed field
  Then only the replacement field remains with its exact typed value
@REQ-0034
Scenario: M08 removal result
  Given an existing row
  When it is removed and then removed again
  Then removal returns true then false and the point read returns None
@REQ-0034
Scenario: M09 idempotent table creation
  Given a table containing a row
  When the same table is created again
  Then creation succeeds and the complete existing row remains unchanged
```

Acceptance requires each named behavior to fail before its implementation and
pass afterward, plus format/lint and documentation checks. Execution: all nine
cases failed at the expected missing operations before implementation, then
passed on Rust 1.98.1. See `docs/implementation/phase-one-foundation.md` for
commands, evidence and CI limitations.
These tests prove the initial memory contract, not full SPEC-0001, schema/query
semantics, persistence, engine parity, subscription or complete REQ-0034 coverage.
