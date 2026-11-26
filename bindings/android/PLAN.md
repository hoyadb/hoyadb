# Kotlin Android SDK implementation packet

Authority: committed API-0011, SPEC-0033, RFD-0006, TC-0036 G01–G24 and
ADR-0035 / approval1a914cd. This language/package packet adds no database, queue,
resolver, schema-validation or storage algorithm. Consumer review/commit precedes
SDK tests/implementation. Version0.1.0, ABI1, Maven coordinate
`dev.hoyadb:hoyadb-android:0.1.0`, Android minSdk24 (native clang android24),
x86_64 executed on the owned API37/16KiB emulator; arm64 build-only unless run.

## Existing installed tool and approved runtime closure

`toolchain-review.json` (copied unchanged from the task evidence directory) verifies all86 hashes
against the approved review commit and the current exact artifact index, plus:

- Installed compiler `/Applications/Android Studio.app/Contents/plugins/Kotlin/kotlinc/bin/kotlinc`,
  version2.2.20-release-333, compiler JAR SHA256
  `26d99b0510e4bc279327827c9f8ce1a7bc859afcef5f8c2cf6d32e0f8751f6df`.
- Its actual script loads kotlin-preloader.jar then kotlin-compiler.jar. The
  compiler manifest Class-Path adds annotations-13.0.jar, kotlin-stdlib.jar,
  kotlin-reflect.jar, kotlin-script-runtime.jar and kotlinx-coroutines-core-jvm.jar.
  All eight tool/script/closure hashes are recorded. These installed build-tool
  libraries do not become SDK runtime dependencies. No compiler plugin is enabled.
- Exclusive SDK/runtime classpath: approved kotlin-stdlib2.1.20 SHA256
  `1bcc74e8ce84e2c25eaafde10f1248349cce3062b6e36978cbeec610db1e930a`
  and annotations13.0 SHA256
  `ace2a10dc8e2d5fd34925ecac03e4988b2c0f851650c94b8cef49ba1bd111478`.
  Both downloaded archive bytes matched approved pins before execution.
- Compilation uses `-language-version 2.1 -api-version 2.1 -jvm-target 1.8
  -no-stdlib -no-reflect -classpath <approved-runtime-jars>:<android.jar>`.
  No `-Xmetadata-version` disguise. A synthetic tool-only probe first verifies
  naturally emitted Kotlin Metadata mv2.1 and compilation of an independent
  consumer under language/API2.1 before any SDK compilation. This is compiler2.2.20
  producing language/API/metadata2.1, never a claim to have compiled with2.1.
- Installed JDK17.0.1, Android SDK build-tools37.0.0/platforms android-37.0,
  aapt2/D8/apksigner/zipalign are recorded by exact path/version/hash in the
  eventual build manifest. No downloaded compiler-embeddable, Gradle plugin or
  additional runtime is activated. Manual standard AAR packaging is labelled
  honestly; no Gradle cache/remote build cache is used by this standalone lane.

Compiler probe command (arguments passed as a process array, not interpolated
shell code): installed kotlinc with the flags above, approved JAR classpath,
`MetadataProbe.kt -d metadata-probe.jar`; installed `javap -v` inspects Metadata;
then `MetadataConsumer.kt -classpath metadata-probe.jar:<approved jars>` compiles.
Tool probe is setup evidence, not a behavioral SDK RED.

## Public typed surface

Package `dev.hoyadb`. The existing package-private Java NativeBridge remains
unchanged, with owned byte-array open/call and binary key. Proposed common public
`HoyaNativeTransport` exposes asynchronous owned-byte `open(frame,key,callback)`
and `call(frame,callback)` plus ABI version, all on the same serial non-UI runtime.
Callbacks receive owned byte[] or a static kind after the JNI call/borrow ends;
input arrays are copied before queueing and owned key copies cleared afterward.
Kotlin SDK and RN use this exact AAR runtime and one native library. Terminal attempts finish/abort/cancel_request/close/reset signal the shared host
wake only after the native attempt returns, including failures that may release
admission. begin(false) and ordinary calls do not signal. This prevents a Kotlin FIFO waiter sleeping forever when an RN owner
releases via a different wrapper. JNI C and its six export symbols do not change.
No arbitrary handler or callback is invoked under Rust's registry lock. Ordinary
transport JSON never contains the key. This byte transport is an advanced host
capability, not another schema/query/merge algorithm or wire operation.

