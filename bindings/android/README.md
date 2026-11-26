# HoyaDB Android Kotlin package

For AI-assisted integration, the [tool, skill and reference catalog](../../AI.md)
describes the expected remote-dependency workflow. Use the resolved SDK version's
public Kotlin API and release guidance, checking its AAR/ABIs. A JS method does
not establish a Kotlin convenience API. This source link does not add a file to
the AAR or establish a public Maven release.

Local, unpublished Phase7 SDK under verification. The general Rust facade owns
schema validation, queries, migrations, transactions, command admission,
reconciliation and storage. Kotlin translates owned typed values and manages
awaiting, callbacks and lifecycle. See [PLAN.md](PLAN.md),
[API-0011](../../docs/design/contracts/API-0011-general-platform-sdk.md) and
[consumer/README.md](consumer/README.md) for the acceptance and packaging gates.
Earlier `--development` AARs are not final; the final local certificate and its
evidence are recorded in
[phase-seven-kotlin-final.md](../../docs/implementation/phase-seven-kotlin-final.md).
Third-party notices: [NOTICE.md](NOTICE.md) and [third-party/](third-party/).

The package has no mandatory kotlinx-coroutines, networking or serialization
library. Its runtime graph is the approved Kotlin stdlib2.1.20 and annotations13.
The installed compiler2.2.20 is used with language/API2.1 and naturally emitted
metadata2.1. Building a resource-free AAR manually does not claim an AGP build;
the RN Android consumer separately checks actual Gradle consumption.

## Opening and closing

All database calls are `suspend`. Supply the executor on which application event
callbacks should run. For example, Android's main-thread executor can post to a
Handler. Protected opens accept an externally supplied32-byte key; the library
does not derive one or store it in ordinary JSON. Validate that key before calling
an OS path getter. The default selects a controlled child of noBackupFilesDir;
explicit backup opt-in selects filesDir. The four-field protection capability
reports host_managed backup policy, rather than claiming Android backup-service
execution. The policy is fixed by location (SPEC-0036): `openProtected` holds
`BackupPolicyLock` (a process-wide semaphore, then a no-follow FileChannel lock on
`noBackupFilesDir/hoyadb/.hoyadb-backup-policy.lock`) from the opposite-parent
check until the key-bearing open replies. If the store or one of its `-wal`,
`-shm` or `-journal` files exists under the other parent's `hoyadb` child, the
open rejects `BackupPolicyFailure`; a changed flag never selects another, empty
store. A rejected open creates only that never-deleted lock file and its
`noBackupFilesDir/hoyadb` child. `BackupPolicyLock` is for hoyadb platform
wrappers only. This guard has been compiled but not yet executed on a device.

`openMemory(schema, namespace, store)` uses the same Rust memory engine. Supply
the optional deterministic32-hex namespace in hermetic tests to bypass platform
entropy. Normal opens use SecureRandom; memory opens do not evaluate Context
paths or key providers. A separately created MemoryStore retains its real closed
state across authored schema upgrades, and refuses active duplicate open/drop.

Call `close()` deterministically. It invalidates the owner generation, stops
managed work and disposes owned native state. `useDatabase(database) { ... }`
is the scoped close helper (LifetimeTests cover success and error paths).
Garbage-collection timing is not a close API.

## Typed data and operations

Schema/Table/Column/Relation describe caller-owned schemas. All seven authored
migration steps map to the Rust representation. Query covers predicates, ordering,
limit and relation navigation. Insert/replace/remove/batch/link/unlink return
owned Row/Change values; no host predicate, schema inference or merge engine runs.
The acceptance consumers use two unrelated literal caller schemas.

Cell preserves Null, String, Boolean, Integer, finite Float, Timestamp and
Reference kinds. Long carries signed64-bit integer/time values; UnsignedDecimal
preserves full-u64 handles/counts as checked decimal strings. JSONValue preserves
business integer versus Float kinds, nested ownership and i64 extrema. Its
decodeJSON/encodeJSON helpers delegate opaque JSON parsing to Rust. Platform
org.json is used only for structural typed frames and trusted ABI replies.

Arguments and results are immutable snapshots. UTF16/UTF8 errors reject instead
of replacing lone surrogates. The shared1MiB wire budget also bounds readonly
fetches; large readonly results reject with LimitExceeded rather than silently
truncate. Mutator preflight and owned event/push prefix paging use the core
facade's existing rules. Advanced `execute(op, ProtocolValue arguments)` retains
the same owner, reset and managed-drain lifecycle checks.

