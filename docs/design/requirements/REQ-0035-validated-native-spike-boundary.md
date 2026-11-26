---
id: REQ-0035
title: The native spike validates boundary inputs and preserves ownership
type: requirement
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-20
updated: 2026-10-20
parents: [OBJ-0002]
mitigates: [THR-0009]
tags: [spike, native, safety]
---

# REQ-0035: Validated native spike boundary

WHEN the native spike receives an unknown/disposed handle, null required input,
invalid UTF-8 or a boolean outside 0/1, THE bridge SHALL return an error without
inserting a row. Within documented pointer and serial-call preconditions, it
SHALL copy inserted strings, expose query bytes only for the visitor's duration,
and release internal borrows before invoking a visitor that re-enters for a read.
Ordinary Rust panic unwinding SHALL NOT cross the exported C boundary.

This experimental requirement does not promise safety for arbitrary pointers,
cross-thread calls, foreign unwinding or allocation failure. SPEC-0018 defines
the local error/ownership contract; TC-0018 verifies its testable subset.
