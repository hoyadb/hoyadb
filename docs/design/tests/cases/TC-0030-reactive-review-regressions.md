---
id: TC-0030
title: Reactive error state and normalized ordering remain consistent
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-23
updated: 2026-10-23
parents: [TP-0001]
verifies: [REQ-0007, REQ-0009, REQ-0042]
depends_on: [SPEC-0027, API-0005]
regression_for: [DEF-0004, DEF-0005]
tags: [reactivity, regression]
---

# TC-0030: Review regressions

```gherkin
Scenario: A dirty group cannot deliver stale counts or allocate duplicate caches
 Given a count observer with a pending count change
 When a later commit succeeds but its projected refresh fails
 Then only the typed error delivers and equivalent registration fails without new cache
 When explicit refresh succeeds with the previous successful cardinality
 Then current count republishes at the existing sixteen millisecond deadline
Scenario: Error recovery cannot bypass the count delivery deadline
 Given a successful count delivery at one hundred milliseconds
 When a commit refresh fails at one hundred and one and recovers at one hundred and two
 Then no count delivers before one hundred and sixteen
 And the exact current count delivers then
Scenario: Ordering keys preserve portable semantics independently of requested cells
 Given valid source rows and a related target with a wrong-kind ordering cell
 When the same ordered limited query uses fetch and observation
 Then both select the same source rows and count using Null ordering
 And explicitly requesting that invalid cell still returns InvalidRow
 And malformed physical ordering payload still returns BackendFailure
```

Shared memory/native/browser assertions extend TC-0029 R08/R10/R11; the
execution report records actual RED/GREEN and preserves these regressions.

## Phase 3 execution — 2026-10-23

The [reactive implementation report](../../../implementation/phase-three-reactivity.md)
records shared memory/native/browser assertions and actual Swift/main-thread
callback execution, including behavioral RED/GREEN, error recovery, normalized
ordering, retained count deadlines and cancellation. Implementation `58101a9`
preserves these as permanent regressions. These are explicit Rust assertion
mappings, not a claim that a Gherkin runner executed this Markdown file.
