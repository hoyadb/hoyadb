---
id: TC-0041
title: Store ownership per-database admission and change-proportional command and sync work
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-31
parents: [TP-0001]
verifies: [SPEC-0037, REQ-0011, NFR-0001, NFR-0003, DEF-0034, DEF-0035]
regression_for: [DEF-0034, DEF-0035]
tags: [purpose-audit, storage, ownership, admission, scaling]
---

# TC-0041: Store ownership, per-database admission and bounded work

These are acceptance cases before implementation, not a passing report.
Fixtures are synthetic, offline and task-owned; no user store is opened. Each
case first fails on unchanged source (HEAD 7058b29 lineage) for its stated
behavioral reason. A missing symbol or fixture does not count.

"Char" marks a characterization case: it stays green before and after, and it
guards preserved semantics.

Phases follow SPEC-0037. A phase's cases are implemented only after that
phase's review.

## Conventions

- **"Equal at N":** the identical workload runs on stores seeded with N=10 and
  N=1000 unrelated rows. Entity cases also use M=10 and M=1000 unrelated
  metadata entries. The named SPEC-0037 R4 entry counters are then equal.
- **"Bounded":** the case states its exact bound as a function of the touched
  rows c and the window k. Each `ordered_ops` entry is one B-tree operation of
  O(log n) cost, and equality is on operation counts, not comparisons.
- **"Fullscan-free":** `sqlite_fullscan_steps == 0`, excluding statements on
  `__hoya_meta` and `__hoya_tables`.
- **No timing assertion appears anywhere.**

Phase-3 review clarification (2026-10-31, SPEC-0047): comparisons run the same
adapter and workload at each size, including `sqlite_rows_written`; memory and
SQLite totals are not compared with each other. Before planner conversion,
`work-counters` inspection seams expose the last transaction's touched record
and metadata keys (initially empty), its epoch, and the cached tombstone total
(initially absent). They do not alter control flow. Cache recomputation is a
test-only invariant, excluded from the measured operation. The static proof-site
test scans Rust source under `crates/hoyadb-core/src` for proof construction
tokens, pins the exact gated persist site, and tests rejection of an injected
additional site; it does not claim a pre-existing checker.

