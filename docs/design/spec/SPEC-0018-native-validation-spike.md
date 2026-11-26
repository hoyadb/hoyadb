---
id: SPEC-0018
title: Validate the host spike through a native C ABI and Swift
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-20
updated: 2026-10-20
parents: [ADR-0008, ARC-0002]
satisfies: [REQ-0035, REQ-0001, REQ-0003, REQ-0007, REQ-0008, REQ-0011, REQ-0034]
depends_on: [SPEC-0016]
tags: [spike, native, ffi]
---

# SPEC-0018: Native validation spike

Origin: operator instruction to continue Stage 0 without adopting the unapproved
browser dependency. Extend native insert/query with explicitly drained subscription
events and platform-driven async command scopes; this is not full Stage 0.

## Scope and acceptance

Add an isolated std-only Rust `staticlib` bridge depending on the existing core,
a handwritten C header, and a Swift desktop executable using `-import-objc-header`.
Core insertion/query behavior stays in Rust. No new external runtime dependency,
production API, persistence, browser execution, sync or mobile support.
Run the named Gherkin cases in TC-0018 from a real Swift process; retain host tests.
Record expected behavioral RED, GREEN, formatting, lint and exact build commands.
Review and commit this specification and cases before test code or implementation.

## Experimental boundary contract

Stores use checked monotonic nonzero `u64` handles in a global mutex registry;
the validated harness uses serial Swift calls (async resumption may switch OS
threads). The mutex is an internal
choice, not a general concurrency support claim. Handles are not authentication tokens.
Do not reuse a disposed handle. Exhaustion must fail rather than wrap/reuse IDs.
Creation returns zero on failure. Other operations return status: 0 success,
1 invalid/disposed handle, 2 invalid input (null required input/callback,
invalid UTF-8 or boolean outside 0/1), 3 caught Rust panic or poisoned registry.
If several arguments are invalid, no error precedence is promised.
Validate before insertion; rejected input does not change store contents.

Input strings are borrowed, readable NUL-terminated memory for the entire call;
null and UTF-8 are checked, but arbitrary pointer validity cannot be checked.
Insertion copies strings. Totals cross as signed 64-bit integers; paid is `u8`.
Query clones the core snapshot, drops the registry lock, then invokes
a synchronous row visitor with borrowed ID/status byte pointers and lengths,
an `i64` total and a `u8` paid flag. Swift copies bytes during each callback.
Pointers expire when the callback returns; empty results invoke no row callback.
The callback context is opaque caller-owned memory kept valid through the query.
Bounded nested reads from a row callback are supported; no registry borrow or
exclusive core reference survives the foreign callback. This is not a subscription.

Local C names are `hoya_open() -> u64`, `hoya_close(u64) -> i32`,
`hoya_insert(u64, const char* id, const char* status, i64 total, u8 paid) -> i32`,
and `hoya_query(u64, const char* status, row_callback, void* context) -> i32`.
The callback receives context plus ID/status pointers and lengths, total and paid.

Catch ordinary unwinding Rust panics at exported entry points; none unwinds into C.
This does not recover allocation aborts, invalid-memory access or foreign exceptions.
Swift callbacks must not throw/unwind across C. A panic status is not a rollback
guarantee: discard the affected handle. No general cross-thread safety is claimed.

## Subscription and async-scope experiment

The core allocates checked nonzero subscription tokens and queues an initial
event when subscribing to one status predicate. With unique fixture IDs, each
committed insertion that changes membership queues one event per affected
subscription; an irrelevant insertion queues none. A multi-insert scope finish
queues at most one event per affected subscription, after all rows are visible.
Unsubscribe rejects stale tokens and removes their pending events; closing a
store drops its registrations. No retained foreign callbacks or contexts exist.

`hoya_drain(handle, event_callback, context)` snapshots and clears pending events
under the registry lock, drops the lock, then synchronously invokes Swift once
per event with subscription token and matched-row count at enqueue time. The
bridge rechecks store/subscription liveness before each callback, suppressing
remaining extracted events if an earlier callback unsubscribes or closes it. The
visitor may query the current full result reentrantly. Queued events are change
signals, not historical row snapshots: several commits before draining can make
the current result newer than an event's count. Callback-triggered writes queue
events for a later drain. The caller must keep the context valid for the drain.
This is explicit poll/drain delivery, not push; report insertion-to-callback
latency, calling thread and drain scheduling cost without a UI-thread claim.
Acceptance fixtures drain after each commit to observe its exact full result.

Additional experimental C names: `hoya_observe(handle, const char* status)`
returns a subscription token or zero on failure; `hoya_unobserve(handle, token)`
returns a status. The drain callback is `(void* context, u64 token, size_t count)`.
`hoya_begin_scope(handle)` returns a scope token or zero;
`hoya_scope_insert(handle, scope, id, status, total, paid)` and
`hoya_scope_query(handle, scope, status, row_callback, context)` use the existing
row/input conventions. `hoya_finish_scope(handle, scope, u8 commit)` consumes
the scope (0 abort, 1 finish; other flags reject with input error).

The core permits one active command scope per store. Begin returns a checked
nonzero scope token, or zero when busy/exhausted; staging and scoped queries
reject stale tokens. Scope reads combine committed rows with staged inserts;
ordinary reads/subscriptions hide staged rows. Finish publishes all staged rows
atomically then emits membership events; abort discards them without an event.
Finish/abort consume the token, release the active slot and reject stale reuse.
Invalid/disposed subscription or scope tokens return status 1; existing input
validation and panic barriers apply. Duplicate fixture IDs are outside this slice.

Swift drives an actual async function: begin, stage, suspend, re-enter for reads,
then finish on success or abort on a thrown error. No Rust borrow or registry
lock survives a callback or await. Core scope state enforces serialization, not
Swift convention. A busy begin rejects rather than queues another handler.
Interleaving ordinary writes, cross-store serialization, cancellation, abandoned
scopes and stuck handlers remain unresolved; this is partial REQ-0011 evidence,
not a production transaction or durable-commit guarantee. Close discards scopes.

Before implementation, observe failing Rust tests for initial/change/no-change
events, unsubscribe/close cleanup, drain snapshot isolation, staged visibility,
atomic finish with one event, abort/no-event, busy begin, stale tokens and scope
exhaustion. Then run the Swift N07–N09 scenarios and retain N01–N06 checks.

## Evidence and rollback

Primary guidance: [Rust staticlib linkage](https://doc.rust-lang.org/reference/linkage.html),
[Rust FFI safety](https://doc.rust-lang.org/nomicon/ffi.html), and
[Swift C interoperability](https://www.swift.org/documentation/articles/wrapping-c-cpp-library-in-swift.html).
Installed compilers alone do not prove linking or runtime behavior. THR-0009 and
REQ-0035 track the boundary risk. Revert only this isolated bridge/harness and links;
no persisted user data, publication or shared-history rewrite is involved.