## Commands, observations and sync

startCommand returns an explicit CommandTask with cancel/await; command is the
suspending convenience. A CommandScope exposes only scoped operations and captured
arguments/context. It holds no native borrow across an awaited application handler.
Rust remains the sole admission authority. Proven exclusive native stores use
per-database FIFO; independent databases may progress while another handler is
suspended. Unproven stores and web retain conservative global admission
([ADR-0037](../../docs/design/decisions/ADR-0037-per-database-command-serialization.md)). Explicit cancel/close
retires a noncooperative suspended handler, while a submitted finish retains its
actual durable result. Plain stdlib continuations are handed to the managed
executor, keeping the serialized native byte-reply channel available.

observe/observeStatus/observeOutcome/observeProgress return explicit cancellable
subscriptions. Initial delivery occurs without a write or manual drain, on the
selected executor. Cancel inside a callback suppresses later buffered callbacks;
raw drain is Busy during managed registration/delivery. While a registration
is pending no managed drain starts, an explicit `drainNotifications()` is Busy
and an in-flight drain's events are buffered until the new listener is installed.
A single injected or platform timer drives the unchanged16ms count deadline; if
that deadline drain comes before the core's window has ended and a count listener
still has no value for a later write, one follow-up timer is armed, and a window
without any drain ends the sequence.

The trailing `Database(..., onBackgroundError = { fact -> ... })` callback receives
only `BackgroundError.source`, a sanitized public `kind`, and an optional
subscription `token`. Its five-source enum is `BackgroundError.Source`; unknown
or native failures become `BackendFailure`, while malformed reply failures become
`InvalidPayload`. Delivery uses `eventExecutor`, rechecks the owner epoch before
invoking the sink, and contains sink failures without replacing committed results.
Close/reset suppress queued old-epoch facts. Automatic drain, sync and timer
failures report once per failed attempt; awaited API failures return to the caller.
`startSync(onError)` remains available alongside the database sink.

Generic terminal observations deliver `ObservedValue.Terminated(kind)` once and
retire without another core unsubscribe. Queue status, outcome and progress
observers have explicit `(callback, onTerminated)` overloads; their original
trailing-lambda forms remain supported. Throwing callbacks retire once and report
`CallbackFailure`; a failed cleanup produces its separate observation fact.
The current `pullOnce` API is awaited, so pull/merge failures return without a
background report. No automatic Kotlin pull driver is implemented.

`ConflictPolicy` selects `ConflictReaction.Stop` (the default) or `Discard`,
with exact versioned command-name overrides. Pass `conflict` to the longer
`Database` constructor overload to install it before each owner is published;
`setConflictPolicy` changes only the active owner. A failed configuration disposes
the unpublished owner. If disposal also fails, `close()` can retry it and a new
open remains Busy. Kotlin returns the original configuration error if that
cleanup also fails; the current JavaScript and Swift surfaces return the cleanup
error in that double-failure case. The original constructor and its trailing background-sink
lambda remain supported.

`reconcileResolving`, `rollbackResolving` and `discard` return an owned
`Resolution.Completed` or `Resolution.KeptCurrent`. Existing unit-returning
reconcile/rollback calls and managed sync also use the core reporting operations.
Only KeptCurrent invokes `onResolution(key, resolution)`, once on the selected
executor while its owner epoch is current. A throwing callback remains installed
and reports `CallbackFailure` with no subscription token. Keep callbacks short:
an executor that runs inline invokes this callback before managed mutation wakes.
Its error cannot replace
a committed result or reach the sync driver's `onError`. Close/reset suppress
queued callbacks and facts from the retired epoch. Core owns eligibility, guarded
validation, current-value preservation, report bounds and truthful queue state;
Kotlin adds `QueueState.Discarded` without inventing a remote outcome. See
[SPEC-0040](../../docs/design/spec/SPEC-0040-simple-command-conflict-policy.md)
and [TC-0044](../../docs/design/tests/cases/TC-0044-simple-command-conflict-policy.md).

Entity configuration samples its context provider before direct entity mutations.
Typed patch/pull/reset and passive per-table merge hooks delegate all validation,
comparison clocks and atomic application to Rust. The host hook sees an owned
proposal and returns a Row or null; it cannot mutate core state while awaited.

