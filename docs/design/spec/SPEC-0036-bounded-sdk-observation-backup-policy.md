---
id: SPEC-0036
title: Bounded SDK observation delivery, typed background failures and fixed native backup policy
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-26
parents: [SPEC-0033]
satisfies: [REQ-0007, REQ-0045, REQ-0046]
depends_on: [API-0011, API-0010, DEF-0033, DEF-0037]
tags: [purpose-audit, remediation, bindings, observations, backup]
---

# SPEC-0036: Bounded SDK observation delivery and fixed native backup policy

Proposed behavior. Nothing here is implemented yet. TC-0040 holds the
acceptance cases.

## Need and verified root cause

**DEF-0033.** The facade's shared prefix helper rejects an event that cannot
be the only item of a drain frame. It neither removes nor transforms that
event. The owner's captured event queue is refilled from core only when it is
empty. One oversized membership snapshot therefore blocks every later
observation, status and outcome event of that owner. The SDKs then report the
failure differently:

- JavaScript appends it to an undeclared array;
- Kotlin suppresses it;
- Swift sends it to every listener.

Core data notifications can already carry recoverable storage errors. Those
must keep their listener.

**DEF-0037.** Protected native backup policy is requested per open but applied
per directory.

- Apple writes the exclusion flag on a directory that several stores can
  share, so the last opener wins.
- Android selects `noBackupFilesDir` or `filesDir` from the flag, so a changed
  flag silently selects a different, possibly empty store.
- The Swift SDK, the RN iOS host, the Kotlin SDK and the RN Android host each
  decide independently. Nothing orders two wrappers or processes that open in
  the same location.

## Scope

Kept unchanged:

- the API-0011 ABI1 wire limits (1,048,576 UTF-8 bytes, depth 64, 100000
  nodes), the ABI version, core command admission and core reactive semantics;
- the existing path, key, redaction and readback controls.

Not part of this change:

- any limit increase, streaming or paging feature, new dependency, or
  transport or network behavior;
- any store migration, move, deletion or reset.

## Behavior

1. **Terminal oversized observation (facade).**
   - Drain pages are built in event order. An event is tested for termination
     only when it would be the first item of a page.
   - Let S be its subscription. For this size calculation only, disregard S's
     later buffered events without removing them yet. `has_more`
     is true exactly when at least one other buffered event remains. Each such
     event later yields either its original or a terminal event.
   - If the frame containing the event as its only item, with that `has_more`,
     exceeds a wire limit, the subscription terminates:
     - remove that event and every later buffered event of S;
     - end S's core subscription and remove its handle;
     - place `{kind:"error",subscription:S,value:{kind:"LimitExceeded"}}` in
       that position;
     - the page then continues.
   - A non-first event that does not fit ends the page normally, with
     `has_more:true`.
   - Other events keep their relative order. Each original event is delivered
     at most once.
   - A terminated handle produces no further event, and unsubscribing it
     returns false.
   - The data error variant
     `{kind:"data",subscription,value:{kind:"error",error:{kind}}}` remains
     the recoverable core notification. It never terminates.
   - Fetch keeps returning LimitExceeded without truncation.
2. **SDK terminal handling.**
   - Every managed decoder (JavaScript, Swift, Kotlin) handles the generic
     error event before kind-specific decoding.
   - The decoder delivers it once to its own listener in the typed terminal
     form below, then retires that listener without another core call.
   - Recoverable data errors are delivered as today and keep the listener.
   - Ordered delivery requires a serial FIFO event executor. Swift and Kotlin
     deliver through the application-selected executor; Swift's default
     `DispatchQueue.main` is serial. A concurrent executor can reorder
     callbacks, run the terminal delivery first and thereby suppress that
     listener's earlier pending deliveries. No ordering guarantee is claimed
     for a concurrent executor.
   - JavaScript explicit `drain()` and Swift/Kotlin `drainNotifications()` use
     the same delivery rules.
   - Raw `execute("drain")` remains a raw pass-through when allowed by the
     existing managed-stream exclusion; it still returns Busy while managed
     listeners or delivery own that stream.
