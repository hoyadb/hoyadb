---
id: TC-0018
title: Native Swift host exercises the isolated Rust spike safely
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-20
updated: 2026-10-20
parents: [TP-0001]
verifies: [REQ-0035, REQ-0001, REQ-0003, REQ-0007, REQ-0008, REQ-0011, REQ-0034]
tags: [test, spike, native]
---

# TC-0018: Native validation cases

Execute in a real Swift desktop process against SPEC-0018. Compare row membership
and complete fields without assuming ordering. Product requirements are covered
only partially; persistence, browser and production-level guarantees are excluded.
Use equivalence partitioning for valid/invalid input, state transitions for
open/close, and boundary-value analysis for integers. Fixtures are synthetic;
the Swift scenarios and Rust-only error-barrier/core-semantics checks must all pass.

```gherkin
@REQ-0035 @REQ-0001 @REQ-0003
Scenario: N01 typed insertion and filtering
  Given a native store and mixed pending and complete rows
  When Swift inserts them and queries pending
  Then exactly the pending rows return with every field unchanged
@REQ-0035 @REQ-0003 @REQ-0034
Scenario: N02 empty nonmatching and isolated stores
  Given independent empty and complete-only stores
  When pending is queried and a pending row is inserted into another store
  Then the first two results remain empty
@REQ-0035 @REQ-0001
Scenario: N03 copied ownership and integer boundaries
  Given Unicode input buffers and signed 64-bit minimum and maximum totals
  When rows are inserted and the original input buffers expire
  Then queried strings and integer totals remain exact
@REQ-0035
Scenario: N04 rejected boundary inputs
  Given valid readable buffers and a store
  When unknown or disposed handles, null required inputs or callback, invalid UTF-8, or paid 2 are supplied
  Then each call returns its documented error without inserting a row
@REQ-0035
Scenario: N05 reentrant query visitor
  Given a store containing a pending row
  When its Swift row visitor performs one nested query on the same store
  Then both queries return the expected row without a borrow panic or deadlock

@REQ-0035
Scenario: N06 Rust-only error barriers
  Given a forced unwinding panic, a poisoned test registry, or an exhausted test handle counter
  When the corresponding boundary helper executes
  Then a panic or poison maps to error 3 and exhausted creation returns zero without handle reuse

@REQ-0035 @REQ-0007 @REQ-0008
Scenario: N07 explicitly drained subscriptions
  Given a status subscription and its initial event drained into Swift
  When matching and irrelevant unique-ID rows are inserted with a drain after each commit
  Then only the matching insertion emits another token and count
  And its visitor re-queries the complete matching rows without deadlock
  And insertion-to-callback latency and calling thread are recorded including drain scheduling
  And unsubscribe suppresses pending and later emissions and stale unsubscribe rejects
  And closing the store discards registrations and later drain rejects the disposed store
  And unsubscribe during a callback suppresses remaining extracted events for that token

@REQ-0011 @REQ-0008
Scenario: N08 suspended handler visibility and atomic finish
  Given a live subscription whose initial event has been drained
  When a Swift async handler begins a scope, stages two matching rows and actually suspends
  Then scoped reads see those rows while ordinary reads and drained events do not
  When the handler resumes, re-enters for a read and finishes successfully
  Then ordinary reads see both rows together and exactly one membership event is drained

@REQ-0011
Scenario: N09 abort and serialized scope lifecycle
  Given an active scope with a staged row
  Then a second begin returns zero without starting another handler
  When the suspended Swift handler resumes and throws, and its host aborts the scope
  Then no staged row or membership event is published
  And the consumed scope rejects reads writes finish and abort
  And a new scope can begin and finish, after which another begin succeeds
```

Execution: N01–N05 passed in a real Swift desktop process; the three Rust
error-barrier tests for N06 passed after observed failing runs. See
`spikes/cross-platform/native/README.md` for commands and evidence.
N07–N09 passed in a real Swift process after failing against the stub bridge;
five new core scenarios failed before implementation and now pass. The core
also checks token exhaustion. See the native README for delivery mechanism,
latency sample and the limits of these experiments.
Never test dangling/arbitrary pointers
as if rejection were guaranteed; they violate the contract. Dispose live stores
after each case and copy visitor bytes before returning.