- `ProtocolValue`: null/boolean/string/array/object only, immutable owned copies;
  advanced raw operation arguments/results, exact all-operation escape hatch.
  Never permit a raw Java Number as a wire integer. Strict UTF16→UTF8 encoder
  uses CodingErrorAction.REPORT and the decoder REPORT; reject lone surrogates.
  Frame byte1MiB/depth64/node100000 budgets apply before native submission.
- `Cell`: Null, Boolean, String, Integer(Long), Float(finite Double),
  Timestamp(Long), Reference(String). `JSONValue`: corresponding business
  null/bool/string/integer/finite-float/array/object tags, excluding cell-only
  timestamp/reference. Business JSON parse/canonicalization delegates to
  owner-independent Rust json_decode/json_encode, including fake transport.
- `UnsignedDecimal`: checked canonical0..18446744073709551615 decimal String;
  owner/scope/subscription handles are positive instances internally. No signed
  JavaLong overflow for u64 handles/counts/drain. Versions/limits/indexes fit u32.
- `Schema`, `Table`, `Column`, `Relation.ToOne/HasMany/ManyToMany`, all seven
  `MigrationStep` variants and explicit default none/some(null) union;
  immutable constructor snapshots and enum wire spelling exactly API-0011.
- `Query`, `Predicate`, `Order`, `Source`, all seven operators, ordered predicate
  arrays; `Write.Insert/Replace/Remove`, `Row`, `Change`, typed link/unlink.
  No host validation/filtering/inferred schema or raw internal schema version.
- `Database(eventExecutor, idGenerator?, uptime?, timer?)`: explicit serial
  non-UI native executor (shared process runtime); suspend openMemory/openProtected,
  schema/protection/CRUD/batch/link/unlink/fetch/close/execute. `MemoryStore` owns
  retained real memory handle, suspend create/drop, no filesystem or key callback.
  `execute(op,args)` covers every API-0011 operation and preserves ownership,
  managed-stream guard, mutation wake, reset and epoch semantics.
- openMemory accepts optional deterministic32hex namespace; when supplied it calls
  no SecureRandom/key/path/platform-store capability. Normal namespace uses
  SecureRandom16bytes, with failure propagated and no clock fallback. Protected
  open checks32keybytes before any path action and copies its owned key only;
  temporary owned key copy is cleared after native completion. Caller owns its key.
  Key validation precedes even evaluating Context.noBackupFilesDir/filesDir getters,
  since the AOSP getters may create their private roots. No Context path getter
  is evaluated by any memory open.
- Protected default uses a dedicated canonical nonsymlink child of Android
  Context.noBackupFilesDir; explicit backup opt-in selects filesDir. Read back
  canonical parent/child identity; the OS consumer fixture records that actual
  noBackup/filesDir selection independently. Protection retains exactly its four
  API-0011 fields and native backup remains host_managed; no extra public field
  or claimed backup-service execution. Never infer policy
  from its backup_excluded JSON Boolean. Same app cannot open a canonical file twice.
- Managed `observe/observeStatus/observeOutcome/observeProgress` return explicit
  cancellable `Subscription`; immutable typed events invoke only selected Executor.
  Register callback before next-turn initial drain. Reentrant raw/managed drain is
  Busy, bounded owned prefix snapshots dispatch once, liveness rechecked immediately
  before each callback; cancellation suppresses remaining buffered callbacks.
  Mutation drains coalesce; one outstanding16ms count timer is not postponed by
  later writes. Injectable monotonic uptime/timer, not captured UTC, drives drains.
- `CommandDefinition`, immutable `CapturedContext`, `CommandResult`,
  `CommandScope`: suspend scoped args/context/query/writes/link/unlink only;
  no request/execute method. `startCommand(..., suspend (CommandScope)->JSONValue)`
  returns `CommandTask` with explicit cancel + suspend await; suspend `command`
  convenience can accept that explicit cancellation token. No promise of Job
  cancellation without kotlinx. Arguments/context copied at invocation, immutable
  through await. Proposed IDs sampled before entering Rust, exact injected value;
  strict entity allocator untouched, approved `local:test-fixed-id-001` command.
- Global owned wake events register before begin to avoid lost FIFO release;
  no native lock/core borrow spans a handler. Waiting jobs wake on release/close,
  not a busy-spin polling loop. Explicit cancel or owner close immediately retires
  queued/admitted requests and settles task even if its handler never cooperates.
  Late handler resume cannot finish/reuse scope. Once finish is submitted,
  cancellation preserves its actual durable result.