3. **Typed background sink.**
   - Each SDK accepts an optional per-database `onBackgroundError`.
   - It receives only `{source, kind, token}`. It never receives an exception,
     message, command key, payload, row, domain value or key bytes.
   - Each fact is reported exactly once per owner epoch:

     | Fact | Trigger | Report |
     | --- | --- | --- |
     | T | terminal event received | `observation`, event kind, token |
     | C | application callback throws, or its returned thenable rejects | `callback`, `CallbackFailure`, token; at most once per listener, which then retires |
     | U | the core unsubscribe issued by a C retirement fails | `observation`, kind, token |
     | D | one managed drain request fails | `observation`, kind, null; coalesced callers share one report |
     | S, M, X | an automatic sync, merge/pull or host timer attempt fails | that source, kind, null |

   - A terminal callback that throws or itself returns a rejecting thenable
     yields T, then C. A rejection from an earlier invocation arriving after
     that listener's terminal retirement is suppressed, as are failures from
     any other already retired invocation.
   - Facts produced after close or reset of their owner epoch are suppressed.
     This includes late rejections of Promises returned earlier.
   - Failures inside an awaited caller API, including an explicit drain
     request, are returned to the caller and not reported. Callback failures
     during an explicit drain are C facts.
   - Sink failures, including a rejected returned thenable, are contained
     without recursive reporting or an unhandled rejection. Every reported kind
     is a fixed public kind; callback facts use CallbackFailure. Unknown kinds,
     including malformed wire terminal kinds, become BackendFailure.
   - A failure of a host timer set or clear is a timer fact. It cannot mask a
     committed result, interrupt delivery of other buffered events, retain a
     scheduling guard, or produce an unhandled rejection. Failed automatic
     timer actions are not silently reported as observation failures.
   - The existing Swift and Kotlin `startSync` onError adapters still receive
     S failures.
4. **JavaScript callbacks.**
   - Managed and explicit delivery invoke callbacks synchronously, in event
     order, and do not await returned values.
   - Callbacks are not serialized with one another's asynchronous
     continuations.
   - A pending Promise never delays later callbacks, drains or owners.
   - Unsubscribe, close and reset suppress later buffered callbacks for their
     token or owner.
