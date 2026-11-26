---
id: TC-0021
title: Native committed rows survive restart without partial failure publication
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-21
updated: 2026-10-21
parents: [TP-0001]
verifies: [REQ-0035, REQ-0037, NFR-0003]
tags: [test, spike, native, persistence]
---

# TC-0021: Native persistence cases

Tests implement SPEC-0021; synthetic temporary files only. Rust tests cover
SQLite error/restore behavior, while a Swift executable takes explicit writer
and reader modes. Launch each mode as a new OS process with recorded exit codes.
Compare full row membership and typed values without assuming ordering.

```gherkin
Scenario: P01 committed insert survives process restart
  Given a new harness-owned SQLite path
  When a Swift writer opens persistent mode, inserts mixed statuses and exits successfully
  And an independent Swift reader opens the same path
  Then status queries return exactly the committed Unicode rows and i64 extrema
Scenario: P02 committing handler scope survives restart
  Given a persistent store with an observed query whose initial event is drained
  When two matching inserts are staged then finished successfully
  Then one post-commit event reports both rows and ordinary reads see them
  And a new process restores both rows without subscriptions or active scopes
Scenario: P03 failed persistent write publishes nothing
  Given committed rows and a deterministic SQLite transaction failure fixture
  When a candidate insert or committing scope finish attempts persistence
  Then status 4 is returned and prior memory disk rows and pending notifications are unchanged
  And a failed scope remains valid for retry or abort
Scenario: P04 aborted staged writes remain absent after restart
  Given a persistent store with committed rows
  When a handler stages new rows then aborts and its process exits
  Then an independent reader restores only the earlier committed rows
Scenario: P05 invalid inputs and incompatible files fail cleanly
  Given null or invalid UTF-8 paths, empty or memory paths, file: SQLite URIs, an unusable path and incompatible database fixtures
  When persistent open is attempted
  Then it returns zero without a live handle or foreign unwind
  And unrelated harness fixture files are not overwritten
Scenario: P06 memory mode remains independent
  Given memory-only and persistent stores
  When each receives different committed rows
  Then each query sees only its own data and only persistent rows survive restore
```

P03 uses a narrowly scoped Rust fixture to force failure inside a transaction,
not permission changes or a disk-fill attack. Test insert and scope-finish paths
separately. Validate invalid stored boolean/type/duplicate data before handle
publication. No scenario has run yet; require behavioral RED followed by GREEN.
These cases do not prove crash/power-loss durability, encryption, multi-writer
coordination, production adapter parity or durable command queue behavior.
