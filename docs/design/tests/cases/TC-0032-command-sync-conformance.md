---
id: TC-0032
title: Commands remain isolated durable and recoverable across hosts
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-23
updated: 2026-10-23
parents: [TP-0001]
verifies: [REQ-0010, REQ-0011, REQ-0012, REQ-0013, REQ-0014, REQ-0015, REQ-0016, REQ-0017, REQ-0018, REQ-0019, REQ-0020, REQ-0021, REQ-0022, REQ-0028, REQ-0029, REQ-0033, REQ-0034, REQ-0043, NFR-0003, THR-0017]
depends_on: [SPEC-0029, RFD-0003]
tags: [phase-four, commands, sync, cross-platform]
---

# TC-0032: Phase 4 acceptance

Policies P1–P7 refer to the operator-approved RFD-0003 adopted by ADR-0032.
The implementation report records actual executed portions; preparing this
scenario list alone does not establish that any whole scenario passes.
Each scenario receives an exact API/test mapping in its reviewed slice before
test code; no compile failure counts as behavioral RED.

## Target and fixture matrix

Shared behavioral cases run on memory, native SQLite and actual browser SQLite
OPFS. Use the existing tests/conformance harness and crate adapter integration
tests. C02–C04/C18/C20 additionally require real suspended Swift and browser host
handlers via existing spike directories; Rust mocks cannot establish that gate.
C07/C09/C10/C15/C16/C17 require genuine separate native processes/browser reloads
where restart is asserted. Physical commit faults are adapter-specific. Inject
clock, jitter, outcome sequence and storage faults; do not use timing sleeps as
proof of ordering. Owned synthetic stores/namespaces only; no broad cleanup.

