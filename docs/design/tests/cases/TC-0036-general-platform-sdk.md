---
id: TC-0036
title: General consumer packages preserve owned core behavior across platforms
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-26
parents: [TP-0001]
verifies: [REQ-0046, REQ-0034, NFR-0002, NFR-0005, SPEC-0033, API-0011, THR-0020]
tags: [phase-seven, bindings, conformance, packaging]
---

# TC-0036: General consumer packages preserve owned core behavior across platforms

Acceptance before implementation, not a passing report. All executable targets
use the same Rust facade/core. Maintain per-target result/source/artifact matrices;
stub-only Node tests do not establish Rust/native/mobile behavior. ADR-0035
records the now-approved command-ID clarification and preserves its exact assertions.

## Fixed acceptance fixtures

Two unrelated **caller-supplied** schemas, neither built into the facade:

- A: authors(name:String), articles(title:String,score:Integer,ratio:Float,
  active:Boolean,at:Timestamp,note:optionalString,author ToOne authors), tags and
  article_tags explicit two-ToOne through table with articles.tags ManyToMany.
- B: sensors(label:String), readings(value:Float,at:Timestamp,sensor ToOne),
  sensor HasMany readings. No orders/customers/items tables.

Fixture builders live in tests/consumer apps, not production dispatch. Supply
authored versions exercising every MigrationStep, exact raw synthetic key and
deterministic times/namespace. Tests assert concrete normalized rows/state/errors,
not only successful JSON parse or matching two stubs.