5. **Apple backup policy claim.** The Swift SDK `openProtected` and the RN iOS
   host implement the same claim and error algorithm: steps 1-7 below, the
   item 7 outcomes and the Busy and BackupPolicyFailure results. Their open
   lifecycles differ, as stated next. Let D be the validated canonical
   non-symlink directory.
   - Swift SDK lifecycle: reserve the wrapper's Opening lifecycle before the
     first asynchronous claim wait. A concurrent second open on that wrapper
     returns Busy before touching its directory. Close invalidates and cancels
     a pending claim; cancellation is checked at claim entry, before
     initialization mutations, during retry and between the claim and native
     submission. No key-bearing open starts after that close. Any reservation
     already written is retained; a later fresh open follows the normal claim
     rules. Existing disposal of an already-submitted native open is preserved.
   - RN iOS host lifecycle: an open has two phases.
     - Policy phase, on a per-module serial policy queue separate from the
       module queue: copy the key into one owned prepared secret, run both
       read-only `hoya_v1_validate_open` preflights, resolve the parent,
       create and validate the child, then claim. Those preflights use a fresh
       facade and touch no shared runtime, owner or registry.
     - Submit phase, on the module serial queue: the final invalidation check
       is the admission boundary; an admitted open performs the single
       key-bearing C ABI open, then updates the owner registry.
     - Apart from those read-only preflights, every C ABI call, including the
       key-bearing open, and the owner registry stay on the module queue. A
       contended claim therefore does not delay other module calls. Opens
       serialize on the policy queue, so a second open waits rather than
       returning Busy. Module calls queued after an open began may run before
       its native submission.
     - Bridge invalidation is the only native cancellation signal. It is
       checked at the four claim points and at the admission boundary. Before
       admission it rejects Closed: no key-bearing open is submitted, and a
       written reservation is retained. After admission the key-bearing open
       may complete; the existing post-reply check then closes the late owner
       and rejects Closed, so no owner is leaked. A JavaScript
       `Database.close` does not cancel a pending native open; disposing a
       late owner is left to the existing JavaScript lifecycle.
     - The prepared secret is zeroized on every path that ends the attempt:
       preflight or policy rejection, Busy, invalidation before or after
       admission, a failed native open and success. The key is never encoded
       into a frame, string or log.
       Before handoff the policy phase owns and clears it on failure. Once
       the submit block is enqueued, only that block owns it and clears it in
       `@finally` on every exit; the policy phase never touches it again.
   - Residual (DEF-0035, not waived): the 2000 ms bound covers only one
     active claim's lock-acquisition retries. An open queued on the policy
     queue also waits for every earlier open's whole policy phase, whose
     filesystem I/O and Foundation calls are unbounded.
   1. Open D with `O_DIRECTORY|O_NOFOLLOW|O_CLOEXEC` and require that `fstat`
      device/inode equal the validated `lstat`.
   2. Acquire the owned claim file `.hoyadb-backup-policy` with
      `openat(O_RDWR|O_CREAT|O_NOFOLLOW|O_CLOEXEC|O_EXLOCK|O_NONBLOCK, 0600)`.
      Retry on `EWOULDBLOCK` for at most 2000 ms, then reject Busy. This bound
      covers only claim-lock acquisition retries; it does not bound filesystem
      I/O, Foundation flag calls or the native open.
   3. Reject BackupPolicyFailure unless all of these hold:
      - the claim is a regular file with a single link, owned by the
        effective uid, and at most 16 bytes long;
      - `fstatat(dfd, name, AT_SYMLINK_NOFOLLOW)` device/inode equal the
        `fstat` of the opened descriptor.
   4. Under the lock, read the claim:
      - `excluded\n` or `included\n` is an immutable reservation. A different
        request rejects. A directory flag that differs from the reservation
        rejects.
      - Empty content means unreserved.
      - Any other content fails closed.
   5. Unreserved: list entries through `fdopendir` on a `dup` of the
      directory descriptor, ignoring the claim file.
      Distinguish end of listing from a `readdir` error; a listing error fails
      closed before any flag or reservation write.
      - With no other entry, set D's flag, read it back, then write the
        reservation (`ftruncate` 0, `pwrite`, `fsync`).
      - With other entries (a legacy directory), write the reservation only
        when D's existing flag equals the request. Otherwise reject.
   6. Immediately before each Foundation flag read or write, require that
      `lstat(D)` device/inode equal `fstat` of the directory descriptor.
   7. Release the claim, then perform the key-bearing open.
   - hoyadb never unlinks, renames or re-truncates a claim file beyond that
     first reservation write.
6. **Android backup policy.** The Kotlin SDK `openProtected` and the RN
   Android host implement this identically.
   - Acquire `BackupPolicyLock`: first a process-wide semaphore, then a
     no-follow `FileChannel` lock on
     `noBackupFilesDir/hoyadb/.hoyadb-backup-policy.lock`. The lock file must
     be a regular file.
   - Hold both from the opposite-parent check until the key-bearing open
     replies.
   - If any store family file (main, `-wal`, `-shm`, `-journal`) exists under
     the opposite policy parent's `hoyadb` child, reject BackupPolicyFailure.
     Neither copy is created, moved, deleted or opened.
   - Release the FileLock, then the semaphore, in `finally` and on every reply,
     rejection and RN bridge teardown path.
   - The lock file is never deleted.
   - No acquisition timeout is specified for the Android semaphore or
     FileChannel lock; the 2000 ms Apple retry bound does not apply here.
