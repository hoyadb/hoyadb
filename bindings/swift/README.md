# HoyaDB Swift SDK (local, unpublished)

For AI-assisted integration, read the repository's [tool, skill and reference
catalog](../../AI.md). It describes the expected remote-dependency workflow;
use the resolved SDK version's public Swift interface and release guidance.
JS declarations do not establish Swift convenience APIs. This source link does
not add a file to the XCFramework or establish a remote release.

`HoyaDB` is a SwiftPM product backed by the same general Rust facade as the
other SDKs. Schemas, queries, migrations, validation, scoped writes, the durable
queue and entity resolution execute in Rust. Swift owns async handlers, callback
executors, cancellation, platform entropy and backup policy.

From the repository root, build the local XCFramework from the approved locked
Rust graph, then capture the compilation-parent digest from that successful
local build before checking or distributing a copy:

```sh
python3 bindings/swift/build-artifacts.py
HOYADB_APPLE_PARENT_SHA256="$(shasum -a 256 bindings/swift/Artifacts/native-build.json | cut -d ' ' -f 1)"
python3 bindings/swift/check_artifacts.py --expected-parent-sha256 "$HOYADB_APPLE_PARENT_SHA256"
swift test --package-path bindings/swift -Xswiftc -warnings-as-errors
xcrun swift-format lint --strict --recursive bindings/swift/Sources bindings/swift/Tests bindings/swift/Examples
```

This requires the pinned Rust toolchain and installed Apple Rust targets/Xcode.
The script fetches no packages and publishes nothing. Generated artifacts are
ignored. The current recipe builds macOS x86_64, iOS arm64, and a universal
x86_64/arm64 simulator slice. Arm64 macOS is not included by this recipe.
Continue only after the build succeeds: its notice finalizer also requires the
reviewed provider evidence to match the newly built libraries. A
`stale-provider-evidence` failure needs fresh reviewed build evidence; supplying
a digest to the checker does not override that failure.
Every slice uses the same `libhoyadb_native.a` basename because CocoaPods
rejects vendored static XCFrameworks whose slice binary names differ;
Each slice's headers keep `hoyadb.h` at the top level, so `#include <hoyadb.h>`
is unchanged. The `HoyaNative` module map lives in `Headers/HoyaNative/`. An app
that receives the framework through both SwiftPM and the React Native pod
therefore has two copies on its search paths; clang uses the first module map
it finds rather than reporting `redefinition of module 'HoyaNative'`.
`check_artifacts.py` checks the slices, basenames, header layout, a
two-copy `import HoyaNative` and the recorded hashes. It also accepts the path
of a packed copy of the framework after the required digest argument:

```sh
python3 bindings/swift/check_artifacts.py --expected-parent-sha256 "$HOYADB_APPLE_PARENT_SHA256" /path/to/packed/HoyaNative.xcframework
```

Keep the digest from the trusted local build separately from the distributed
framework. When checking someone else's build, obtain its compilation-parent
digest through an independently trusted build record. Do not derive it from
the packed framework's `native-build-parent.json` or `packaging.json`: those
files are the claims being checked, not the trust anchor.
`Artifacts/native-build.json` records exact source/artifact hashes and commands;
compilation alone does not establish runtime support.
The builder captures the root and five native crate manifests, lockfiles,
Rust toolchain pin, all files below their `src` directories, conventional
`build.rs` files, repository Cargo configuration, C headers and the builder
itself. It re-enumerates these inputs after compilation, so additions and
removals also invalidate the build. Changes to the local dependency graph or
build scripts reading resources outside `src` require an inventory review;
toolchain, environment and external dependency evidence remains separate.

Before claiming that an existing artifact verifies current source, compare its
captured inputs with the complete current build inventory, including additions,
removals, Cargo manifests and build scripts. The artifact checker binds its
compilation parent; it does not perform that current-source comparison.

Use a local SwiftPM dependency on this directory and the `HoyaDB` product. The
ordinary consumer under `Examples/GeneralConsumer` imports only public APIs and
can be copied outside the checkout. SwiftPM links the existing Security and
CoreFoundation platform frameworks; plain C/Swift consumers must link them too.
Link one native facade runtime per process.

```swift
let db = Database()
try await db.openMemory(schema: Schema(version: 1, tables: [
  "notes": Table(columns: ["body": Column(.string)])
]))
let row = try await db.insert("notes", fields: ["body": .string("offline")])
let rows = try await db.fetch(Query("notes"))
try await db.close()
```

`openProtected` takes a caller-supplied binary32-byte key and an existing canonical
app-owned nonsymlink directory. It applies and reads back the Apple backup flag
before opening. Wrong/missing keys fail closed; there is no plaintext fallback,
key derivation, rotation or arbitrary-store import API. Production namespaces
use Security CSPRNG. Explicit namespaces, IDs, clocks, jitter and retained real
memory stores support deterministic tests. A command-created ID must retain the
core `local:` prefix; ordinary CRUD accepts unprefixed IDs. Exhausted/invalid
custom generators reject before the atomic call and never silently fall back.

