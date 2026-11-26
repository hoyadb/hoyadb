---
id: TC-0031
title: Large query sets and isolated observable recovery
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-23
updated: 2026-10-23
parents: [TP-0001]
verifies: [REQ-0007, REQ-0008, REQ-0009, REQ-0042]
depends_on: [SPEC-0028, API-0003, API-0005]
regression_for: [DEF-0006, DEF-0007, DEF-0008]
tags: [reactivity, review-resolution]
---

# TC-0031: Large query sets and isolated observable recovery

```gherkin
Scenario: Q01 large bounded SQL sets preserve portable selection and delivery
 Given over eleven hundred related records and a separate reactive window above the 4096-operand TEMP threshold
 When fetch projection observation navigation and reverse invalidation execute
 Then results match memory and no expression-depth error occurs
 And a targeted update intersects oversized matched and affected-ID sets without leaking temporary operands
 And an In query with over thirty-three thousand typed values remains exact
 And multiple In predicates Null duplicates Unicode and numeric extrema retain order and limit
 And flat and oversized paths select identical Float 42.0 and 1.25 i64 extrema Null and Unicode values
 And the oversized path runs in the actual browser worker without new SAH pool files
 And temporary operands leave persistent schema and subsequent queries unchanged
Scenario: Q02 explicit recovery failure is observable every time
 Given a dirty candidate group after a committed write
 When two explicit refresh attempts each fail
 Then each drain reports a typed error without stale successful count
 And repeated committed writes against the same dirty group do not flood duplicate errors
 When the next refresh succeeds
 Then exact current state delivers while retaining the count deadline
Scenario: Q03 row failure cannot poison projected counts
 Given equivalent default column and count observers
 When full-row materialization fails after a successful write
 Then row observers receive typed errors and count receives current projected cardinality
 When explicit refresh fails then later succeeds without another write
 Then repeated row errors are observable and exact current rows recover
 And count remains valid without duplicates or full-record reads for its own work
 And equivalent count registration succeeds from valid keys
 And equivalent row registration tries a fresh snapshot and failure has no side effects
 And final cancellation clears failed row retry state
Scenario: Q04 actual SQLite read fault follows a successful commit
 Given observed records and a post-commit fixture that corrupts a consulted physical cell
 When SQLite projection or row decoding fails
 Then the committed write remains successful and delivery reports the actual backend error
 And unaffected count observers remain live on row-only faults
 When the fixture restores the cell and refresh retries
 Then current exact snapshots recover without data loss
Scenario: Q05 real host error delivery preserves ownership and recovery
 Given native Swift and browser callbacks on the production reactive owner
 When fixture projected reads fail after a successful write then retry
 Then typed error callbacks deliver outside owner borrows and recovery is exact
 And counts retain their deadline and unsubscribe or close stops further delivery
```

Execute shared cases on memory/native/browser where portable; physical SQL faults
on native SQLite, plus real Swift/browser boundary callbacks. Preserve prepared
TC-0029/0030 regressions. Exact tests and RED/GREEN evidence accompany delivery.

Browser execution keeps the existing twenty second request deadline. Separate
large relation/dependency and minimal owner-driven TEMP window fixtures avoid
combining unrelated setup costs into one oversized scenario. Both run the same
assertions on memory/native/browser; each shared case uses an individual bounded
worker request, with no platform-specific lowered expectation.