```gherkin
Feature: Phase 1 exclusive native ownership (SPEC-0037 R1, R2)
  Scenario: SI-L01 second unprotected open of an owned store
    Given store A opened with open_unprotected on native path P
    When open_unprotected(P) is called again in the same process
    Then it returns StoreInUse and A still reads and writes
    # RED: the second open succeeds

  Scenario: SI-L02a protected second open with the correct key
    Given a protected store owned at P
    When a protected open of P is attempted with the correct key
    Then it returns StoreInUse rather than the redacted protected-open error
    # RED: the second open succeeds

  Scenario: SI-L02b wrong key on owned versus unowned stores
    Given a protected store owned at P and a separate unowned protected store U
    When P is opened with a wrong key
    Then it returns StoreInUse, because the lease precedes any page read
    When U is opened with a wrong key
    Then it returns the unchanged redacted protected-open error and writes no byte to U
    # RED: the P open fails redacted today (the owner holds no exclusive lock,
    # so the wrong key fails at the first page read)

  Scenario: SI-L07 close and reopen release the lease
    Given a store owned at P
    When the owner is closed or dropped
    Then open of P in the same process succeeds with every row intact
    And the same holds through Facade close followed by open
    # char today; RED only if the lease were leaked

  Scenario: SI-L08 exclusive-mode journal on a protected store
    Given a protected store owned at P after committed writes containing a known plaintext canary
    Then any -journal sidecar present while owned contains no canary bytes
    And after close the presence or absence of -journal is recorded as observed evidence
    And reopen with the key returns every committed row, and no rollback of committed data occurs
    And the store's directory and sidecar family remain under the SPEC-0036 backup rules
    # RED: no lease, so this mode is never exercised

  Scenario: SI-L03 second process
    Given a child process owning P and blocked at a named barrier
    When the parent opens P
    Then it returns StoreInUse
    And after the child is killed the parent opens P with every row intact
    # RED: the parent open succeeds while the child lives

  Scenario: SI-L04 two facades map StoreInUse to Busy
    Given two Facade instances
    When both open the same native directory and name
    Then the second reply is ok false with error kind Busy and leaks no path or key
    # RED: the second open succeeds

  Scenario: SI-L05 external raw connection is excluded
    Given an owned store at P
    When a raw rusqlite connection runs BEGIN IMMEDIATE or SELECT on P
    Then it fails with SQLITE_BUSY and the owner's next write succeeds
    # RED: succeeds (current sqlite_cases q05 relies on it)

  Scenario: SI-L06 WASM open path is unchanged
    Then the wasm32 open path issues no locking statements and claims no exclusivity
    # char by source inspection; no browser execution in this case

  Scenario: SI-K01 real SQLite COMMIT failure (q05 moved to a cfg(test) unit test)
    Given a seeded store in the lib.rs test module
    And a deferred foreign-key constraint whose triggering statement succeeds
    Then SQLite fails each COMMIT with SQLITE_CONSTRAINT_FOREIGNKEY and the call returns BackendFailure
    And the complete changed row and the valid batch prefix are rolled back, nothing is published, and the indexes are intact
    And the next insert succeeds and is readable
    # char of every q05 commit-failure assertion on the engine's real commit
    # path; the private stimulus exists only in cfg(test) builds

  Scenario: SI-K02 every raw-connection-while-open test is redesigned, not deleted
    Given the suite run with R1 only
    Then each test failing with SQLITE_BUSY from a raw connection held while its store is open is listed by name in the Phase-1 evidence
    And each is rewritten to close, mutate or inspect, and reopen, or to use SI-K01's stimulus, keeping its assertions
    And same-live-owner retry fixtures use native-only dev-enabled test fault functions
    And their original statement abort, COMMIT failure and corruption assertions are preserved
    And the large-operands corrupt/read/restore/read fixture uses the feature-gated direct-store SQL seam without an extra write or reopen
    And closed-store tamper and inspection tests are unchanged
    # RED: the listed tests fail under R1

  Scenario: SI-K03 child readiness is never published partially
    Given a publisher paused after creating its output file and before writing its mode
    Then the parent's final readiness path is absent
    And after publication the path contains the exact complete mode
    And existing s05 sidecar encryption and s07 real process interruption assertions remain unchanged
    # RED: today's fs::write creates the final path before writing any bytes

Feature: Phase 1 per-database admission with global identity (SPEC-0037 R3)
  Scenario: SI-A01 queued request on A does not block B
    Given owner A on database X with a requested, unbegun command
    When owner B on database Y inserts
    Then the insert commits
    # RED: Busy (target/purpose-audit-20261026/availability.txt)

  Scenario: SI-A02 active handler on A does not block B's handler or writes
    Given A with an admitted scope awaiting its host handler
    Then B's command is admitted and commits
    And B's insert, entity patch and begin_pull succeed
    # RED: Busy or not admitted

  Scenario: SI-A03 same-database serialization is unchanged
    Then within one owner FIFO admission, Busy writes during its own queued or active work, and cancellation hold
    # char

  Scenario: SI-A04 public cross-owner ticket rejection
    Given public CommandDatabase owners A and B
    When A's CommandRequest is used with B's begin or cancel
    Then it rejects with the existing invalid-ticket error and changes nothing
    # char (today's global coordinator already rejects); preserved after R3

  Scenario: SI-A04u isolated coordinators cannot accept each other's tickets
    Given two coordinators created in the execution.rs unit-test module, one owner each, and each owner having allocated one request
    When the first owner's request is presented to the second owner's validate_request, admit and cancel
    Then each returns exactly the error "invalid execution request"
    And their owner ids and request ids are pairwise distinct
    # RED: isolated() State allocates owner 1 and request 1 in both
    # (execution.rs:9-16,64-118), so the second owner accepts the forged request

  Scenario: SI-A05 public identities are never reissued after close or reopen
    Given owner A closed after allocating requests
    When A2 reopens the same database in the same process
    Then A's stale request rejects on A2 with the existing invalid-ticket error
    # char today; preserved after R3

  Scenario: SI-A05u reissue across coordinators at unit level
    Given a coordinator dropped after allocating owner and request ids, and a new coordinator
    Then the new coordinator never allocates an id equal to any earlier id
    # RED: a fresh State restarts at 1

  Scenario: SI-A06 coordinator selection function
    Then for_database(Exclusive, native) is a fresh coordinator
    And for_database(Unproven, native), for_database(Exclusive, wasm32) and for_database(Unproven, wasm32) are global
    # unit-tested natively with an explicit target parameter

  Scenario: SI-A07 conservative fallback for unknown adapters
    Given owners A and B over a test wrapper adapter that reports StoreOwnership::Unproven
    When A has a queued unbegun request and B inserts
    Then B's insert is Busy exactly as today
    And native SQLite and owned memory stores report Exclusive
    # char of the conservative path, plus new capability values

Feature: Phase 1 JavaScript host queues preserve core authority (SPEC-0037 R3.5)
  Scenario: SI-J01 independent database handlers progress
    Given an unresolved yielding handler on A with an exclusive core coordinator
    When B executes a command on an independent database
    Then B commits before A's handler resolves
    # RED: the module-global host pump waits for A

  Scenario: SI-J02 independent reset and sync progress
    Given an active handler on A and an idle independent database B
    Then B resets and its background sync reaches prepare_push
    And A's command is unchanged
    # RED: global jobs makes B reset Busy and postpones its sync

  Scenario: SI-J03 same-instance FIFO and explicit cancellation
    Given two commands queued on A and the first handler suspended
    Then the second handler remains queued
    When an AbortSignal cancels the first
    Then the second commits and the first's late result cannot finish or publish
    # preserved control

  Scenario: SI-J04 a shared conservative core still serializes handlers
    Given A and B use one global scripted core arbiter
    Then B's handler runs only after its own begin is admitted following A's release
    And no host invocation-order guarantee across instances is inferred
    # preserved control

  Scenario: SI-J05 disposal is owner-scoped
    Given suspended handlers on A and B
    When B closes
    Then B's work settles without cancelling A
    And closing A releases its request and suppresses a later handler completion
    # preserved control and stronger cross-owner fixture

  Scenario: SI-J06 global admission Busy is a release wait rather than a sync failure
    Given A holds a global core permit and B starts sync
    Then B's prepare_push Busy is retained without an S fact or polling
    When A releases admission
    Then B proceeds through its existing release subscription and leaves no waiter

Feature: Phase 1 counters measure entries and stay out of production (SPEC-0037 R4)
  Scenario: SI-W01 instrumentation self-check
    Given a memory store of N rows in the default test build with work-counters dev self-enabled
    When StagedScope::begin runs
    Then rows_cloned equals N and inventory_rows_validated equals N
    # neither RED nor char: it verifies that the instrumentation is wired
    # before later RED cases depend on it

  Scenario: SI-W02 test-only features are absent from production feature graphs
    Given the release feature graph excluding dev edges
    # cargo tree --offline --locked -e features,no-dev -i <crate> --manifest-path <M>
    # <crate> is rusqlite, hoyadb-core, hoyadb-storage-sqlite and hoyadb-storage-memory
    # <M> is the workspace Cargo.toml (package hoyadb-bindings),
    # crates/hoyadb-native/Cargo.toml and crates/hoyadb-web/Cargo.toml, each also with
    # --target wasm32-unknown-unknown where that manifest builds for web
    Then no output line names the features work-counters, test-hooks or rusqlite functions
    # the pinned rusqlite declares functions = []; no dependency is added
    And the artifact build scripts (bindings/swift/build-artifacts.py, crates/hoyadb-web/build.sh, the crates/hoyadb-native build path) pass neither feature
    # guard; plain "cargo tree -e features" includes dev edges and is not the release graph

Feature: Counter completion before Phase 3 optimization (SPEC-0037 R4)
  Scenario: SI-W03 named work counters are non-vacuous before planners change
    Given unchanged planners and known record clones metadata clones packet encoding parsing validation and SQLite writes
    Then each corresponding counter is nonzero
    And counters for full metadata traversals grow with M on the unchanged baseline
    And strict SQLite open or catalog counts all three catalog consistency statements at N=10 and N=1000 against a direct statement-status oracle with existing measured work accounted for
    And exhaustive manually counted clones equal fully populated originals
    And instrumentation preserves the SI-Y06 characterization
    And post-conversion replacements are named in the R9 counter-test transition below before any assertion is replaced
    # RED first on zero increment sites; instrument minimally, then record SI-Y performance RED.

Feature: Phase 2 ownership capability and open validation (SPEC-0037 R5)
  Scenario: SI-V01 untouched corrupt row fails command-capable open
    Given a store with one untouched row of wrong kind, one missing a required field and one with a wrong row version
    And the same rows behind an Exclusive adapter and behind a wrapper reporting Unproven
    Then CommandDatabase::open fails for each on both
    And plain Database::open behavior is unchanged
    # RED: open succeeds and every command fails at begin

  Scenario: SI-V02 closed-store ledger edit is caught at open
    # char

  Scenario: SI-V03 Unproven adapter keeps the conservative path in addition to strict open
    Given a wrapper adapter reporting Unproven
    Then open validates the full inventory once
    And begin still snapshots and validates the full inventory
    And commits use the full SyncPlan
    # char for begin and commit; open is SI-V01

Feature: Phase 2 overlay staging (SPEC-0037 R6)
  Scenario: SI-O01 command touching one row
    Given a validated Exclusive store whose reads_ready capability is true
    When a command does fetch(id) and replace(id), measured from begin through handler completion
    Then rows_cloned is at most 3, from its named sites: the fetch result, the first image and the delta image
    And inventory_rows_validated is 0
    And it is equal at N, and fullscan-free on SQLite in that window
    # RED: begin clones and validates N rows
    # finish/commit bounds are SI-Y07 (Phase 3); the commit stays whole-store until R9

  Scenario: SI-O02 overlay oracle
    Given a fixed handler-script table run on both adapters
    # inserts, replaces, removes; ToOne-path predicates and orders whose
    # targets change in-scope; ordered limits whose window gains or loses
    # delta rows; HasMany and ManyToMany navigation with through-row edits;
    # link, unlink, proposed IDs, read-your-own-writes
    # limit 0 (no base query); limit None (unbounded base query); one id
    # affected through two paths (set union); a target edit that changes
    # membership; two predicates on one target; a dangling reference resolved
    # by an in-scope insert and one created by a delete; a count observation
    # whose relation-path predicate changes because its target was edited
    # (subtraction on base targets, addition on overlay targets)
    Then results and committed changes equal those of snapshot staging, pinned first on current code
    # oracle

    And a predicate-free overlay base query takes the validated fast path and still merges deltas and affected relation rows exactly

  Scenario: SI-O03 entity staging paths are equal at N
    # measured from begin through handler completion; commit bounds are SI-Y07
    # RED: full copy at entity_owner.rs:115, 249, 404, 474

  Scenario: SI-O04 configure emptiness check is equal at N
    # RED: snapshot at entity_owner.rs:81

  Scenario: SI-O05b Unproven merge retains full-snapshot staleness
    Given an Unproven wrapper and an outstanding merge with a page touching r
    When another writer edits an untouched row s between prepare and commit
    Then merge rejects StalePull and commits no row metadata ledger or notification change
    And the Exclusive SI-O05 touched-only bounds remain unchanged

  Scenario: SI-O05 merge prepare is equal at N
    # measured through prepare and reply; merge commit bounds are SI-Y07
    And a test-hook change to a touched row between prepare and commit still rejects StalePull
    # RED for counts; char for rejection

  Scenario: SI-O06 yielding Exclusive adapters keep awaited snapshot staging
    Given an Exclusive adapter whose reads each yield Pending once and wake the executor
    And reads_ready uses its provided false default
    When a test polling loop with a counting waker drives begin and the full SI-O02 script
    Then the script results and committed changes match the original snapshot oracle and ready path
    And begin copies and validates the full inventory before publishing the scope
    And all scope base reads use only the private immediate-read snapshot without a proof
    And no Phase-2 staging performance claim is made for this fallback
    # RED: the generic live overlay treats a legal Pending as BackendFailure

  Scenario: SI-O07 cancellation during snapshot acquisition releases admission
    Given SI-O06's yielding Exclusive adapter and a requested command
    When begin is polled until a base read is Pending and its future is dropped
    Then no active scope remains and rows model and ledger equal their prior state
    And admission is released so a new request and begin succeed and commit normally
    # char on the restored fallback; no impossible mid-scope suspension fixture

  Scenario: SI-O08 false immediate-read promises fail without partial staging
    Given a validated Exclusive adapter reporting reads_ready true whose selected base read yields Pending
    When a scope read or write encounters that read
    Then it returns Storage BackendFailure without retry or busy-polling
    And the delta and first images equal their state before the failed operation
    And a batch whose later primitive hits Pending also restores earlier primitives
    And later valid operations in the same scope finish with exactly their intended commit
    And Engine forwards readiness while an unknown adapter defaults to false
    # no new runtime dependency; built-in ready memory and SQLite paths stay unchanged

Feature: Phase 2 memory atomicity without whole-store copy (SPEC-0037 R7)
  Scenario: SI-M01 batch of k mutations
    Then rows_cloned is at most k and it is equal at N
    # RED: whole-store clone

  Scenario: SI-M02 error at mutation j of k restores every row, index entry and sync packet
    And a selective index query returns pre-batch results (extends index_cost p12)
    # char semantics on the new mechanism

  Scenario: SI-M03 panic during apply is undone by the guard
    Given a test-only panic injected after mutation j
    And separately inside a primitive: after the map change before index updates, within index removal or addition, between replacement index operations, and during table creation
    When it unwinds through apply
    And panic points cover each map change, partial index removal/addition, replacement between index operations, table creation and consecutive batch primitives
    Then the store, every index entry and every sync packet equal the pre-batch state, and the store is usable
    # new; double panic during replay aborts and is disclosed, not tested

  Scenario: SI-M04 existing memory adapter suites are unchanged
    # char

Feature: Phase 2 ordered limit and reactive bounds (SPEC-0037 R8)
  Scenario: SI-Q01 SQLite bounded order without predicates
    Given an owned CommandDatabase with Exclusive ownership after R5 open validation
    And a finite limit k no predicates no relation source and a direct non-optional order column
    Then sqlite_sort and sqlite_fullscan_steps are 0 and sqlite_rows_read is at most k
    And for fixed k distinct-value count d and schema the bounded sqlite_vm_steps and row reads are equal at N=10 and N=1000
    And the DESC walk makes at most d+1 next-value queries d equality ranges and k exact-id row gets
    And ASC and DESC return ascending ids within one giant tie group and a tie group straddling the limit
    And all-distinct values empty tables k=0 k>N and integral plus fractional Float values keep exact order
    And predicates optional columns and relation paths stay on today's query
    # bundled-engine physical counters are proof; system SQLite planner text is not
    # RED: today's sqlite_sort is nonzero and the work grows with N

  Scenario: SI-Q02 memory ordered limit 10 without a predicate
    Given an owned CommandDatabase with Exclusive ownership after R5 open validation
    Then memory_rows_visited is 10 and ordered_ops is at most 11, equal at N
    And a finite filter-free no-order query visits k rows in default ascending id order at both N
    And SI-O04 uses that case with k=1
    # RED: all candidates are visited

  Scenario: SI-Q03 reactive single-row update on a limit-10 group
    Given an owned CommandDatabase with Exclusive ownership after R5 open validation
    Then ordered_ops is at most 2 per affected id and visible entries taken equal 10, at both N
    # RED: re-sort of all keys

  Scenario: SI-Q04 optional, path, wrong-kind, dangling and Null orderings are unchanged on both adapters and reactive
    And the facade Engine forwards the owner's opaque ValidatedQuery to each concrete adapter, with the same bounded physical work and exact tie order at N=10 and N=1000 (DEF-0058)
    And on raw TypedStorage, plain Database and an Unproven CommandDatabase, non-optional wrong-kind numeric and boolean payloads keep today's result through CASE and the full sort
    # e.g. a raw Float(100) in a non-optional Integer column next to a valid Integer(5): DESC limit 1 still returns the valid row
    And adapter order equals the Rust API-0003 comparator for every value kind, including -0.0 versus 0.0 ties
    And an external compile_fail doctest cannot construct ValidatedQuery or invoke the fast path without a proof
    And a static check finds exactly the fetch and overlay base-window proof sites behind the core Exclusive plus validated-owner gate and rejects any third
    And the Unproven private-snapshot overlay never receives a proof and matches SI-O02's original oracle
    And every reactive group matches the previous full-sort oracle after each update, including raw plain and Unproven setups
    And those sequences cover Null wrong-kind DESC target edits entering/leaving the limit and ties at its boundary
    # char

Feature: Phase 3 delta sync commit (SPEC-0037 R9)
  Scenario: SI-Y01 entity patch c=1 and pull page c=100, Exclusive
    Then packets_parsed, packets_encoded, sqlite_rows_written and ledger_entries_cloned are equal at N and M
    And it is fullscan-free
    And cloned-entry counters include each nested record and metadata entry copied by enqueue finish push receive reconcile inverse entity patch pull and merge
    And a planner error midway restores the exact prior owner ledger data and model without publishing notifications
    And failed planning or cancellation before persist restores all first images and does not bump ledger_epoch
    And Pending after commit during notification publication retains the committed state and bumped epoch with the owner ledger equal to persisted rows
    And post-commit notification cancellation keeps API-0005's existing no-guarantee boundary and is never claimed as a rolled-back write
    And review confirms no LedgerTxn is held across an await in the converted planners
    And a C4 refusal records no first image and does not bump the epoch
    And a test-only invariant matches the cached tombstone total after every commit
    And metadata_entries_visited includes progress and format-2 rank work
    # format 3, or format 2 in append order
    # RED: full catalog, loads, rewrite

  Scenario: SI-Y02 format-3 bootstrap of 10 pages has equal per-page counters
    # RED: they grow

  Scenario: SI-Y03 a header, changed-packet or guarded-row change between plan and apply rejects with no partial write
    And core validator unit tests reject duplicate upserts deletes or expected ids overlapping upserts and deletes missing headers surplus expected packets frozen-envelope changes and header regression
    And forged untouched metadata escapes only the Exclusive delta commit but fails the next strict open
    And the same forged untouched metadata fails the default or Unproven full commit
    And an external compile_fail doctest cannot construct SyncDelta or turn SyncDeltaParts back into a proof
    And the static proof-site check accepts exactly the validated Exclusive persist construction and rejects another
    And an Engine forwarding test proves its delta call reaches the adapter
    # RED: no delta API

  Scenario: SI-Y04 default provided apply_sync_delta on an Unproven wrapper
    Then it calls apply_sync_plan with the full prior rows
    And the stored result equals the override path
    And a well-formed out-of-band change to an untouched packet between the default's full load and apply_sync_plan fails its full CAS without partial writes
    And malformed untouched state present before the load fails full validation; no detection of a well-formed untouched edit predating that load is claimed
    # new; conservative; the default deliberately checks more than the Exclusive override

  Scenario: SI-Y05 format-2 insertion at ordinal i upserts exactly M - i + 1 metadata packets
    # RED: full rewrite

  Scenario: SI-Y06 TC-0032, TC-0035, TC-0044 and SPEC-0038 recovery, policy, pull, merge and reset cases pass unchanged
    And s05 and s07 kill-barrier restarts are preserved
    And Reset retains its full plan and is excluded from SI-Y01 and SI-Y07 equal-at-N/M claims
    And the characterization is pinned before any planner conversion
    And a planner-level dual-fault characterization supplies a record-level invalid ledger which both ownerships still check together with an invalid snapshot and pins the existing ledger error first; a metadata-level ledger fault is pinned on the Unproven full-validation path only
    And that characterization names reconcile, inverse, finish_entity_scope, plan_receive, plan_push and the pull planner separately, pinning their actual pre-conversion validation order at each reachable entry
    And Unproven retains full validation at the existing reconcile and inverse entry, finish to_rows and persist points in the same error order
    # char

  Scenario: SI-Y07 command finish, entity commit and merge commit are equal at N, Exclusive
    Then the commit path of SI-O01, SI-O03 and SI-O05 is fullscan-free and equal at N
    # moved from Phase 2: until R9 the commit runs the full catalog twice and reloads the ledger
    # RED: full catalog and ledger loads

  Scenario: SI-Y08 bounded reconcile and inverse snapshots preserve every mapped reference
    Given a local id referenced from another table outside the closure and explicit guards
    And a correction mapping that local id to a target whose occupancy is checked
    Then the external reference is rewritten exactly and its row is guarded and committed
    And Exclusive results equal the full-snapshot Unproven twin
    And row visits are bounded by closure plan guard mapping and actual reference-rewrite rows at N

  Scenario: SI-Y09 an owner write stales an Exclusive merge through its epoch
    Given an Exclusive merge prepared before another successful owner write
    Then merge commit returns StalePull without partial writes or notifications
    And the Exclusive stale check visits zero metadata entries
    And failed persistence leaves the epoch unchanged

Feature: Phase 3 format 3 and offline converter (SPEC-0037 R10, R11)
  Scenario: SI-F01 first entity configuration writes ledger_format 3 and identity packets
    And it reloads
    # RED: missing

  Scenario: SI-F02 an existing format-2 store stays format 2 through every operation
    # operations: open, configure, reconfigure, patch, pull, reset in both modes, rebind
    And an existing format-3 ledger also retains its marker through those operations
    And to_rows followed by from_rows preserves the marker after both resets, configure and reconfigure, including an empty comparison-state shell
    And removing existing entity comparison state is rejected
    # char plus new

  Scenario: SI-F03 format-3 decode rejects malformed input
    # data/id mismatch, uppercase or odd hex, length overrun, invalid UTF-8,
    # mixed formats, and ordinal packets under a format-3 marker
    And raw hex suffixes at the checked MAX_BYTES-derived bound and one byte over are validated before allocation
    And fewer than four decoded bytes excessive table-length prefixes empty names invalid UTF8 and empty ids reject
    And canonical fixed-width prefixes retain their required leading zero bytes
    And from_rows and to_rows preserve packet-id order with unchanged trimming and local-reference rules
    # RED: missing

  Scenario: SI-F04 ("ab","c") and ("a","bc") produce distinct identities
    # RED: missing

  Scenario: SI-X01 the tool round trips canonical ledger bytes with exact reported counts
    Then every ledger id and canonical packet field byte is restored after v2 to v3 to v2
    And from and to are numeric formats metadata_entries is M and packets_rewritten is M plus one
    And canonical model and application dumps are unchanged and sidecars are absent after close
    # SQLite main-file byte identity after successful writes is not claimed
    # RED: missing

  Scenario: SI-X02 the tool rejects without writing
    # live owner (exit 3); no ledger, target already present, invalid source (exit 2);
    # missing store or malformed arguments (exit 2); key-file read error (exit 4)
    # wrong key/protected-open failure (redacted exit 5); existing backup (exit 6)
    # backup/conversion I/O or transaction failure (exit 7); persisted WAL source (exit 2)
    And pre-conversion refusals preserve source main-file bytes and leave no new files or sidecars after close
    And a transaction failure preserves exact logical source state and keeps the verified backup
    And no refused run repairs a hot journal or initializes a missing store
    # RED: missing

  Scenario: SI-X03 dry-run and backup
    Then dry-run preserves main-file bytes creates no missing store and leaves no new file or sidecar after close
    And validation uses READ_ONLY with a retained read transaction and refuses hot-journal recovery
    And a real run creates a create_new byte-verified backup before converting
    And a test-only sync-call recorder shows backup-file sync then backup-parent-directory fsync both before any source write
    And either sync failure prevents conversion and cleans up only this invocation's incomplete backup
    And it never prints key bytes plaintext recovery packets or rows
    And owned key buffers are zeroized on every exit
    # RED: missing

  Scenario: SI-X05 a wrong protected-store key is redacted exit 5
    Given a synthetic protected store and a different valid-length key
    Then the tool returns exit 5 with a static redacted error
    And source main-file and sidecar bytes are unchanged and no new file exists
    And stderr contains no key plaintext recovery packet or row

  Scenario: SI-X06 an existing backup is exit 6 and never overwritten
    Given a valid synthetic source and an existing requested backup
    Then the tool returns exit 6 and the backup's exact bytes remain unchanged
    And source main-file and sidecar bytes are unchanged and no new file exists
    And stderr contains no key plaintext recovery packet or row

  Scenario: SI-X07 backup preparation I/O failure is exit 7 before conversion
    Given a valid synthetic source and a test-only injected create_new or backup sync failure
    Then the tool returns exit 7 without converting the source
    And source main-file and sidecar bytes are unchanged and no new file remains after successful cleanup
    And cleanup never removes an existing backup
    And an injected backup-path swap before cleanup preserves the swapped-in file and reports exit 7
    And cleanup compares the current device/inode to the identity captured immediately after create_new
    And stderr contains no key plaintext recovery packet or row
    # A separately injected cleanup failure reports the incomplete task-owned backup and exit 7.

  Scenario: SI-X08 read-only validation refuses persisted WAL without touching sidecars
    Given synthetic unprotected and correctly keyed protected WAL-mode stores
    And variants closed with no WAL/SHM sidecars and with existing WAL/SHM bytes
    Then the first read under READ_ONLY and locking_mode EXCLUSIVE returns unsupported-source exit 2
    And main-file and existing sidecar bytes are unchanged and no new file exists
    And a wrong protected-store key returns redacted exit 5 before WAL admission
    And protected IOERR_LOCK is classified on the same retained read-only connection with the existing cipher_integrity_check
    And failure rows give exit 5 while zero rows still gives unsupported exit 2 and never admits conversion
    And unavailable or failed integrity classification fails closed without printing its result text
    And the documented full-file work and preparation-time proportional failure-row memory are accepted costs
    And hot-journal refusal still gives exit 2 and Busy still gives exit 3 before this classification
    And an ordinary rollback-journal source passes the same opener
    # This pins the target engine's lock behavior, including encrypted page-1 headers.
    # If refusal before WAL/SHM access cannot be demonstrated, conversion remains gated.

  Scenario: SI-X04 store conversion is explicit native tooling only
    Then no facade SDK or CommandDatabase path calls either conversion tooling entry
    And ordinary native and WASM release graphs exclude ledger-convert-tool
    And the binary requires explicit ledger-convert-tool and the store entry is absent without that feature
    # static call search plus positive and negative feature/build controls

  Scenario: SI-XL01 a raw writer already holds RESERVED before converter validation
    Then the read-only pass validates but RW BEGIN IMMEDIATE returns Busy exit 3
    And source main-file and journal bytes are unchanged

  Scenario: SI-XL02 a raw writer takes RESERVED during the handoff
    Given RO has validated and still holds its read transaction
    Then converter admission returns Busy exit 3 with no recovery or source change

  Scenario: SI-XL03 a real killed writer leaves a hot journal
    Then a hot journal present before RO admission returns recovery-required exit 2
    And one published between validation and RW admission returns Busy exit 3
    And each preserves exact main-file and journal bytes without replay

  Scenario: SI-XL04 a raw reader prevents conversion commit
    Given converter RW admission succeeded and RO was then closed
    And a reader holds SHARED before commit
    Then commit fails exit 7 with exact logical source rollback and the verified backup retained

  Scenario: SI-XL05 no owner enters during read-to-reserved handoff
    Then an existing hoyadb owner makes converter exit 3
    And a competing production open attempted at every handoff barrier returns StoreInUse

  Scenario: SI-XL06 successful handoff and conversion
    Then the requested format and exact counts are reported after commit only
    And the backup equals the source main-file bytes from before any converter write
    And the roundtrip restores exact canonical ledger model and application bytes
    And no sidecar remains after close
    # Child processes and atomic readiness publication; synthetic stores only.
```