- `QueueStatus/QueueEntry/PushBatch/RemoteOutcome/ReconcilePlan`, typed exact
  attempts/deadline and payload-free status; preparePush, receive/receiveEntity/
  receiveWire, pendingReconciliations/reconcile/rollback/cancel/forget.
- `EntityConfig/EntityChange/PullPage/PullRequest/SyncProgress`, entityContext
  provider sampled before each direct entity mutation, configure/reconfigure,
  batch/patch/entityLink/entityUnlink, begin/apply/abortPull, reset modes. One-shot
  captured context is never reused after an error or duplicated as command diff.
- Passive `MergeHook` gets immutable owned current/metadata/incoming/default
  parts from core and returns Row|null asynchronously; prepare/step/reply/commit/
  abort remains core. Per-table hooks, sequential repeated-row proposals, complete
  stale guard, Busy retained owned proposal without rerunning hook, StalePull
  discards/refetches old ticket, all failures retire stage. No host LWW/clocks.
- `startSync(transport,reconciler?,clock,jitter?,timer?,onError)`: host-owned
  async transport, production SecureRandom0..2500 jitter, injected deterministic
  tests, no empty sends, one core deadline timer, resume wake, Busy retains
  transport response and mapper output; stale epoch never publishes. Frozen
  opaque envelopes use Rust lossless codec helpers to form batch JSON. Library
  has no mandatory networking library, auth/TLS or background-service promise.
- `FakeTransport` attaches exact command key/entity triple using those same Rust
  codecs; handler result remains business JSON; delay/reject/drop ACK options
  affect transport only. It is not a database/resolver or a second retry engine.
- `diagnostics` remains exact ten static-named count/Boolean fields. Error kind
  is static; no raw Throwable cause/message/path/SQL/key or domain payload in
  implicit SDK toString/error/diagnostic formatting. Explicit data APIs return data.
- Client owns deterministic close; scoped `useDatabase` closes on success/error.
  A JVM reference-queue fallback may release an abandoned native handle using
  only detached handle/runtime state, never a strong callback→Database cycle.
  No deterministic GC timing is promised. Close/reset stop timers/transport work,
  cancel scopes/listeners/merge plans and advance epoch before stale delivery.
  Opening reserves synchronously; close during a delayed open waits for its late
  acquired owner disposal before allowing a replacement open. Failure releases
  only the reservation that owns that opening epoch.

Android org.json is platform-provided and restricted to trusted ABI replies and
structural typed frame construction. It never parses opaque server business
numbers; unknown/raw number reply nodes reject. Duplicate/unknown ordinary input
keys remain the Rust exact-frame parser's responsibility for raw byte consumers.

## Named acceptance before implementation

K01/G01–04: two literal unrelated caller schemas, every Cell (i64 extrema and
integral Float) and JSONValue; all relation/query operators and authored seven-step
migration, null/absent distinction and independent exact result assertions.
K02/G03/G11: malformed UTF16/UTF8, wire node/depth/byte bounds, fullu64 and u32
boundaries, near-bound mutation no committed-failure and large owned paging.
K03/G20/G22: preloaded native runtime plus deterministic namespace real memory
CRUD/observe/commands while storage/entropy/network hooks reject; exact protection.
K04/G22/G24: retained memory v1→v2 authored reopen, failed reopen preserves original,
active duplicate/drop Busy, explicit closed drop.
K05/G09: controlled delayed open, second open Busy, close/reopen waits disposal;
failed old open cannot clear new reservation; no stale owner/key publication.
K06/G05/G10: callback initial delivery without write, selected UI Executor,
reentrancy/raw drain Busy, cancel in first callback suppresses later delivery,
mutations and count deadline schedule exactly one unpostponed host timer.
K07/G06/G07: genuine suspend/resume commands, outer FIFO across two owners,
injected local ID/context fixed clock exactness, staged row invisible before finish.
K08/G06/G10: shared byte transport release wakes a Kotlin FIFO waiter, including
an RN-style low-level owner; begin(false) never self-wakes into a busy loop.
Noncooperative suspended handler cancel/close immediately releases
admission; late resume rejected; submitted finish result survives cancellation.
K09/G17/G18/G21: real command/entity mixed reordered partial duplicate/ACK/drop/
rejected/corrected transport, fake exact correlations/i64 preservation,
payload-free attempts/deadline/status and exactly10 diagnostics fields.
K10/G19: injected clock/jitter extremes, persisted frozen retry after protected
restart, no early/empty resend, production jitter range, Busy retention does not
repeat network/mapper or spin.
K11/G17/G22: partial entity patch/removal/mapping/managed joins/tombstone and
protected reopen, typed authoritative correction clocks with older/newer pull.
K12/G12–15: asynchronous passive hook two same-row changes, core validator rejects
identity/clock/unrelated-field corruption atomically; stale retires/refetches,
Busy retries same owned candidate without repeating hook; abort/close cleanup.
K13/G16: default actual noBackup path versus opt-in filesDir, malformed key before
path action, correct/wrong key, ciphertext+canary absence/foreign plaintext/
authenticated corruption byte preservation using only synthetic owned files.
K14/G23: ordinary locally packaged AAR installed in outside-checkout consumer,
no relative spikes/source imports; public Kotlin metadata consumable at language2.1,
actual APK native ZIP+ELF16KiB checks and execution on isolated owned emulator.
K15/G23/G24: APK force-stop/relaunch persistence/queue/checkpoint, event affinity,
async awaiting/cancel and close/reset stale response handling; current source /
artifact / tool / dependency hashes and actual runtime counts agree. Existing
core/provider fault suites stay separate supporting proof, never substituted for
SDK execution. arm64 slice explicitly build-only unless actual device run.