```gherkin
Feature: P1 tracked async handlers commit as a single visible operation
 Scenario: C01 awaited operations read staged writes and commit exact final state
  Given the walkthrough schema and a committed prior order
  When a handler awaits its prior-count query and inserts an order and two items
  Then later handler reads and relation navigation see those staged rows
  And each supported query operator order limit and optional value matches ordinary fetch
  And committed readers see only the prior state until success
  And success commits the exact order and both items and queues its envelope
  And ordinary observations emit only the complete committed transition
 Scenario: C02 competing handlers on one database serialize without holding callbacks
  Given one exclusive command database owner under ADR-0037
  When its first host handler suspends after a staged write and a second is requested
  Then the second handler has not begun and committed reads and drains still work
  When the first finishes
  Then the second starts and same-store reads see the complete first commit
  And FIFO admission holds for more than two queued callers of that database
  And independent exclusive databases progress as specified by TC-0041 SI-A01 and SI-A02
  And unknown adapters and WASM keep their conservative global fallback
 Scenario: C03 failure abandonment and explicit abort release serialization
  Given a suspended handler with staged inserts replacements and removals
  When the handler fails or its host task is cancelled or its owner closes
  Then no staged data envelope counter change or observation commits
  And the next admitted handler can run
  Given a handler that never settles
  When the host explicitly aborts its ticket
  Then its late completion rejects and another command proceeds
  And no implicit watchdog timeout or forced host-loop termination is claimed
 Scenario: C04 stale scopes and other writers cannot bypass isolation
  Given a live scope and forged stale cross-owner and closed-owner tickets
  When a stale ticket attempts a read write finish or abort
  Then it rejects without changing the active scope
  When direct writes reconciliation migration or reset are attempted during the live scope
  Then they cannot mutate committed or staged state
  And nested execute through the handler scope rejects without deadlocking
  And committed queries registrations drains and unsubscribe remain usable

Feature: P2 command envelopes preserve values context and identity
 Scenario: C05 bounded JSON and immutable trusted context survive host conversion
  Given nested args and opaque identity with Unicode exact i64 extrema and finite floats
  When a versioned command executes without network access
  Then its envelope contains only command args captured context schema version and generated key
  And its handler cannot reassign the envelope identity or captured time
  And local rows and undo images are absent from the transport payload
  And host bindings preserve integer values exactly or reject unsafe coercion
  When missing context duplicate JSON keys invalid numbers malformed UTF-8 excessive depth or size are supplied
  Then execution rejects before local mutation or handler admission
 Scenario: C06 generated keys and counters are durable and admission is atomic
  Given equal args same clock samples backward clock samples and a reopened store
  When distinct commands commit
  Then generated keys are distinct time-sortable and local without application key construction
  And two fresh stores in one installation use separate persisted random namespaces
  And equal clocks and equal initial sequences across those owners cannot repeat a key
  And retry never allocates a replacement key
  When key exhaustion capacity failure or the final typed batch fails
  Then neither application rows nor queue metadata partially commit
  And failure emits no committed data or successful command event

Feature: P3 P4 queue transitions and dependency state survive interruption
 Scenario: C07 commit boundary restart contains both data and command or neither
  Given native SQLite and browser OPFS clients in independent lifetimes
  When a client is terminated immediately before or after the command transaction commits
  Then reopen before commit has neither optimistic rows nor envelope
  And reopen after commit has both exact rows and their original envelope
  And sent state survives restart and remains eligible for unacknowledged retry
 Scenario: C08 explicit reference paths produce a durable dependency DAG
  Given A creates a local order B references A and C references both A and B
  When all three execute locally with declared reference paths
  Then B and C stay queued and are never transmitted before every parent maps
  And identical local-looking text in undeclared strings is unchanged
  When unknown local references invalid paths or cycles are introduced
  Then the entire attempted command rejects without data or queue mutation
 Scenario: C09 receipt without final outcome survives restart and never resends
  Given a sent parent and its blocked dependent
  When durable receipt arrives without an authoritative outcome and the client reopens
  Then the parent has left the retry queue but awaits its outcome durably
  And explicit retry and issuing another command cannot resend it
  And neither the parent local ID nor the dependent args has been mapped
 Scenario: C10 mapping and first-send freezing are atomic across restart
  Given multiple parents and dependents including schema Reference fields
  When authoritative mappings arrive and the client is interrupted during mapping
  Then reopen observes either the entire mapping transaction or none of it
  And declared argument paths and reference cells are corrected exactly
  And ordinary string fields and unrelated records are unchanged
  And a dependent becomes eligible only when every parent has resolved
  When that dependent sends and its response is lost then it reopens and retries
  Then the complete transmitted bytes and key match the first send exactly
 Scenario: C11 ID collisions and malformed maps cannot partially reconcile
  Given a live parent graph and an already occupied authoritative record ID
  When a mapping collides has ambiguous ownership or a reconciliation plan is invalid
  Then all rows maps dependencies and terminal state remain unchanged
  And acknowledged work stays available for local resolution without network resend
  When the client stops after receipt but before its local plan commits
  Then reopen recovers the exact authoritative result for local reconciliation
  And no acknowledged envelope is sent again

Feature: P4 P5 transport results reconcile independently and rollback safely
 Scenario: C12 one array seam correlates independent results by key
  Given one eligible command then three independent eligible commands
  When fake and host HTTP transports receive their pushes
  Then every call uses an array in local execution order even for length one
  And reverse-order results apply to the correct keys
  And missing results leave only those entries retryable
  And one rejection never rolls back another successful batch member
  And duplicate identical receipts and outcomes are idempotent
  And unknown keys or conflicting terminal outcomes return typed errors without overwriting accepted state
  And after history eviction or explicit forget late duplicate keys reject as unknown without recreating work
 Scenario: C13 explicit backend result mapping corrects all intended local rows
  Given a handler that creates several rows and an arbitrary structured backend result
  When its registered reconciliation adapter returns a validated correction plan
  Then all intended rows maps and outcome state commit atomically
  And ordinary subscriptions observe the complete correction
  And app CommandOutcome has no wire correlation field or HTTP code
  And no client mutation log is treated as backend execution or authorization
 Scenario: C14 explicit rollback cascades without destroying later unrelated edits
  Given a rejected parent and transitive multi-parent blocked dependents
  When rollback is invoked on their unconflicted optimistic writes
  Then the complete dependency closure reverses in reverse execution order atomically
  And unrelated rows remain and a repeated rollback is harmless
  Given a later independent write to a row in the rollback or correction set
  When rollback or correction is attempted
  Then RollbackConflict preserves every current row and all recoverable undo state
  And no partial cascade or stale before-image overwrites that later write
 Scenario: C15 cancellation distinguishes unsent work from irreversible sends
  Given queued sent and rejected commands with dependent optimistic rows
  When explicit cancel executes
  Then safe local rollback and dependency cascades are atomic
  And unsent cancelled work is never dispatched and releases capacity
  And sent cancelled work retains a durable minimal tombstone after restart
  And late receipts or outcomes cannot resurrect its rows or retry queue
  And the API promises no backend cancellation or undo of a sent action
  And a rollback conflict keeps data and recovery state for explicit resolution

Feature: P6 P7 scheduling capacity and lifecycle are explicit
 Scenario: C16 backoff gates retries and persists due state
  Given deterministic clocks and different injected client jitter samples
  When push repeatedly fails or returns only partial outcomes
  Then only unacknowledged eligible entries retry with the original envelope
  And exponential base delays increase to their configured cap with bounded positive jitter
  And explicit retry and new-command wakes cannot bypass due times
  And restart and backward clock samples do not create a retry storm
 Scenario: C17 queue-aware lifecycle rejects unsafe transitions without deletion
  Given queued sent awaiting-outcome rejected-unrolled conflict or tombstone state
  When authored upgrade missing-chain reset or membership reset is requested
  Then the operation rejects and all application and recovery records remain exact
  Given fully cleared recovery work and a supported queue-free managed store
  Then existing atomic authored migration and known forward missing-chain reset still work
  And corrupt future or unknown reserved metadata always rejects unchanged
  And private SQLite physical DDL and format marker remain intact
 Scenario: C18 live status outcomes and callbacks obey ownership and redaction
  Given data status and outcome observers with queued owned notifications
  When callbacks re-enter for reads unsubscribe close or throw on the browser host
  Then no owner borrow spans the callback and cancelled later deliveries are suppressed
  And unrelated live callbacks continue according to the existing bridge contract
  And one status call includes aggregates entries blocked local IDs and resolution attention
  And default status errors and diagnostics contain no args identity undo images or row payloads
 Scenario: C19 capacity bounds live recovery and terminal history separately
  Given configured per-store live capacity 500 across tables and 500 admitted recovery records
  When another command is issued
  Then it rejects before invoking its handler or changing rows
  And awaiting outcomes unresolved rollback and sent tombstones cannot evade the cap
  And rejected commands are not auto-cancelled to create capacity
  When fully resolved diagnostic results exceed their configured count
  Then only the oldest resolved history is evicted and live recovery is preserved
  And final mappings still needed by live dependents are retained within recovery capacity
  And a later argument using an evicted local-ID mapping rejects as UnknownLocalReference
  And another distinct store has independent capacity despite sharing the app coordinator
 Scenario: C20 all network and host scheduling belongs to the integration
  Given failing fake transport then a task-owned HTTP loopback server
  When offline commands commit and later eligible work is pushed
  Then offline commit succeeds independently of transport failure
  And core opens no connection timer or thread and emits no telemetry
  And the host reference forwards intended envelopes without logging credentials or payloads
  And actual suspended Swift and browser handlers retain the same lifecycle contract
```