## Execution notes

- **Default suite.** Phase-1 cases are Rust tests in the default
  `cargo test --workspace --offline --locked`.
  SI-J01..SI-J06 are JavaScript host cases in the existing Node client suite;
  they supplement rather than replace the Rust coordinator cases.
  - SPEC-0037 R2's stimulus is `#[cfg(test)]` in the crate's own unit tests.
  - `work-counters`, and `test-hooks` if needed, are enabled for tests by
    dev self-dependencies, so no case is silently skipped. SI-W02 proves
    production excludes them.
  - SI-L03 uses the existing child-process barrier pattern
    (`storage-sqlite/tests/security_engine_cases.rs`).
- **Held targets.** No JDK/Android run.
- **WASM.** WASM-specific execution is a separate root decision; SI-L06 and
  SI-A06 are source and unit checks only.

## R9 counter-test transition

SI-W03 is the pre-refactor non-vacuity record. Its source-bound baseline logs
remain immutable. These replacements are named before conversion; no other
existing assertion is retired by this table. Stage names refer to the R9 plan:
S3 converts single-call planners, S4 adds the session overlays and tombstone
cache, S5 adds delta persistence and adapter overrides, and S7 accounts for
format-2 rank work. Intermediate expected REDs stay labelled as RED.

| Existing test on both adapters | Replacement and stage |
| --- | --- |
| `si_w03_patch_counters_are_nonzero_and_grow_with_m` | At the S3 conversion that first invalidates Exclusive growth, replace it with `si_y01_exclusive_patch_work_is_bounded_at_m` and `si_y04_unproven_patch_keeps_full_validation_work`. The Exclusive conformance case requires equal-at-M counters, `ledger_entries_cloned >= 1` and encoded packets at least equal to the touched packets at both M. Its core companion `si_y01_entity_patch_records_bounded_first_images` drives the public patch operation on a validated Exclusive owner and observes the actual guard through test-only inspection: at M = 10 and M = 1000, the first-image count is at least one and equal, and the touched record and metadata keys are exactly the expected keys. A compiling observation reports zero or absence before conversion and fails by assertion; missing symbols are not RED. The observation never changes control flow and adds no counter field or runtime API. The conformance case's remaining persistence/rank bounds stay RED until the corresponding S5/S7 work is GREEN. The Unproven twin retains nonzero, growing full-path packet parse/encode and metadata-visit counts. |
| `si_w03_progress_visits_every_metadata_entry` | At S4, replace it with `si_y01_progress_reads_cached_tombstones_without_metadata_visits`, requiring zero metadata visits per progress read, plus `si_y01_cached_tombstone_total_matches_recompute`, the test-only cache invariant checked after commits. |
| Core C4 exact visit controls `c4_counter_row_undo_match_examines_no_touch_metadata` and `c4_counter_no_match_examines_every_touch_metadata_entry` | At S3, successful rebase adds exactly one metadata visit to capture W's rollback first image. Their formulas become `9 + 1 = 10` and `4 + 1 = 5`. The refused unpaired case remains exactly `3`, with no first image. This changes neither the touched-entry definition nor any equal-at-M/N bound. |
| `si_w03_receipt_counts_both_sync_packet_images` | At S3/S5, replace with `si_y03_receipt_counts_exact_delta_packet_images`: the configured-entity fixture already has both headers and the command packet, so the core clones exactly those three expectations and the changed command upsert (minimum 4 including adapter copies). Preserve `si_y04_receipt_counts_both_full_packet_images` on an Unproven wrapper with the original minimum 6. An absent entity header never counts as a cloned Row. |
| `si_w03_merge_counts_session_packet_image` | At S4, replace with `si_y09_empty_merge_retains_no_packet_inventory` requiring zero Row copies for an empty Exclusive page, and `si_y04_empty_merge_retains_full_packet_inventory` retaining the Unproven fixture's exact `M + 2` copied packets. Both owners abort the prepared empty session after measurement. Unproven full application snapshot copies are separately counted; their total is `2M + 2` for this fixture (one snapshot copy plus the two-header/M-packet inventory). |

Keep the exhaustive counted-clone and comparison controls,
`si_y06_reset_keeps_full_plan_work`, SI-W01 and the Phase-2 overlay/ordered/merge
controls. Reset still uses its full plan. Capture work counters before test-only
diagnostics, ledger dumps or cache recomputation; never suppress production
increments. Equal-at-M fixtures use unrelated M entries and pin format-2 append
order; required middle-insertion suffix visits remain counted under SI-Y05.