```gherkin
Feature: General schemas share one production implementation
  Scenario: G01 arbitrary caller schemas and every relation kind
    Given caller schemas A and B and explicit memory or protected engine
    When each opens through the same general facade with no preinstalled tables
    And public CRUD relations and managed link unlink execute
    Then exact expected rows and navigation IDs match the Rust public owner
    And neither schema depends on fixture table names or magic version numbers
    And declared invalid reserved names aliases and through mappings reject in core

  Scenario: G02 authored schema codecs cover every existing migration step
    Given populated protected schema A v1 and caller-authored migration chain
    When AddTable DropTable AddColumn DropColumn RenameColumn Backfill SetRelations run
    Then all exact endpoint rows relations defaults and schema identity match core
    And None differs from explicit Some Null for an optional added column
    And unknown keys duplicate map keys ambiguous chains wrong endpoints downgrade reject
    And every failure preserves original data and recovery bookkeeping
    And reopen applies no implicit SDK upgrade reset

  Scenario: G03 lossless typed cells and boxed command JSON
    Given null missing optional Unicode reference timestamp Boolean and finite float values
    And i64 minimum maximum integral Float42.0 fractional1.25 negative zero and float extrema
    When values cross actual JS Swift Kotlin and RN supported boundaries
    Then their exact core kinds normalized values and row IDs survive
    And unsafe JS integer Numbers NaN infinity overflow malformed UTF8 raw numeric tokens reject
    And core business limits and tighter tagged wire representation limits are reported honestly
    And retained mappings to longer absent canonical reference targets are included in pre-commit reply budgets

  Scenario: G04 every query concept delegates to the existing builder
    Given related rows and ties nulls missing references and managed memberships
    When Eq Ne Lt Le Gt Ge In order directions limit and lazy navigation execute
    Then exact ordered ID windows equal existing public Rust fetch
    And contradictory source target query declarations reject
    And empty In and sets across4096 and32766 use unchanged core behavior
    And no host predicate filter sort or merge algorithm substitutes

  Scenario: G05 general observations retain sharing and recoverable delivery
    Given membership columns and count observers on equivalent queries
    When writes related-row reorders value-only edits and injected storage failures occur
    Then initial and subsequent exact snapshots errors counts and cache-sharing match API-0005
    And managed observers receive their initial snapshot without manual drain or a mutation
    And preparing a queued push delivers its managed Sent outcome without receipt another write or manual drain
    And receiving a wire receipt delivers its managed outcome without another operation
    And these observable transitions do not make the sync scheduler wake itself in a loop
    And one outstanding host deadline timer delivers pending counts without being postponed by later writes
    And repeated explicit refresh failure is observable without write-driven error storms
    And the16ms count deadline is measured with the injected monotonic clock
    And unsubscribe inside a callback suppresses later buffered callbacks for that token
    And an SDK owner's advanced raw drain is Busy during managed registration or live delivery
    And explicit managed drain invokes selected callbacks without consuming them invisibly
    And a committed write while an owned initial reply is suspended delivers a follow-up without another write
    And a retired Kotlin drain paused in its clock cannot consume a replacement owner's initial event
    And a throwing Kotlin observer followed by explicit subscription close retires its core token and drain guard exactly once
    And JS invokes callbacks in event order without awaiting them and their asynchronous continuations may interleave
    And a throwing or later-rejecting callback retires only its listener and reports CallbackFailure once
    And recoverable data errors keep the listener while a generic error event retires it
    And owner-wide managed drain failures reach the typed background sink and are not broadcast to listeners

Feature: Ownership spans awaits without borrowed native state
  Scenario: G06 commands preserve isolation FIFO and cleanup
    Given two owners and an awaited host command handler
    When it reads then writes through scope and suspends before finish
    Then read-your-writes is scoped and outside rows remain unchanged
    And no registry lock Rust borrow or transient native-call idle permit spans the host callback
    And the staged scope retains its API-0006 admission permit until finish or abort
    And handlers of the same database obey FIFO while independent proven-exclusive native databases progress
    And unproven adapters and WASM keep the conservative global fallback
    And the tracked scope exposes no nested request or execute capability
    And awaiting an independently captured outer queued child inside its parent is documented misuse
    When the handler returns throws cancels or its owner closes
    Then exact atomic finish or abort and handle cleanup matches API-0006
    And Kotlin RN and Swift RN mixed-wrapper waiters resume after a foreign terminal release without polling
    And a release arriving while begin false is suspended cannot be missed
    And native release notifications carry no arguments IDs operations or data
    And registration failure creates no command ticket and no permit
    And cancellation close reset and stale queued callbacks remove or invalidate every release listener
    And a Busy owned-response retry retains its response without repeating transport or mapping

  Scenario: G07 proposed IDs preserve core validation and custody
    Given an ordinary unconfigured table and injected generator test-fixed-id-001
    When one insertion crosses the public package API
    Then the row ID is exactly test-fixed-id-001 without a second generated ID
    And empty duplicate exhausted and thrown generator results reject without partial writes
    And a failed batch does not advance core counters or leak a proposed ID into another call
    And abort drop and later default-generator insertion preserve core generator custody
    And automatic entity creation keeps its strict core namespace counter identity

  Scenario: G08 command injected prefix clarification remains explicit
    Given original TC-0016 expects command ID test-fixed-id-001
    And API-0006 requires surviving command-created IDs to have local prefix
    Then no test or acceptance statement silently changes either contract
    And ADR-0035 records the operator's explicit prefixed-command choice
    When that choice explicitly adopts local:test-fixed-id-001 for commands
    Then the command row uses exactly that injected value and normal CRUD retains G07
    And an unprefixed command proposal still cannot bypass recovery validation

  Scenario: G09 pending open and close cannot publish or leak a late owner
    Given an actual protected open reply paused after submission
    When a second open or close then reopen targets the same client
    Then opening reserves it synchronously and second open is Busy
    And close waits for late acquired owner disposal before allowing replacement open
    And neither keys nor stale Open state or late callbacks escape
    And a failed open releases only its own reservation
    And a prepare_push reply suspended across close reopen cannot invoke replacement transport with old bytes
    And a pending reconciliation list suspended across close reopen cannot invoke replacement mapper
    And stale sync cleanup cannot cancel or mutate the current owner run
    And a delayed old RPC rejection is suppressed while a current-owner sync error remains visible
    And expected owner epoch and native handle are captured together before an owned drain or Busy retry dispatch
    And close reopen while an old Kotlin drain clock is paused preserves exactly one replacement initial snapshot

  Scenario: G10 callback affinity stale handles and close cleanup
    Given actual UI executor selected and active rows status outcome progress callbacks
    When request handles are reused forged cross-owned or closed and callbacks reenter
    Then static errors preserve data and no callback uses stale native memory or locks
    And C ABI null plus nonzero length and over-limit lengths reject before dereference
    And valid owned JNI Swift buffers satisfy caller readable-span lifetime obligations
    And no test claims arbitrary forged valid-size addresses are safely catchable
    And callbacks run on their documented executor
    And an inline Android release listener throwing IOException cannot replace the collected native reply or suppress later listeners
    And throwing asynchronous release listeners retire without damaging later owned operations
    And a plain Kotlin continuation waiting for a later managed callback cannot occupy the shared native reply channel
    And readonly C and JNI open preflight shares exact original-frame validation without registry owner token provider or filesystem actions
    And its admitted collector can reenter ordinary calls with no held registry mutex
    And reset close worker replacement or native owner drop invalidates generation caches
    And current live listeners keep their owned delivery and abandoned listeners are released
    And a checked exception from a Kotlin observer cannot retain raw-drain Busy after retirement or close/reopen
    And a custom Kotlin timer throwing after native success cannot mask the committed Row or retain an armed deadline or observer token

  Scenario: G11 bounded replies cannot hide a committed operation or lose events
    Given a valid near-bound mutation and a success report exceeding the wire frame
    When the public call is attempted
    Then it rejects before mutation and preserves rows counters queues and events
    Given aggregate push or drain data larger than one frame
    When successive owned prefix pages are read
    Then exact original bytes and event order appear once with has_more
    And no prefix selection repeats prepare or discards events on serialization failure
    And an event that cannot be a sole page item terminates only its subscription with one generic LimitExceeded error event
    And other subscriptions' events are delivered once in original relative order across pages
    And a large readonly fetch returns LimitExceeded rather than truncated rows
    Given 128 distinct keyed replacements with small String fields in one table
    Then a fitting exact before-after report succeeds without unrelated prior row accumulation
    And repeated same-ID transitions use their latest staged image including removals
    And unknown core-allocated insertion IDs remain conservatively bounded
    Given a two-item prefix whose true-flag frame is exactly one MiB
    And its false-flag frame would exceed the byte limit by one
    Then successive valid prefixes make progress and deliver each original item once
    And an intrinsically oversized drain event terminates only its subscription
    And push prefix failure retains its original envelope unchanged

Feature: Passive custom merge keeps all semantics in core
  Scenario: G12 foreign merge offer uses current staged row and shared validator
    Given a pull page with two changes for one record and a host hook for its table
    When the first immutable owned offer is awaited and valid candidate returned
    Then the second default is computed in core from the first validated staged candidate
    And valid custom values equal the synchronous Rust hook result
    And no row checkpoint progress or notification publishes before atomic commit

  Scenario: G13 invalid merge proposals cannot rewrite authority or unrelated fields
    Given the same page and current clocks tombstones ties and removed fields
    When a hook changes identity existence schema bounds unrelated tied or removed fields
    Then the exact shared core validator rejects and discards the whole plan
    And current rows metadata checkpoint queue and events remain unchanged
    And no foreign callback runs while Rust holds a permit or borrowed row
    And existing authoritative correction does not silently acquire a host hook

  Scenario: G14 stale merge plans never consume checkpoints or rebind old bytes
    Given an offered foreign merge awaiting the host
    When any local model ledger receipt cancel reconciliation reset or configuration change commits
    Then resume or commit retires the session and pull ticket as StalePull
    And old page bytes are never rebound to a new revision
    And a fresh fetch starts from the unchanged checkpoint
    When the only obstruction is Busy with unchanged expected state
    Then the exact original owned proposal may retry after FIFO idle without rerunning hook

  Scenario: G15 merge commit failure cancel and close are atomic
    Given a ready validated session and a real SQLite deferred or trigger commit failure
    When commit is attempted
    Then no staged rows checkpoint progress or callback is delivered
    And retry succeeds only if its complete expected state remains valid
    When the hook throws cancels aborts or closes during any offer
    Then every stage input handle and ticket is retired without leaked admission
    And hidden optimistic no-op and live recovery protection still reject losing pages

Feature: Secure packages expose existing sync status and testing support
  Scenario: G16 actual target crypto keys and backup policies preserve Phase 6
    Given binary key open through real native browser Android and RN packages
    When missing wrong malformed keys foreign plaintext or tampered files are used
    Then API-0010 secure defaults and byte preservation hold without fallback
    And key bytes never enter ordinary JSON transport Debug errors or diagnostics
    And invalid ABI duplicate original-frame keys and unknown variants reject before VFS initialization
    And pure preflight accepts the same near-limit original frames without quoted wrapper overhead
    And RN original duplicate keys raw-number schema options namespace and binary-key failures reject before OS directory getters
    And RN memory opens invoke no directory or backup policy
    And RN protected open accepts only the default marker and one-component name not arbitrary native roots
    And an expanded default path that exceeds wire bounds rejects before child creation or backup changes
    And actual RN Android and Apple controlled roots backup readback and nonsymlink checks precede the one key-bearing open
    And actual Android provider entropy failure has no weak clock fallback
    And browser harness transfer controls reject foreign names open files and oversize buffers
    And ordinary packaged WASM exports omit every test-control capability
    And target cipher profile sidecars temporary storage and killed-process recovery pass
    And Apple exclusion Android noBackup root and browser unsupported controls are truthful
    And Apple and Android backup policy is fixed by an atomic owned claim or shared lock and policy-rejected opens write no store file or flag

  Scenario: G17 mixed queue outcomes corrections and pull reuse unchanged core
    Given command and entity writes including dependent local IDs and explicit clocks
    When actual transport returns reordered partial receipt corrected rejected and duplicate outcomes
    Then exact correlation retries mappings inverses tombstones retention and typed correction hold
    And schema-aware authoritative clocks are never invented by wrappers
    And actual restart or page reload retains queue checkpoint and exact frozen retry bytes

  Scenario: G18 persisted attempt and deadline status is payload free
    Given queued sent acknowledged final cancelled and dependency-blocked records
    When status is read live and after protected reopen
    Then attempts equals the persisted u32 count
    And next_due_ms is present only for Sent retry-eligible records
    And existing aggregates kind table recordId blocking IDs and bounded history match core
    And identity args result reason frozen undo payloads are absent
    And safe diagnostics remain exactly their ten count Boolean fields

  Scenario: G19 production jitter and deterministic retry scheduling share one planner
    Given injected clock jitter extremes options and a transport that fails
    When send retry resume and receipt occur at exact chosen times
    Then durable attempts deadlines frozen bytes and125percent maximum jittered cap match API-0006
    And no empty push early resend busy spin or automatic cancellation occurs
    And production native and browser entropy samples remain in0 through2500 basis points
    And an external wire receipt wakes a settled sleeping driver before its future retry deadline
    And its local mapper reconciles once without another transport push or external wake
    And preparing an empty due queue never wakes its own scheduler in a loop
    And Busy retains response mapper output and hook proposal without another network call

  Scenario: G19 retired Swift sync driver does not resume into application callbacks
    Given a successful prepare or pending-reconciliation reply held before delivery
    When the driver stops or is replaced on the same owner without close or reset
    And the old reply is released
    Then no new old-driver transport mapper or error callback is invoked
    And the current driver still sends and reconciles its recoverable work once

  Scenario: G20 memory consumer touches no platform storage
    Given an explicitly preloaded core runtime and the packaged memory factory
    And path key OPFS filesystem and network hooks that fail if called
    When schema relations observation and command handler execute
    Then exact production behavior matches protected adapter
    And zero path key provider filesystem or OPFS calls occur
    And protection is exactly false none not_applicable not_applicable
    And unsigned monotonic drain time is not rejected by a UTC year9999 bound

  Scenario: G21 fake and actual transport use the same exact correlation seam
    Given packaged fake command handlers returning only business fields
    And entity handlers for two writes to the same record
    When pushes are applied rejected delayed and dropped before ACK
    Then the fake attaches each command key or entity table recordId writeId automatically
    And reordered responses follow exactly the actual transport receive path
    And a dropped ACK leaves Sent eligible for the existing durable retry
    And core json_decode and json_encode preserve i64 extrema and integral Float wire kinds
    And no opaque envelope is parsed through host JSON Numbers

  Scenario: G22 all original offline local operations succeed
    Given real memory and protected package consumers whose every push and pull rejects
    When schema declaration migration query live subscribe command entity write and queue status execute
    Then zero documented local operations fail or await a successful network call
    And schema open performs zero network attempts
    And an explicit retained memory_store closes and reopens the same real backend from v1 to v2
    And a failed authored reopen leaves its original closed memory state unchanged
    And duplicate active open or memory_drop is Busy and closed explicit drop releases it
    And a preloaded runtime plus injected namespace touches no entropy or storage bootstrap API
    And fixed clock handler timestamps and injected IDs are deterministic

Feature: Consumer packaging proves claimed platforms
  Scenario: G23 local artifacts install outside the checkout
    Given packed local npm assets declarations SwiftPM XCFramework Android AAR and RN package
    When independent clean consumers import typecheck build and execute
    Then no relative spikes source path or preinstalled fixed schema is required
    And actual browser macOS Swift iOS simulator Android Kotlin and RN iOS Android run
    And device ABI slices and16KiB ELF ZIP checks are labeled build-only unless executed
    And user devices stores keys and existing simulator AVD state remain untouched

  Scenario: G23 Android consumers preserve explicitly selected 32-bit ABIs
    Given the existing two-ABI local package default and an explicit four-ABI selection
    When the approved provider and JNI recipes build armeabi-v7a and x86
    Then every selected slice is bound to current sources and inspected independently
    And ELF32 ARM and Intel 80386 checks retain the exact exports NEEDED and16KiB LOAD guards
    And unknown duplicate missing mismatched or corrupted selected slices cannot produce a certificate
    And a fresh owned32-bit process opens a protected store with pending entity writes
    And later processes reconcile and forget producers while durable aliases and child references survive
    And wrong-key open preserves bytes and the original correct key reopens the protected store
    And actual process width page size ABI translation and artifact hashes are recorded
    And the earlier16KiB consumer gate remains unchanged and build-only results remain explicit

  Scenario: G24 API compatibility full regression and truthful closure
    Given versioned header exports declarations Swift Kotlin API snapshots and protected prior fixtures
    When packages upgrade reopen and execute authored migrations
    Then no undeclared breaking API or implicit local reset is introduced
    And full applicable unchanged conformance security Node native browser mobile and container gates pass
    And independent Claude review resolves demonstrated issues rather than quotas
    And exact source artifact dependency license and runtime evidence matches claimed targets
    And no package publication tag push or merge occurs without separate authorization

  Scenario: G24 Kotlin relation-source formatting keeps implicit output opaque
    Given a Source holding a synthetic private row identity
    When it is implicitly formatted alone and inside a collection
    Then neither representation includes that identity
    And explicit id access and exact query-source wire encoding retain it
    And data-class copy equality and hashing retain their original semantics

  Scenario: G07 submitted Swift finish preserves its result across close
    Given an actual committed finish whose successful host reply is held
    When its owner closes and a replacement owner opens before the reply is released
    Then the original command returns its exact committed key and result
    And independent retained-store reopen proves its row and recovery record
    And no stale completion changes replacement rows subscriptions or scheduling
```