startSync owns injected transport/clock/jitter/timer seams and no networking
library. It retains exact responses and reconciliation plans across Busy, keeps
durable retry attempts/deadlines in Rust and ignores a retired generation's late
work. The packaged testing.FakeTransport uses the same lossless Rust JSON codec
and attaches command/entity correlations. Both were implemented after genuine
unsupported-stub REDs (SyncDriverTests, TestingHelpersTests); their packaged
outside-checkout APK cases (K09, K10_K22, K15) passed in the final consumer run.

Kotlin and RN share HoyaNativeTransport and one native library. Its asynchronous
owned-byte open/call/validateOpen channel also exposes a payload-free release
subscription. Raw consumers sharing managed owners must preserve that broker's
terminal-attempt lifecycle. It grants no admission and is not a host queue.

## Verification and artifact status

Commands and exact tool/runtime hashes live in tools/, toolchain-review.json and
task evidence. Test-only split DEX is an iterative optimization; production AAR
and independent APK always use the ordinary complete program-class path.

`tools/package-aar.py` (without `--development`) issues the final local
certificate only after the PACKAGING_CASES P01–P08 checks: native SO/evidence
binding, independent llvm-readelf ELF checks, production-only classes from the
complete current source closure, exact notice closure and a fresh output with no
partial certificate. The native evidence must also record the Rust source closure
(`rust_sources_sha256`, written by `build-jni.py`) and it must equal the current
manifests, lockfiles, toolchain pin, crate sources and headers, so a library
linked before a Rust change is refused; evidence written by an older
`build-jni.py` needs a relink. `third-party/index.json` must name the current
lock files (`Cargo.lock`, `crates/hoyadb-native/Cargo.lock`) with their current
SHA256 in `lock_sha256`. It produces the local Maven coordinate
dev/hoyadb/hoyadb-android/0.1.0 (AAR, POM, sources, notices, SHA256 sidecars).

The default remains `x86_64 arm64-v8a`. Consumers that also declare 32-bit
Android ABIs can request all four slices explicitly:

```sh
python3 bindings/android/tools/package-aar.py --classes <production-build>/classes.jar --out <fresh-maven-repo> --abis x86_64 arm64-v8a armeabi-v7a x86
```

All selected JNI slices must already have current native evidence. The packager
checks ELF class and machine per ABI while retaining the export, dependency and
16KiB alignment guards. It rejects duplicate or unknown selections. Additional
32-bit certificates remain build-only until independent runtime checks pass;
they do not inherit the historical 64-bit consumer results below. See the
[native recipe](../../crates/hoyadb-native/README.md) for target mappings.

The [2026-11-08 local checkpoint](../../docs/implementation/approved-engineering-evidence/android32-verification-20261108.json)
contains the fresh four-ABI certificate and separate narrow runtime evidence.
Both additional slices passed protected writes, alias/pending-write restart and
plaintext SQLite coexistence in 32-bit API30 processes with 4096-byte pages.
x86 executed directly; ARM32 executed through `libndk_translation.so` on the
installed x86_64 image. All four packaged ELF slices retained 16KiB alignment.
These checks do not establish physical ARM execution, 16KiB runtime behavior,
full Kotlin/RN application parity or a later source version.

The historical outside-checkout manual APK built from that local AAR/POM passed all 23
predeclared groups on the task-owned x86_64 API37 emulator with actual16384-byte
pages, including force-stop/fresh-process restart. The arm64 slice is build-only
(ELF-checked, never executed). This is a manual SDK build, not an AGP build; the
RN Android consumer separately checks Gradle consumption. No publishing or
distribution license is granted.

Policy-checkpoint verification, 2026-10-31: the full Kotlin suite passed 103
unique cases across 24 entrypoints, including 17 controlled policy/lifecycle
cases, four real-JNI policy cases and 18 background-reporting cases. A fresh final
AAR with both rebuilt JNI slices passed packaging P01–P08; its outside-checkout
consumer passed all 25 groups, including new memory/protected CP13 cases and a
fresh-process restart. The task-owned x86_64 API37 runtime reported 16384-byte
pages; arm64 remains build/ELF-only. See the
[policy verification record](../../docs/implementation/phase-seven-kotlin-policy-followup.md)
for source/artifact hashes, RED evidence and review. These results bind the
recorded policy checkpoint; subsequent core changes require a fresh native
build and consumer run before a later-source claim.