7. **Side effects.**
   - **Class A, policy-rejected** (conflict, preflight checks, or Busy from
     claim or lock acquisition), raised before any flag or claim write:
     - writes no store file;
     - never changes a directory backup flag;
     - may create only the empty owned bookkeeping file — the Apple claim, or
       the Android lock file together with its required
       `noBackupFilesDir/hoyadb` child — which is never deleted.
   - **Class B, Apple initialization failure** (step 5 has started: setting
     or reading back the flag, `ftruncate`, `pwrite` or `fsync` fails or
     mismatches):
     - fails closed with BackupPolicyFailure; no native open is attempted;
     - may leave D's flag changed, but only when D had no other entry; a
       legacy directory's flag is never changed;
     - may leave the claim empty, a partial prefix, or the complete
       reservation without a confirmed `fsync`;
     - never creates a store file and never unlinks, renames or deletes;
     - provides no rollback guarantee.
   - The next open is deterministic under the step 4 and 5 rules:
     - an empty claim with no other entry is unreserved, and the open retries
       initialization with whichever flag it requests (no store exists);
     - an empty claim beside existing entries is also unreserved and takes
       step 5's legacy path: reserve only if the existing flag matches the
       request, without changing that flag;
     - a partial or malformed claim fails closed on every open; recovery is
       manual only — during separately authorized offline maintenance an
       operator may truncate the claim to empty, without deleting, moving or
       changing any store file or directory flag. The empty claim then follows
       the appropriate empty-directory or legacy rule; hoyadb never repairs
       the claim automatically;
     - a complete reservation is reserved, and step 4 rejects a differing
       flag; a complete reservation lost in a crash becomes empty or partial,
       and the preceding rules apply.
   - Android has no class B writes: it writes only the lock file and its
     `hoyadb` child, so every Android policy failure is class A.
   - An open that passes the policy step and then fails in the key-bearing
     native open keeps its written reservation. For a previously empty Apple
     directory it also keeps the directory flag it set. That reservation is
     immutable: there is no automatic re-reservation, and recovery is manual.
     Native-open failure side effects are otherwise unchanged from current
     behavior.
   - An empty claim is unreserved after a crash or zero-byte initialization
     failure before the reservation write, a policy-rejected legacy open, or
     the explicitly authorized manual recovery above.
   - Existing canonical, symlink, single-component name, key preflight, key
     zeroization and readback controls are unchanged.
   - Residual: the key-bearing SQLite open remains path-based after the claim
     or lock is released, as it is today.
8. **Query bounds.**
   - Results larger than one frame use an explicit limit and keyset predicates
     on a unique ordered field such as `id`.
   - There is no implicit paging, truncation or large-dataset transfer
     guarantee.

## Exact public types

Additive, except where noted as source-breaking:

- **JavaScript:**
  - `type BackgroundSource='observation'|'callback'|'sync'|'merge'|'timer'`;
  - `interface BackgroundError {readonly source:BackgroundSource; readonly kind:string; readonly token:Handle|null}`;
  - `HostOptions.onBackgroundError?:(error:BackgroundError)=>void`, which the
    Database option allowlist and the RN factories forward;
  - the `Event` union gains `{kind:'error';subscription:Handle;value:{kind:string}}`;
  - `EventCallback` keeps its type, but its returned value is no longer
    awaited.
- **Swift:**
  - `HoyaError.subscriptionTerminated(String)`. Source-breaking for exhaustive
    `switch` statements without `default`;
  - `public struct BackgroundError: Equatable, Sendable {public enum Source: String, Sendable {case observation, callback, sync, merge, timer}; public let source: Source; public let kind: String; public let token: String?}`;
  - `Database.init` gains a trailing
    `onBackgroundError: (@Sendable (BackgroundError) -> Void)? = nil`.
- **Kotlin:**
  - `ObservedValue.Terminated(val kind: String)`. Source-breaking for
    exhaustive `when` expressions without `else`;
  - `class BackgroundError(val source: Source, val kind: String, val token: String?)`,
    with `enum class Source {Observation, Callback, Sync, Merge, Timer}`;
  - the public `Database` constructor gains a trailing
    `onBackgroundError: ((BackgroundError) -> Unit)? = null`;
  - `observeStatus`, `observeOutcome` and `observeProgress` gain a trailing
    `onTerminated: (String) -> Unit = {}`;
  - `public object BackupPolicyLock { @JvmStatic fun acquire(context: Context): Held }`,
    where `Held : AutoCloseable` releases in reverse order and is idempotent.
    It is documented for hoyadb platform wrappers only.

## Integrity and compatibility

- The wire changes by one additive Event member under ABI "1".
- The facade no longer fails a drain because one event is oversized. This
  supersedes the API-0011 and TC-0036 G11 "pending until cancellation" rule
  and the facade test that pinned it. The cancellation assertion is kept.
- The JavaScript changes are:
  - callbacks are not awaited;
  - throwing callbacks retire.
