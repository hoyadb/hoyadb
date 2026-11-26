---
id: DEF-0040
title: Android SDK omits its declared per-database background error sink
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-27
updated: 2026-10-31
parents: [SPEC-0036]
affects: [API-0011, REQ-0046]
regression_for: [TC-0040, TC-0044]
tags: [purpose-audit, android, kotlin, background-errors]
---

# DEF-0040: Android omits the declared background error sink

## Symptom and checked evidence

At code base 8efb1af, Android's managed SDK has no `BackgroundError` model or
per-database `onBackgroundError` parameter or reporting helper. Searching all
Kotlin sources under `bindings/android/src` finds neither symbol. In contrast,
API-0011 explicitly declares both for Kotlin and requires redacted background
facts, once per owner epoch, with stale-epoch suppression and sink containment.
SPEC-0036 and TC-0040 OD04/OD07 describe the same reporting contract.

`bindings/android/src/main/kotlin/dev/hoyadb/Sync.kt` has only the sync-driver
`startSync(onError)` adapter. It reports sync failures and contains an exception
from that callback; it is not a per-database sink for observation, callback,
merge or timer facts. The public `Database` constructor in `Database.kt` does
not accept the declared sink. A host therefore cannot install the Kotlin
background callback promised by API-0011 through the current SDK surface.

This is a source-confirmed contract gap, discovered while preparing SPEC-0040.
It predates Stop/Discard and is tracked separately from that feature and the
broader DEF-0033 observation work. No Kotlin compilation, JVM/Android runtime
reproduction, passing regression or production-incident claim is made here.

## Confirmed cause

The declared Kotlin surface and per-database reporting path are absent. The
sync-driver-specific error callback cannot cover failures when no driver is
installed and must not relabel callback failures as sync failures. Further
runtime effects are unverified while JVM and Android execution remain held.

## Required correction and scope boundary

Implement the existing API-0011 contract without a new event framework or
runtime dependency: a Kotlin `BackgroundError`, the documented trailing
`onBackgroundError` parameter, and an owner-epoch-aware reporting helper.
Reports contain only the existing source, kind and token fields, never a
message, exception, command key, payload or row. Preserve committed mutation
results, suppress reports after close/reset, and contain sink failures.

The full defect includes the API-0011 sources observation, callback, sync,
merge and timer, including failed unsubscribe and managed drain. A later
SPEC-0040 callback-only change would address only its `onResolution` failures
and would not close this defect or establish Android reporting parity.
`startSync(onError)` remains the sync-failure adapter. No source correction is
implemented by this intake, and no user-store operation is authorized.

## Regression gap and verification hold

TC-0040 OD04/OD07 already enumerate cross-SDK redaction, containment, exactly-once
facts and stale-epoch suppression; their Kotlin cases remain specified and
unexecuted. The corresponding model and sink regression was not found under
`bindings/android/src/test/kotlin/dev/hoyadb`. TC-0044 CP13 will also need to
pin `onResolution` containment through this existing sink, with no fabricated
outcome or change to a committed resolution.

Before a fix, review the exact Kotlin cases and add meaningful regression tests.
Missing symbols or compilation failures are not behavioral RED. Source-only
test additions must be labelled not-run while the JVM/Android execution hold
remains; static review is not runtime verification. All sources and this defect
remain open until their implementation and verification are reported honestly.

## 2026-10-30 authorized remediation acceptance

The operator's fix-all request authorizes the current correction and task-owned
runtime tests; the older execution-hold narrative above remains historical.
Before implementation, the acceptance cases are:

- The trailing per-database sink receives only source, sanitized public kind and
  optional subscription token; unknown/native failures expose no exception or
  message. A throwing sink cannot replace a committed result or stop delivery.
- A terminal event reports once and retires its listener. A terminal callback
  failure also reports `CallbackFailure` once. Other callback failures retire
  once, and failed cleanup reports its own observation fact once.
- A failing automatic managed drain reports once despite coalesced wakes.
  Awaited `drainNotifications`, pull and merge failures return to the caller
  without a background report for the awaited failure.
- Automatic sync failures retain the existing `startSync(onError)` adapter and
  also notify the database sink once. Timer scheduling/cancellation failures
  use the timer source and preserve successful caller mutations.
- A fact or queued sink delivery from an old owner epoch is suppressed after
  close/reset; a new owner may report its own independent failure. The public
  model includes all five API-0011 sources. Current Kotlin has only awaited
  `pullOnce`; adding an automatic pull API is outside this correction.

Tests will use the current Kotlin sources and controlled native replies on the
task-owned Android runtime. Controlled replies exercise host reporting and
ordering, not the native database implementation. Any nonfunctional API stubs
needed to compile the new test must be identified separately from behavioral
RED. Actual commands, failures and remaining limits will be recorded here.

## 2026-10-31 correction and runtime evidence

`Background.kt` supplies the five-source model, fixed public-kind sanitization
and a per-database reporting helper. It dispatches through `eventExecutor`,
checks the owner epoch both before scheduling and immediately before invocation,
and contains any `Throwable` without holding database or observation locks across
foreign code. The constructor accepts the declared trailing optional sink.

The managed observation dispatcher handles generic terminal events before typed
query/queue decoding, reports once, and retires without another core call.
Callback, cleanup and automatic-drain failures produce their respective facts;
explicit drain failures still return to the caller. A failed drain clears its
coalesced wake flag rather than repeatedly retrying the same failed attempt;
a genuinely new wake remains eligible. The last count listener detaches its
timer and invalidates its callback token before cancellation outside the lock.
Timer failures preserve healthy listeners and already-committed caller results.
Sync reports retain the existing `onError` adapter and contain `Throwable` from
either sink. An inline database sink may close/reset the owner, so liveness is
checked again before the adapter starts. Cleanup reports follow throwing-callback
retirement; manual subscription cancellation preserves its prior silent cleanup
failure behavior. Original queue observer signatures remain; explicit two-callback
overloads add `onTerminated` without stealing existing trailing lambdas.