## Historical case mapping and explicit limits

| Existing cases | Phase 4 coverage |
| --- | --- |
| TC-0005 all six scenarios | C01/C02/C07/C20; actual host execution required |
| TC-0006 command envelope/key/version scenarios | C05/C06/C10; its entity-diff scenario belongs to Phase 5 |
| TC-0007 each status example, receipt, failure, correction and rollback | C09/C12/C13/C14; all three outcome example rows execute |
| TC-0008 transactional queue/restart/dependency scenarios | C06–C11; nontransactional engine has no atomic claim and requires explicit capability rejection or separately specified best-effort support |
| TC-0009 retry scenarios | C10/C12/C16/C20; automatic wakes respect the approved backoff |
| TC-0010 status/cancel/capacity scenarios | C15/C18/C19; zero inferred automatic cancellation |
| TC-0012 identity/egress scenarios applicable to commands | C05/C18/C20; pull and entity diffs remain Phase 5 |
| TC-0014 command-only array/outcome scenarios | C12; mixed/entity-diff cases remain Phase 5 |

This is a contract mapping, not a claim that historical feature files have an
executable Gherkin runner. Nontransactional engines are not currently implemented
by the public schema owner; do not invent a passing nontransactional target.
Backend deduplication/authentication, independent multi-client store access,
physical power loss and production TLS are not proved by these client fixtures.