- Swift no longer broadcasts owner-wide drain failures to every listener.
- The undeclared JavaScript `syncErrors` and `callbackErrors` arrays stay, for
  existing tests, bounded to the most recent 64 entries.
- The only new on-disk artifacts are:
  - the owned Apple claim file in a store directory;
  - the Android lock file, together with its `noBackupFilesDir/hoyadb` child.
  Older code ignores both.
- No store format, path or layout changes. No user store is migrated,
  deleted, reset or opened by this change.

## Acceptance and TDD

- TC-0040 supplies OD01–OD08 and BK01–BK09. TC-0036 G05, G11 and G16 are
  amended.
- Each change starts with a source-bound behavioral RED on unchanged code, not
  a missing symbol. Then comes the smallest GREEN, then the existing
  surrounding controls.
- Source, toolchain and fixture hashes are recorded before and after.
- Root's GO message for this fix request explicitly authorizes task-owned,
  pinned, offline Rust, JavaScript, Swift and WASM test and rebuild fixtures,
  with no download and no new dependency. It is not a JDK/Gradle/Android hold
  lift.
- A native framework rebuilt for testing goes only into task-owned output,
  outside `bindings/swift/Artifacts`. Reviewed artifacts are not overwritten.
- No packaged claim is made until the DEF-0030 `stale-provider-evidence`
  finalizer result is resolved honestly.
- Kotlin, RN Android, iOS lock behavior and RN iOS runtime remain unverified
  until actually executed.

### React Native rejection translation (BK10)

The native module rejects host operations with a static React Native `code`,
whereas the shared JavaScript runtime consumes a redacted `kind`. The transport
adapter translates rejected open, call and namespace operations into fresh errors
containing only `kind` and its identical static message. For open, preserve only
`InvalidRequest`, `BackupPolicyFailure`, `Busy` and `Closed`; for call, only
`InvalidRequest` and `Closed`; for namespace, only `Closed`. `BoundaryFailure`
explicitly maps to `BackendFailure`, as do unknown, malformed, unreadable or
operation-inappropriate codes. In particular, a host call rejection of `Busy`
must not be confused with core admission Busy and wait forever for a release.
It never copies a native
message, stack, path, key, payload or cause. The shared JavaScript public-kind
allowlist admits `BackupPolicyFailure`, already required by this spec. Successful
wire frames stay opaque and unchanged. This is a transport-only correction;
scripted TurboModule tests do not establish native runtime behavior.
Compatibility change: a native `Closed` rejection previously surfaced as
`BackendFailure`; callers and background facts now see `Closed` even while the
JavaScript owner handle has not yet learned that its native module was invalidated.

## Quality attributes and rollback

- **Functional Suitability:** healthy subscriptions continue past an oversized
  one; a changed or conflicting backup flag cannot silently select another
  store or change another store's policy.
- **Performance Efficiency:** one bounded pass per drain page; no extra store
  scan; Apple claim-lock acquisition retries are bounded at 2000 ms, while
  filesystem I/O and the Android lock are not time-bounded; no numeric
  latency promise.
- **Compatibility:** ABI "1" with one additive event; disclosed
  source-breaking enum and sealed-class additions; no store format or path
  change.
- **Interaction Capability:** typed terminal forms and one redacted sink
  replace silent or broadcast failures.
- **Reliability:**
  - termination is per subscription and exactly-once;
  - each reservation is durable before unlock;
  - locks are released on every path;
  - recoverable data errors keep subscriptions.
- **Security:** reports contain kinds and tokens only; no-follow, identity,
  owner and size checks protect claim and lock files; the key preflight order
  is unchanged; the residual path-based open is disclosed.
- **Maintainability:** one shared Apple algorithm with one fixture table; one
  fact table for all SDKs.
- **Flexibility:** an optional sink; existing `startSync` onError adapters are
  retained.
- **Safety:** no physical-control behavior exists. No user store is moved,
  deleted or reset.

Rollback reverts task-owned source and documentation together and keeps every
RED/GREEN and diagnostic artifact. Rolled-back code ignores claim and lock
files and reopens every store at its unchanged path. This spec alters no
security approval or execution hold.