## Original acceptance mapping

TC-0016: no filesystem→G20; parity→G01/G05/G06/G20; fake contract→G21;
automatic correlation→G21; rejection→G21; dropped ACK→G21/G19;
clock→G22; exact ID→G07 plus explicitly pending G08 conflict. Do not mark the
original command-ID spelling verified until the operator clarification exists.

TC-0017: schema/migration/query/subscription/command/entity/queue/zero failure
scenarios each run in G22, backed by G01/G02/G04/G05/G06/G17/G18 respectively.
Existing shared functions reused unchanged retain their exact asserted values;
platform smoke tests are additional translation evidence, not replacements.

## First implementation slice — no new external dependencies

After formal review/commit: G01–G04 exact codecs and real Rust memory/protected
facade dispatch; generic scalar CRUD/query/link; schema/migration operations;
G09 reservation tests; G11 mutator preflight and known paging preservation;
G20 memory no-FS and exact binary-key boundary. Default IDs only initially; new
proposed-ID/merge methods may be stubs returning a static unsupported capability
until their own reviewed REDs. Such stubs are never reported as full Phase 7.
Node tests use a fake *transport* only to test translation/reservation, then an
actual WASM memory owner for behavioral acceptance. No fake database schema.
Start with compiling facade dispatch stubs producing expected missing-operation
behavioral RED; wrong import/link/build is setup, not the regression proof.