The reviewed `tools/build.py --tests --split-runtime` compiled current SDK and
all existing/new Kotlin test sources. Behavioral RED on the task-owned Android
runtime preceded implementation: the initial packet failed 8 of 10 groups for
missing facts, the expanded packet failed 10 of 14, and the isolated observation
timer case failed. A strengthened sink-containment case terminated its test
process; Android's fatal-exception log identifies the synthetic `AssertionError`
escaping `SyncDriver.error`, which caught only `Exception`. Missing symbols and
compilation failures were not counted as RED: inert model/constructor/helper
and terminal-overload declarations enabled the tests before any reporting existed.

The initial 16-group GREEN was followed by two cross-review regressions. An
inline sink closing/resetting its owner exposed a stale subsequent `onError`
call; the isolated runtime test failed before the extra liveness check and
passed afterward. A manual-close control then failed because failed unsubscribe
was reported outside the contract's throwing-callback case; restricting that
report to callback retirement restored the original behavior.

After those corrections, all 18 groups in `BackgroundErrorTests` passed on the
task-owned runtime using `tools/run-tests.py`. They cover terminal ordering, all queue
listener decoders, recoverable errors, callback/cleanup reporting, coalesced drain
failure and recovery, explicit drain callback failures, timer set/clear failure,
commit preservation, both throwing sinks, close/reset epoch suppression, awaited
pull/merge non-reporting, and sanitization, plus both cross-review controls. The
controlled native replies isolate host behavior. Status remains draft.

Exact final commands (repository root):

```sh
python3 bindings/android/tools/build.py --out /private/tmp/hoyadb-fixes-20261030/kotlin-admission-reviewed --tests --split-runtime
python3 bindings/android/tools/run-tests.py --out /private/tmp/hoyadb-fixes-20261030/kotlin-admission-reviewed --class-name BackgroundErrorTests --log /private/tmp/hoyadb-fixes-20261030/kotlin-final-BackgroundErrorTests.log
```

The final build manifest binds all 50 source/tool inputs unchanged before/after the
build and checked against the working tree. Manifest SHA256:
`d969dfbeb11bd83f04b98fc7b93f786ef024c9b890bf2c90cacd8d42086ab94c`;
test DEX: `68bf5d1f6fab115a9531e4305a6db5b01306fd3fd559be12ab3fb4fe6233accf`.
The adjacent `kotlin-final-BackgroundErrorTests.json` binds those exact artifacts to the
passing log. Full RED/GREEN records, commands and limitations are retained in
`/tmp/hoyadb-fixes-20261030/kotlin-fixes.md` and its referenced logs.

Nearby verification rebuilt the pinned provider, Rust native archive and JNI
from unchanged current source, then ran 21 groups across 11 existing Kotlin
suites. Twenty passed; `SyncDriverTests` K10 failed its pre-cancel assertion
because the mapper had already run while the separate FIFO blocker was held.
The current JNI local and device SHA256 matched
`a030509cbf68e64e37214e2b5dd63b8386f617cbeffd7f0c6196d5e2d1d66450`.
`android-native-current.json` records the source/provider binding;
`kotlin-nearby-results.json` records each exact runtime command, result and log.
The copied diagnostic test preserved assertions/timeouts and added only stack
locations and latch/count prints. [DEF-0055](DEF-0055-kotlin-native-admission-regression.md)
confirmed that this test retained the superseded app-wide native FIFO assumption.
After correcting the test to cover both same-database Busy and independent
progress, all three sync groups and all 18 background groups passed against the
final artifact above. The other 19 nearby groups passed the preceding packet;
runtime logic did not change between those packets. Exact results remain bound
to their individual artifacts rather than relabelled as a single new full run.

This correction does not recertify the final AAR or rerun the outside-checkout
23-group ordinary consumer. Earlier packaging results remain historical.

The `Merge` model/helper source is exercised directly, but no automatic Kotlin
pull/merge path exists in `Entities.kt`: `pullOnce` is awaited and its errors
correctly return without sink emission. No automatic merge runtime coverage is
claimed. The later Stop/Discard `onResolution` surface is also absent; TC-0044
CP13 remains future integration work, not a case satisfied by this correction.
The pre-existing retirement of count listeners after `timer.schedule` fails now
emits a timer fact but still has no `Terminated` callback; that callback behavior
is unspecified by the current API and was a nonblocking review observation.


## Stop/Discard integration follow-up — 2026-10-31

The later SPEC-0047/TC-0051 work now implements SPEC-0040's Kotlin policy and
resolution surface, including TC-0044 CP13. The prior absence statement remains
historical. `onResolution` runs only for current-owner KeptCurrent reports;
a thrown `Throwable` leaves the callback installed and committed result intact,
emits one redacted CallbackFailure with null token, and does not invoke the sync
error adapter. Close/reset suppress queued callbacks and their background facts.

Seventeen controlled Kotlin runtime cases passed after a meaningful 12-failure
behavioral RED; four actual JNI policy cases passed. The full packet passed 103
unique cases across 24 entrypoints, including the unchanged 18 background cases.
The fresh final AAR's outside-checkout consumer passed 25 groups, including
memory/protected CP13 and throwing resolution/background callbacks. See the
[policy follow-up record](../../implementation/phase-seven-kotlin-policy-followup.md)
for exact commands, source/artifact bindings and limits. The automatic merge
source remains dormant because Kotlin still has only awaited pull operations;
no automatic pull driver or unspecified timer-termination behavior was added.