`command` captures context and runs an async handler with a tracked `CommandScope`.
The scope offers staged reads/writes but no execute/command method. Independently
captured outer commands queue FIFO. Awaiting a queued child inside its admitted
parent is unsupported application misuse. Cancellation before finish aborts;
a finish already submitted may commit and its actual result is preserved.

Managed `observe`, queue status/progress/outcome subscriptions deliver initial
events on the selected executor without a write or manual drain. Cancellation
is checked again before each callback. `drainNotifications()` uses this same
dispatcher; advanced raw drain is Busy while managed listeners own the stream.
The count timer does not postpone the original16ms deadline. When that deadline
drain comes before the core's window has ended and a count listener still has no
value for a later write, one follow-up16ms timer is armed; it remains the only
outstanding timer and a window without any drain ends the sequence. No application
callback runs inside a native call or borrowed Rust frame.

`configureEntities`, `entityBatch`, partial `patch`, entity joins, typed pull pages
and async per-table merge hooks forward to the core. A stale merge discards the
page/proposal and fetches again from the unchanged checkpoint. Busy retains the
owned proposal; cancellation disposes its staged ticket even if app code keeps
waiting. Hooks cannot change clocks, resurrect tombstones or override core guards.
`close()` and reset retire the entity context provider and merge hooks together
with the owner epoch: configure them again after reopening or resetting.
A managed sync pass that fails before its own `prepare_push` reply schedules no
timer from an earlier pass's deadline; the next write, receipt or resume wakes it.

`preparePush` returns opaque frozen envelope strings. `startSync` joins complete
strings into an array, calls the supplied transport and retains responses/plans
through Busy. The application supplies the reconciliation mapper; there is no
automatic business correction. Core due times drive the host timer, with bounded
platform CSPRNG jitter. Close/reset stop scheduling and invalidate stale work.
The advanced `execute(operation, arguments:)` surface exposes exact ABI1 grammar
for explicit control; it cannot route binary-key open or escape owner checks.

SPEC-0040 adds an optional conflict policy and a resolution callback:

```swift
let db = Database(
  conflict: ConflictPolicy(default: .discard, commands: ["sensitive-edit-v1": .stop]),
  onResolution: { key, resolution in
    // Record that current local data was kept; this does not claim server convergence.
    print(key, resolution)
  })
```

The default is Stop. Constructor configuration is installed before open returns
and reapplied on reopen. `setConflictPolicy` changes only the current owner;
validation and Busy errors leave its prior policy intact. Failed initial
configuration disposes the unpublished owner. A failed disposal remains
available for an explicit `close()` retry and blocks another open.

`reconcileResolving(_:plan:guards:)`, `rollbackResolving(_:)` and `discard(_:)`
return `.completed` or `.keptCurrent(discardedKeys:unmapped:skippedWrites:)`.
The existing unit-returning reconciliation/rollback methods and managed sync
use those same reporting operations. So does an advanced raw `execute` of
`reconcile`, `reconcile_guarded` or `rollback`: it is submitted as the resolving
operation, delivers `onResolution` and returns null. Managed sync reports each
failed non-Busy receipt once and still reconciles in the same pass; `pullOnce`
waits for a Busy `begin_pull` through the admission release. Only a current-owner KeptCurrent result
invokes `onResolution`; throwing callbacks remain installed and report one
redacted callback failure through `onBackgroundError`, without changing the
committed result or invoking the sync error adapter. Close/reset suppress late
results from a retired owner epoch. `onResolution` runs synchronously on the
database actor: keep it short and do not block; schedule longer application work
separately. The historical initializers remain available.

Discard keeps current rows and retires only eligible command recovery; it does
not force rollback or bypass guards, later work or in-flight protection. Queue
status exposes `.discarded` with `rolledBack == false`. This approved enum
addition affects exhaustive Swift switches, and older database binaries refuse
stores containing discarded history. Reverting the policy to Stop does not
remove that history or promise downgrade compatibility. See
[SPEC-0040](../../docs/design/spec/SPEC-0040-simple-command-conflict-policy.md)
and the [source/API/export checks](../../tools/release/README.md#compatibility-evidence).

The separate `HoyaDBTesting` product uses the production database. Its
`MemoryTesting.open` factory accepts an explicit namespace, caller schema,
finite ID generator and optional retained `MemoryStore`/authored migrations.
`ManualClock`, `ManualUptime` and `ManualTimer` provide deterministic host
capabilities; pass uptime/timer closures through the public `Database`
initializer. UTC and monotonic time remain separate. Timer callbacks run after
the helper's lock is released, and cancellation removes pending work.

`FakeTransport` decodes and encodes business values with the production Rust
JSON codec. Command-name and entity-table handlers return only applied/corrected
business values or rejection; the helper attaches the exact command key or
entity table/record/write tuple from each envelope. Optional delay, dropped-ACK
and reversed-reply controls exercise the real queue's recovery behavior. This
helper contains no storage, retry planner or conflict resolver. The ordinary
consumer demonstrates the public production/testing products together.

These files are an implementation in progress until the parent platform SDK report
records the final source cut, full ordinary consumer and supported platform gates.
No remote Swift package, release or publication is claimed.