Every slice begins with compiling expected unsupported-operation REDs, not a
missing import, JAR, native link or emulator setup failure. Preserve those setup
failures separately. No publication, signing identity, user AVD/store, dependency
outside approved pins, implicit reset or security assertion relaxation.

## Packaging and rollback

Manual resource-free standard AAR: AndroidManifest.xml(minSdk24), classes.jar,
consumer-rules.pro and jni/x86_64 + jni/arm64-v8a libhoyadb_jni.so from native lane.
Local Maven POM declares exactstdlib2.1.20 (annotations supplied by its exact graph).
Build records licenses/tool/source/ABI/public snapshot and SHA256s. Consumer uses
installed aapt2/D8/apksigner and a fresh task-only synthetic signing key. Native
libs are packaged uncompressed with real APK16KiB ZIP alignment (`zipalign -P16`),
not extracted merely to bypass alignment. Verify actual ELF LOAD alignment plus
`zipalign -c -P16` and actual16384-page emulator runtime. Consumer root and app ID
are task-owned; uninstall removes only that synthetic app. Retain reproducible
source, clean only owned remote/store/process identities. Reverting unpublished
AAR/source does not mutate customer data or provide a migration reset.

Primary tooling references consulted (behavior still requires actual runtime):
https://kotlinlang.org/docs/compiler-reference.html (language/api/no-stdlib flags);
https://developer.android.com/guide/practices/page-sizes (ELF+ZIP checks).
Android library content page fetch timed out; standard AAR contents will also be
checked against the installed/approved RN consumer integration, not claimed from
an unread page.

## Reviewed test-only split DEX build amendment

Repeated test iterations currently reprocess both exact approved runtime JARs in
D8. An optional `--tests --split-runtime` mode will leave production/final APK D8
unchanged and cache only the standalone approved test runtime DEX. No compiled
SDK/test class may come from that cache or an earlier full test artifact.

The cache identity includes SHA256 of the exact stdlib2.1.20 and annotations13.0
JARs, installed android.jar, D8 launcher and its actual lib/d8.jar, selected JDK
java executable, min-api24 and the exact D8 program-runtime flags/profile. A
changed input creates a different identity. An existing identity with missing,
extra, malformed metadata or a mismatched runtime DEX hash fails closed; it is
never silently trusted/repaired. Root and files must be task-owned nonsymlink
paths. First generation uses a new task-owned directory and publishes metadata
only after successful D8, then validates it exactly as a reused cache.

The SDK/test invocation passes classes.jar as its sole D8 program input and the
two approved runtime JARs as --classpath. The Android test runner validates and
copies both resulting SDK/test DEX and runtime DEX, then uses exactly
CLASSPATH=<test.dex>:<runtime.dex>. Manifests record both hashes and all commands;
no stale SDK class or test implementation fallback is accepted. Installed JDK
selection is explicit for D8 as well as kotlinc. All reviewed archive/hash guards
remain effective under Python -O; validation does not rely on assert statements.
Build output directories are fresh, rejecting accidental stale-class reuse.

