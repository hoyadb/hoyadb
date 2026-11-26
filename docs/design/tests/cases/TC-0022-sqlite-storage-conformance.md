---
id: TC-0022
title: Memory and SQLite share the storage subset and SQLite survives restart
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-21
updated: 2026-10-21
parents: [TP-0001]
verifies: [REQ-0034, REQ-0038, NFR-0003]
tags: [test, sqlite, conformance, persistence]
---

# TC-0022: SQLite foundation and shared conformance

Run exactly the M01–M09 assertions from TC-0019 and T01–T07 from TC-0020
through one test-only generic harness for both memory and SQLite factories.
Factories own fresh isolated stores and cleanup; filesystem fixtures are only
used by SQLite. Keep the 16 behavior names/trace tags and complete-row checks.
This is initial API-0002 conformance, not the full SPEC-0001 acceptance suite.

Additional SQLite cases use equivalence partitions for operational/domain
errors, exact-string/boundary typed values and state transitions across failure
and restart. All files and SQL injection fixtures belong to the current test.

```gherkin
@REQ-0038 @NFR-0003
Feature: Native SQLite storage subset
Scenario: Q01 committed changes survive a process restart
  Given a fresh dedicated database file
  When a writer process creates two logical tables and commits inserts replacements and removals in a batch
  And that process exits and a separate reader process opens the same file
  Then complete rows retain Unicode strings signed i64 extrema and both booleans
  And omitted fields and removed rows remain absent and unrelated rows remain unchanged
Scenario: Q02 failed batch remains rolled back after restart
  Given two tables with committed rows in a dedicated database
  When a writer process executes a valid mutation prefix then a duplicate insertion
  Then the exact RowAlreadyExists is returned
  And another reader process sees all original rows and no prefix or later mutations
Scenario: Q03 exact names and empty rows use bound values
  Given names and string values containing Unicode embedded NULs quotes and SQL-looking text
  When tables rows and fields are created inserted replaced read and removed
  Then all strings and typed values roundtrip exactly without affecting unrelated data
  And empty names and a row with no fields remain valid as in API-0002
Scenario: Q04 open failure is explicit without memory fallback
  Given an empty path a :memory: path a file: SQLite URI a dedicated existing directory used as the database path or a path beneath a nonexistent parent
  When opening each path is attempted
  Then BackendFailure is returned and no adapter is exposed
  And no parent directory or substitute database is silently created
  Given a dedicated unrelated SQLite file or incompatible format marker schema or non-SQLite file
  When opening each as an adapter is attempted
  Then BackendFailure is returned before schema or data mutation
  And the unrelated file and its schema data remain unchanged
Scenario: Q05 lock failure leaves prior state and allows later recovery
  Given committed rows and a separate connection holding a write lock on the test database
  When a storage mutation or batch is attempted
  Then BackendFailure is returned without publishing any candidate rows
  When the external lock is released
  Then prior complete rows remain unchanged and a new ordinary write can succeed
Scenario: Q06 malformed stored fields fail without partial rows
  Given dedicated database fixtures with an unknown field discriminator or invalid boolean or incompatible SQLite storage type or invalid NULL payload
  When the corresponding complete row is read
  Then BackendFailure is returned rather than a coerced or partially decoded row
  And an unrelated valid row remains readable unchanged
Scenario: Q07 SQL failure rolls back a partial row mutation and a batch prefix
  Given committed rows and a fixture-local SQL trigger aborting a later field or row write
  When an insertion replacement or batch reaches that trigger after earlier SQL writes
  Then BackendFailure is returned and every original complete row remains unchanged
  And no new partial row or field is visible after reopening the database
Scenario: Q08 connection ownership and ready Send futures
  Given one opened SQLite adapter owned by the test
  When ownership moves into another thread and that thread performs CRUD and a batch
  Then complete rows and typed errors match the contract
  And every returned operation future satisfies Send and is ready on its first poll
  And mutation execution before the future is polled is consistent with API-0002
Scenario: Q09 fresh-cache CI bootstrap and native WASM exclusion
  Given checked-in root native and browser manifests lockfiles and CI commands with no prefilled task Cargo cache
  When each manifest's locked fetch is run before its offline checks
  Then offline tests lint and builds can resolve their approved dependencies
  And the root WASM command selects only core and memory without the native SQLite adapter
  And browser CI compiles WASM without claiming a browser runtime test
  And existing user runner tags remain unchanged
```

Q05/Q07 must assert operational errors are BackendFailure rather than remapped
TableNotFound, RowAlreadyExists or RowNotFound. Exercise both valid-prefix batch
rollback and a single multi-statement replacement; use complete expected rows
across known IDs, not only counts. Q06 corrupts only test-owned files with SQL,
using fixture schema changes where necessary to bypass physical constraints;
it adds no production corruption bypass. Q08 documents synchronous execution,
not nonblocking performance or concurrent shared use. Q01/Q02 are process
restart evidence, not crash/power-loss or queued-command durability evidence.

Execution: the 16 extracted memory cases stayed green. All 16 SQLite cases
then failed at missing behavior; Q01/Q02 writer processes and Q03–Q08 also failed
before implementation. All 40 workspace tests now pass, including eight
SQLite-specific cases. The URI partition reproduced silent memory selection
before its guard, then passed. Formatting, Clippy and core/memory WASM builds
pass. Q09 fresh-cache CI execution is recorded separately in the foundation
report; remote GitLab execution remains unproved.
