---
id: TC-0040
title: SDK observation termination background failures and backup policy claims
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-26
parents: [TP-0001]
verifies: [SPEC-0036, REQ-0007, REQ-0045, REQ-0046, DEF-0033, DEF-0037]
regression_for: [DEF-0033, DEF-0037]
tags: [purpose-audit, observations, background-errors, backup, concurrency]
---

# TC-0040: SDK observation termination, background failures and backup policy

These are acceptance cases before implementation, not a passing report.
Fixtures are synthetic, offline and task-owned. Each case first fails on
unchanged source for its stated behavioral reason; a missing symbol or fixture
does not count. Results are claimed only for executed targets. Kotlin,
React Native Android and JVM cases are specified while the
JDK/Gradle/Android holds remain. iOS lock behavior and the RN iOS runtime stay
unverified until executed. These cases supersede the facade test
`g11_undeliverable_single_snapshot_remains_until_cancellation`; its
cancellation assertion is kept in OD05.

```gherkin
Feature: Oversized observations terminate one subscription only (DEF-0033)
  Scenario: OD01 healthy subscriptions continue past an oversized snapshot
    Given owner O with membership S1 on table big, membership S2 on table small and queue status Q
    And two valid writes making S1's next snapshot exceed a sole-item frame while S2 and Q change
    When O drains
    Then the drain succeeds with S2's snapshot, Q's status and one generic LimitExceeded error event for S1 in original order
    And later writes to small deliver S2 again and nothing for S1

  Scenario: OD02 single terminal event and deterministic recovery
    Given OD01 after S1's terminal event
    When O drains repeatedly and writes to big
    Then no S1 event appears and unsubscribe S1 returns false
    And a new subscription on big with limit 10 receives exactly one initial snapshot of 10 rows

  Scenario: OD03 the triggering mutation keeps its committed result
    Given the write that makes S1 oversized through each SDK
    Then the write returns its committed row and stored state matches
    And the background sink receives only the terminal report for S1
    Given separately a throwing custom timer set and a throwing custom timer clear
    When an ordinary mutation or handler has already committed
    Then its exact committed row is returned and remains stored
    And each timer failure reports one redacted timer fact
    And healthy subscriptions receive every later event of their page

  Scenario: OD04 managed terminal and background error parity
    Given JavaScript, Swift and Kotlin owners each with onBackgroundError
    When a subscription terminates, an owner-wide drain fails, a callback throws, a JavaScript returned Promise rejects and automatic sync push fails
    Then each listener receives only its own typed terminal form once and retires
    And the sink receives each fact once as source kind and token with no message key payload or row
    And Swift and Kotlin startSync onError still receive the sync failure
    And committed mutation results are unchanged

  Scenario: OD05 frame-exact termination boundaries and controls
    Given a sole-item frame that fits with has_more true but exceeds the limit by one byte with has_more false
    Then the event is delivered when at least one other buffered event remains after removing its subscription's later events
    And the event terminates its subscription when no other buffered event remains
    Given a non-first event that does not fit
    Then the page ends with has_more true and the event is tested as the first item of the next page
    Given S1 unsubscribed before drain, or its owner closed or reset
    Then no S1 terminal event is delivered and other buffered events follow existing rules
    Given an oversized outcome event for sync subscription R
    Then R receives one generic terminal event and Q continues

  Scenario: OD06 explicit bounded query windows
    Given 3000 rows whose full result exceeds one frame
    Then an unbounded fetch returns LimitExceeded and changes nothing
    And fetches ordered by id with limit and id greater than the last id return every row exactly once

  Scenario: OD07 exactly-once background facts
    Given a terminal event whose callback throws
    And a listener with two pending Promises that both reject
    And a callback-failure retirement whose core unsubscribe fails
    And two coalesced managed drain requests sharing one failure
    And an explicit JavaScript drain whose callback throws
    And late Promise rejections after close
    Then the sink receives terminal then callback, one callback, one unsubscribe, one drain and one callback report
    And the explicit drain returns its own result
    And nothing is reported after close
    Given an asynchronous sink that rejects and a malformed terminal error kind
    Then the sink rejection is contained without an unhandled rejection
    And the malformed kind is reported as BackendFailure
    Given an earlier callback Promise still pending when its listener terminates
    Then its later rejection is suppressed
    And a failure from the terminal invocation itself still yields terminal then callback

  Scenario: OD08 recoverable data errors stay subscribed
    Given a forced core materialization failure producing a data error event for subscription S
    Then S's listener receives the recoverable error and is not retired
    And after refresh S delivers rows again
    And no core path emits a data error of kind LimitExceeded

Feature: Native backup policy is fixed by location (DEF-0037)
  Scenario: BK10 native rejection codes reach the shared JavaScript runtime safely
    Given the real RN channel and JavaScript runtime with a scripted native module
    When open rejects with InvalidRequest, BackupPolicyFailure, Busy or Closed
    Or call rejects with InvalidRequest or Closed or namespace rejects with Closed
    Then the caller receives that operation's permitted static kind and identical static message
    And no native message path key payload or cause is copied
    Given BoundaryFailure or an unknown malformed or throwing code property
    Then the caller receives BackendFailure without exposing the original exception
    And both synchronous native throws and rejected Promises follow those rules
    And the new Error's own enumerable keys are exactly kind with no native code userInfo nativeStackAndroid or cause
    Given call rejects Busy or namespace rejects BackupPolicyFailure
    Then the caller receives BackendFailure promptly without an admission release wait
    Given native Closed during a managed drain or sync attempt
    Then exactly one background fact reports Closed without a retry loop
    And pre-native InvalidRequest validation errors retain their kind
    And successful complete wire replies remain byte-identical

  Scenario: BK01 opposite policies in one Apple directory
    Given an empty owned directory D and store a opened excluded
    When store b is opened in D included, and symmetrically in reverse order
    Then the open is policy-rejected with BackupPolicyFailure
    And D's flag, reservation and a's bytes are unchanged
    And b's store file does not exist and only the permitted empty claim file may have been created

  Scenario: BK02 reopen with a changed flag
    Given an existing store under one policy on Apple or Android
    When it is reopened with the other flag
    Then the open is policy-rejected with BackupPolicyFailure
    And no store file is created and only permitted empty bookkeeping is written
    And the store reopens normally with its original flag

  Scenario: BK03 legacy directories are recorded or rejected, never migrated
    Given an unreserved Apple directory with existing entries and flag f, and an Android name present under both parents
    Then an Apple request for f records reservation f without a flag write
    And an Apple request for not f is policy-rejected leaving only an empty claim file
    And the Android ambiguous name is policy-rejected
    And every pre-existing file keeps identical bytes, inode and flag

  Scenario: BK04 path, key and claim-file controls
    Given symlinked or noncanonical directories, multi-component names and wrong key lengths
    And symlinked, hard-linked, foreign-owned, oversized or garbled claim files
    And a directory swapped after validation and a claim path replaced while held
    Then each rejects with BackupPolicyFailure or its existing error before any flag or reservation write
    And owned Rust key buffers retain the vetted zeroization required by REQ-0045 and API-0010
    And foreign-runtime copies including Swift Data are reported as residuals rather than claimed erased

  Scenario: BK05 truthful platform evidence
    Then results are claimed only for executed targets
    And iOS lock behavior, RN iOS runtime, Android and JVM remain unverified until executed

  Scenario: BK06 macOS lock primitive probe
    Given the claim file held through one open
    When a second independent open in the same process and one in a child process request O_EXLOCK and O_NONBLOCK
    Then both fail with EWOULDBLOCK while it is held and succeed after release

  Scenario: BK07 concurrent mixed-wrapper empty-directory race
    Given a pre-created empty D and two claimants requesting opposite policies, as two threads and as two processes, with a barrier after directory validation
    Then exactly one reserves and sets its flag and the other is policy-rejected with BackupPolicyFailure or Busy
    And D's flag, reservation and readback agree
    And the Swift and RN Objective-C implementations pass the same fixture table
    # Not covered: a race in which D does not exist yet. The RN host's creation of its
    # controlled child fails closed with BackupPolicyFailure for the losing creator;
    # a retry after D exists follows the normal claim rules.

  Scenario: BK08 Android concurrent opposite-parent race
    Given two concurrent opens of one new name with opposite flags through the Kotlin SDK and the RN module
    Then exactly one store exists and the other is policy-rejected with BackupPolicyFailure
    And the semaphore precedes the FileLock
    And both are released on success, rejection, native-open failure and bridge teardown
    # Specified only while JVM and Android execution is held.

  # BK-F1..BK-F6 require an injectable syscall/Foundation fault seam in both the
  # Swift SDK and the RN iOS host; without that seam they remain unverified.
  Scenario: BK-F1 flag write fails in an empty directory
    Given an empty owned directory D and an injected failure setting D's flag
    Then the open fails closed with BackupPolicyFailure, the claim stays empty and no store exists
    And a later open with either flag reserves and succeeds

  Scenario: BK-F2 flag readback differs
    Given an empty owned directory D and an injected readback mismatch
    Then the result equals BK-F1

  Scenario: BK-F3 ftruncate of the claim fails
    Given an empty owned directory D whose flag was set and an injected ftruncate failure
    Then the open fails closed with BackupPolicyFailure, the claim is unchanged and empty and no store exists
    And a later open retries and succeeds

  Scenario: BK-F4a pwrite fails without writing any byte
    Given an empty directory and an injected reservation pwrite failure with zero bytes written
    Then the open fails closed with BackupPolicyFailure and no native open is attempted
    And the claim stays empty and a later open retries as unreserved

  Scenario: BK-F4b pwrite writes a nonempty partial prefix
    Given an injected short reservation pwrite with k greater than zero bytes written
    Then this and every later open fail closed with BackupPolicyFailure and no store is created
    And the directory flag may be changed only for a directory that had no other entry
    And documented manual recovery is required

  Scenario: BK-F5 fsync fails after a complete pwrite
    Given an injected fsync failure after the complete reservation was written
    Then this open fails closed with BackupPolicyFailure and no native open is attempted
    And a later open sees a valid reservation and proceeds only when the flag readback equals it

  Scenario: BK-F6a reservation write fails without bytes in a legacy directory
    Given an unreserved legacy directory with existing entries and a matching request whose reservation write fails with zero bytes written
    Then the directory flag is unchanged and the claim stays empty
    And a later matching request retries the legacy reservation without changing the flag
    And a later differing request is rejected without changing the flag

  Scenario: BK-F6b reservation write is partial in a legacy directory
    Given an unreserved legacy directory with existing entries and a matching request whose reservation write writes a nonempty partial prefix
    Then the directory flag is unchanged and the partial claim fails closed on later opens
    And separately authorized offline manual truncation of the claim to empty permits the matching legacy path
    And no store file is moved deleted or changed by that recovery

  Scenario: BK09 reservation lifetime after native-open failure
    Given a request that passes the policy step and writes its reservation, then fails in the key-bearing native open
    Then the reservation and any flag it set on a previously empty Apple directory remain
    And a later opposite request is policy-rejected, the same request proceeds and nothing re-reserves
    Given an empty claim file and an otherwise empty directory
    Then the next open decides as unreserved

  Scenario: SW-L1 close during an asynchronous claim wait
    Given an openProtected attempt paused in claim-lock acquisition before native submission
    When close invalidates the attempt
    Then the attempt throws closed, close completes and no owner remains
    And no key-bearing native open is submitted after close
    And an already-written claim is retained without changing its policy

  Scenario: SW-L2 second open cannot initialize another directory during the claim
    Given SW-L1's first claim paused on one Database instance
    When that same instance opens a different directory
    Then it returns Busy immediately without a claim file flag write or native open for that directory

  Scenario: SW-L3 reopen after cancelled claim
    Given the close in SW-L1 has completed
    Then a fresh open succeeds normally and stale claim work cannot publish into it

  Scenario: SW-L4 cancellation is checked before policy initialization
    Given an open attempt cancelled before the claim starts or while listing before its first mutation
    Then the claim observes cancellation before flag or reservation writes and no native open is submitted

  Scenario: SW-L5 a claim failure during cancellation is a closed attempt
    Given the claim has passed its cancellation check and a flag syscall is held at a test barrier
    When close invalidates the attempt and the held syscall returns a non-cancellation failure
    Then the open reports closed, no native open is submitted and no owner remains
    # regression of the already-implemented post-GREEN error mapping; not initial RED

  Scenario: RN-L1 bridge invalidation during an RN iOS claim
    Given an RN iOS host open that is about to enter the claim, is retrying the claim lock, is listing before its first mutation, or has finished the claim before native submission
    When the bridge is invalidated
    Then the open rejects Closed and no key-bearing native open is submitted
    And no flag or reservation is written when invalidation precedes initialization
    And a reservation already written is retained
    Given a claim failure returned after the bridge was invalidated
    Then the open rejects Closed
    # The analog of SW-L1, SW-L4 and SW-L5 for the bridge-invalidation signal only.
    # Unlike SW-L1, a JavaScript Database.close does not cancel the native attempt;
    # that case is not covered here.

  Scenario: RN-L2 a contended claim does not block other module calls
    Given another process holds D's claim lock
    When an RN iOS open waits on the claim and a C ABI call is queued on the module queue after that open began
    Then the queued call reaches the C ABI and returns before the claim lock is released
    And only the read-only validate_open preflights ran off the module queue
    And after release the open reserves, and its key-bearing native open runs on the module queue
    And a second open queued meanwhile waits for the first open's whole policy phase and is not Busy
    # The harness waits for the queued call under a 500 ms deadlock watchdog,
    # which is not a latency target. The 2000 ms bound covers one active claim's
    # lock wait only; queued opens behind earlier policy phases remain the
    # DEF-0035 residual.

  Scenario: RN-L3 invalidation on both sides of the admission boundary
    Given an RN iOS open whose policy phase reserved and whose submit block is queued on the module queue
    When the bridge is invalidated before that block's admission check
    Then the open rejects Closed and no key-bearing native open is submitted
    And the reservation is retained
    Given an admitted open whose key-bearing native open is in progress
    When the bridge is invalidated before the native reply returns
    Then the native open completes, the late owner is closed exactly once and the open rejects Closed
    And no owner remains registered
    Given an open ending in preflight or policy rejection, Busy, invalidation before or after admission, a failed native open or success
    Then its owned prepared key copy is zeroized and the key never appears in an encoded frame
    # Both sides run through a React-free orchestrator seam; no lifecycle mutex is
    # added beyond the existing queues and live flag.

  Scenario: BK-F7 listing failure is not an empty directory
    Given a directory listing fails before an existing store entry is returned
    Then the policy step returns BackupPolicyFailure without a flag or reservation write or native open
    And a true end-of-listing remains distinguishable from that failure
```