Before using split mode for acceptance: pure stdlib tooling tests first observe
RED for unsupported split planning, changed min-api/flags, corrupt/incomplete
cache and mismatched DEX bytes; then minimal implementation and GREEN. Actual
Android current SdkTests6 and TransportTests2 must pass both full and split DEX
modes unchanged, with the same JNI artifact. Record source/tool/input hashes and
runtime counts. The actual AAR and final independent consumer APK remain ordinary
complete program-class builds and retain ZIP/ELF16KiB verification.

Build manifests additionally record complete source/tool SHA256 maps before and
after compilation and reject drift. Include Kotlin/Java sources, selected tooling
scripts and classes.jar input/output hashes; retain exact command arrays. This is
source/artifact provenance, not runtime logging of application data. Acceptance
cases are recorded in TEST_TOOLING_CASES.md.

## Packaged Kotlin testing helpers — consumer review before TDD

The separate `dev.hoyadb.testing` entry exposes `PreloadedRuntime`, whose
constructor is private. `preloadRuntime(): PreloadedRuntime` explicitly calls
the existing HoyaNativeTransport.abiVersion and accepts only ABI1 before returning
the capability. This named bootstrap occurs before a hermetic memory-open window;
it adds no C/JNI export, library copy, entropy or storage-provider initialization.

`suspend openMemoryDatabase(runtime: PreloadedRuntime, schema: Schema,
namespace: String, eventExecutor: Executor, idGenerator: (() -> String?)? = null,
options: DatabaseOptions = DatabaseOptions(), store: MemoryStore? = null,
migrations: List<Migration> = emptyList()): Database` requires that capability
and an explicit namespace. It constructs the ordinary Database and delegates its
real openMemory operation. Rust remains the namespace/schema validator; there is
no namespace fallback and no Context/key/provider argument. If open fails, close
only the new failed owner before propagating its static error. No shadow engine.

`ManualClock(initial: Long = 0)` provides synchronized `now(): Long`, `set(Long)`
and `advance(delta: Long)`. Advance uses checked signed64-bit addition, leaves its
old value unchanged on overflow and throws static InvalidRequest. Explicit set
and signed deltas are test controls; the existing core validates a captured UTC
value when used. The helper is opaque in implicit formatting, not a second
scheduler. Caller closures inject it into CapturedContext and startSync normally.

FakeTransport's existing handler type receives the full immutable production
JSONValue envelope, matching API-0011 §7. Handlers read args/changes explicitly;
the fake itself attaches correlation fields and returns core-encoded responses.

Named cases, genuine unsupported-stub RED after review and the quiet window:

- KT01: explicit ABI1 bootstrap returns an opaque private-constructor capability; the
  memory factory opens two unrelated caller schemas with fixed namespaces and
  ordinary public operations, no Context/key/entropy/provider parameter.
- KT02: injected exact ordinary ID and local-prefixed command ID/context/clock
  produce the production result; invalid namespace rejects without fallback and
  a later valid open remains usable. Retained-store authored upgrades still use
  the existing MemoryStore/Database path, not a fixture database.
- KT03: now/set/advance and signed64 overflow/underflow preserve exact values;
  failed advance leaves prior value unchanged; implicit formatting has no value.
- KT04: full-envelope fake handlers see command key/args and entity triple/changes,
  preserve i64 extrema through the Rust codec and cannot use array position as
  correlation. Reordered/duplicate/rejected/dropped/delayed transport follows the
  production receive and durable retry path. This extends the existing K09/K10
  tests, not a replacement for the outside-checkout packaged APK gate.

## Managed explicit drain — consumer review before TDD

`suspend Database.drainNotifications(): Unit` invokes the same managed bounded
prefix/liveness/epoch dispatcher used by automatic observation drains and awaits
selected-executor callback dispatch. A concurrent or callback-reentrant explicit
managed drain returns Busy; automatic mutation wakes retain their existing
coalesced follow-up. Advanced raw execute("drain") remains Busy whenever managed
registrations/listeners/delivery are owned. No new clock, deadline, throttle,
native operation or alternate event queue is introduced.

K06a/G05: with a real count observer, injected monotonic clock and deliberately
unfired host timer, advance to16ms and explicitly drain. Assert the selected
executor receives the exact due count once, callback reentry returns Busy, raw
drain remains Busy while the subscription is live, and cancellation restores raw
manual mode. This begins with an actual UnsupportedOperation stub RED after
review/commit and the quiet window, then minimal dispatcher reuse.
