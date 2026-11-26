# Native desktop boundary experiment

[SPEC-0018](../../../docs/design/spec/SPEC-0018-native-validation-spike.md)
and [TC-0018](../../../docs/design/tests/cases/TC-0018-native-validation-spike.md)
describe this isolated C ABI bridge. It uses the shared Rust spike core;
[SPEC-0021](../../../docs/design/spec/SPEC-0021-native-sqlite-persistence-spike.md)
adds optional native SQLite persistence through approved `rusqlite =0.40.2`,
with default features disabled and `bundled` enabled.
Swift calls Rust insertion/query code and copies returned rows during callbacks.
Subscriptions use explicit notification draining, and a second Swift executable
tests async command scopes. This native slice contains no browser binding.

Run from the repository root on the validated x86_64 macOS host:

```sh
cargo fetch --manifest-path spikes/cross-platform/native/Cargo.toml --locked
cargo test --manifest-path spikes/cross-platform/native/Cargo.toml --offline --locked
cargo fmt --manifest-path spikes/cross-platform/native/Cargo.toml --check
cargo clippy --manifest-path spikes/cross-platform/native/Cargo.toml --offline --locked --all-targets -- -D warnings
cargo build --manifest-path spikes/cross-platform/native/Cargo.toml --offline --locked
swiftc -target x86_64-apple-macosx26.5 -warnings-as-errors -framework Security -framework CoreFoundation -import-objc-header spikes/cross-platform/native/include/hoya.h spikes/cross-platform/native/tests/main.swift spikes/cross-platform/native/target/debug/libhoyadb_native_spike.a -o spikes/cross-platform/native/target/native-tests
spikes/cross-platform/native/target/native-tests
swiftc -target x86_64-apple-macosx26.5 -parse-as-library -warnings-as-errors -framework Security -framework CoreFoundation -import-objc-header spikes/cross-platform/native/include/hoya.h spikes/cross-platform/native/tests/experiments.swift spikes/cross-platform/native/target/debug/libhoyadb_native_spike.a -o spikes/cross-platform/native/target/native-experiments
spikes/cross-platform/native/target/native-experiments
```

Tools observed: Rust 1.92.0-nightly, Apple Swift 6.3.2 and Apple clang 21.0.0.
storage follow-up: all three native Rust tests and both Swift executables also
pass after rebuilding with the root's Rust 1.98.1 pin. This crate remains
excluded from the production workspace and owns its lockfile.
This validates real Swift desktop processes, not mobile, other architectures,
general concurrency, packaging, or the choice of production binding tool. Async
Swift suspension can change OS threads; native calls remain serial in the harness.

## Evidence — 2026-10-20

- RED: the Swift executable compiled and linked, then all five named cases
  failed because the stub `hoya_open` returned zero (exit 1).
- RED: all three Rust error-barrier tests failed at the uncaught panic,
  poisoned-lock unwrap, and unimplemented handle allocation (exit 101).
- GREEN: five Swift cases and three Rust tests pass. Cases exercise full
  typed rows, filtering, isolated stores, overwritten Unicode input buffers,
  i64 extrema, invalid input/handles, disposal, and a callback's nested read.
- Formatting, Clippy with warnings denied, and Swift compilation with warnings
  as errors pass. A Clippy test-module placement finding was fixed.
- Subscription/scope RED: five new core cases failed on stub observation, and
  three new Swift cases compiled/linked then failed on stub `hoya_observe`.
- GREEN: core has 11 passing tests; N07–N09 pass in Swift with a real suspended
  handler, reentrant notification callback, busy/stale scope rejection, atomic
  publication and abort cleanup. No Rust borrow spans an await or callback.
- One observed N07 insertion-to-callback sample was 25,075 ns, including an
  explicit immediate drain, on OS thread 66234674. It is a single host sample,
  not an average, UI-thread promise, or performance budget.

## Mechanism and limits

Core maintains membership for the single status predicate and queues token/count
signals. `hoya_drain` delivers them on the drain caller's thread, with no foreign
callback/context retained. Re-querying reads current rows, not a historical
snapshot. The app's drain cadence contributes to latency. Unsubscribe or close
during a callback suppresses remaining extracted events for that registration.

Swift drives the handler: begin scope, stage, await, re-enter, finish or abort.
Core permits one active scope per store and keeps staged rows out of ordinary
reads and notifications. This is not cross-store serialization, durable commit,
cancellation/liveness handling, or a decision about interleaving other writers.
Hung or abandoned scopes remain active until explicit finish/abort or store close.
This experiment does not implement observer sharing, updates/deletes, or the
full production query/reactivity model.

## Native SQLite restart experiment — 2026-10-21

`hoya_open()` remains in memory. `hoya_open_persistent(path)` restores a separate
Store from the fixed SQLite row table, validating types, paid 0/1 and unique IDs
before exposing a handle. Empty paths, `:memory:` and `file:` SQLite URIs are
rejected before opening, preventing silent memory selection. Existing insert and committing scope calls prepare a
cloned candidate, persist all its committed rows in a SQLite transaction, then
publish only after commit succeeds. Status 4 means ordinary storage failure:
prior memory, events and active scope remain unchanged. Abort needs no disk write.
Shared core owns all query/membership/scope behavior; native adds only persistence.
Whole-store clone/rewrite costs O(total rows); this is an experiment baseline.

Compile and run a writer then a reader as separate processes, using a fresh
harness-owned temporary directory (preserve its printed path for inspection):

```sh
swiftc -target x86_64-apple-macosx26.5 -warnings-as-errors -framework Security -framework CoreFoundation -import-objc-header spikes/cross-platform/native/include/hoya.h spikes/cross-platform/native/tests/persistence.swift spikes/cross-platform/native/target/debug/libhoyadb_native_spike.a -o spikes/cross-platform/native/target/native-persistence
spike_fixture_dir=$(mktemp -d /tmp/hoya-swift-persistence.XXXXXX)
spikes/cross-platform/native/target/native-persistence writer "$spike_fixture_dir/fixture.sqlite"
spikes/cross-platform/native/target/native-persistence reader "$spike_fixture_dir/fixture.sqlite"
```

Bundled SQLite was observed as 3.53.2. The C object targets macOS 26.5, so the
Swift commands explicitly use that deployment target rather than its 26.0 default;
these commands were tested on x86_64 macOS 26.7.1. Other deployment floors are
unvalidated. The Rust dependency and native lockfile are isolated from core.

RED: three Rust persistence cases failed because persistent open returned zero;
two invalid-input cases already passed. The real Swift writer compiled/linked,
then exited 1 at the same missing behavior. GREEN: five Rust persistence cases
pass, including deterministic SQL-trigger rollback for insert and scope finish,
malformed-data rejection and memory-mode isolation. Writer and independent reader
both exit 0, preserving Unicode, i64 extrema, committed scope rows and abort absence.
See [TC-0021](../../../docs/design/tests/cases/TC-0021-native-sqlite-persistence-spike.md).

No query execution by SQLite, production default-engine conformance, encryption,
multi-writer coordination, hostile-file sandbox, crash/power-loss test or durable
command queue is established. Only synthetic temporary files were used. A native
panic still has no rollback guarantee; filesystem paths and valid C pointers
remain caller responsibilities. SQLite journal/sync defaults were not weakened.

The header documents caller safety preconditions. Arbitrary pointer validity
cannot be checked; callbacks must not throw/unwind or retain borrowed bytes.
The global registry uses monotonic handles; on poison it denies operations
with error 3 for the rest of the process. Panic capture does not establish
transaction rollback or recovery from aborts, bad pointers or foreign unwinding.

## reactivity production-core reactivity fixture

The separate `include/reactivity.h` surface drives production core/memory
`ReactiveDatabase`, preserving the older spike ABI. Its fixed synthetic schema
returns owned complete row/count/error events; it is not a packaged Swift SDK.
See [actual RED/GREEN commands and results](REACTIVE_RED.md). Calls are serial;
callbacks run on the drain caller thread with no registry lock or owner borrow.
Per-event checks suppress canceled/closed handles, callback writes wait for the
next drain, and recursive delivery rejects. Callers copy borrowed payloads during
the callback and never unwind across C. Scheduling remains the host's job.

SPEC-0028 / TC-0031 Q05 adds two checked fixture-only controls:
`hoya_reactive_fail_next_project(handle)` arms exactly one synthetic projected
read failure; `hoya_reactive_refresh(handle)` requests the production owner's
explicit observation retry. The memory adapter is wrapped only for this native
test fixture; ordinary reads, writes and lifecycle operations forward unchanged.
A zero refresh status acknowledges the attempt, while Error callbacks carry
read failures (`kind = 2`, BackendFailure `error_code = 7`). This is not a real
SQLite, disk, quota or corruption fault. The actual Swift scenario checks that
the write committed, error callbacks can reenter reads without a lock, repeated
explicit failures remain observable, row recovery is exact, the count retains
its original sixteen-millisecond deadline, and cancellation/close stop delivery.

Build the static library using the commands above, then run all four production
reactivity scenarios with:

```sh
swiftc -target x86_64-apple-macosx26.5 -parse-as-library -warnings-as-errors -framework Security -framework CoreFoundation -import-objc-header spikes/cross-platform/native/include/reactivity.h spikes/cross-platform/native/tests/reactive.swift spikes/cross-platform/native/target/debug/libhoyadb_native_spike.a -o spikes/cross-platform/native/target/native-reactive
spikes/cross-platform/native/target/native-reactive
```

## command sync command/sync host experiment

The separate `include/command sync.h` RPC surface uses the same std-only fixture as
WASM over the real production `CommandDatabase<SqliteStorage>`. The Swift
reference in `tests/commands.swift` owns async handlers, a shared FIFO dispatcher,
trusted-context capture, callbacks, platform entropy, retry timers and URLSession;
Rust owns staged queries/writes, atomic durable enqueue and the sync state machine.
It preserves tagged i64/Float values without Foundation-decoding opaque envelopes.

See [RED/GREEN evidence and reproducible commands](COMMAND_RED.md) for H01–H06,
including independent SIGKILL writer/readers, explicit task cancellation,
reentrant callback disposal, keyed HTTP outcomes, correction/inverse guards and
migration refusal while recovery remains. The executable creates its own fresh
canonical temp root and passes it at child startup; successful runs remove that
owned root. Run the loopback fixture on port 8774 first. This remains an isolated
reference host, without production SDK/mobile/concurrent-writer guarantees.

entity sync extends that same C ABI and reference host with configured automatic
entity patches, mixed push outcomes, chunked paged pull, progress and explicit
generation reset. See [entity sync reproduction and evidence](ENTITY_RED.md).
The default command executable also runs the four new scenarios; `phase-five`
runs only those scenarios. No native runtime dependency or new ABI is added.

## security supplied-key and backup boundary

API-0010 / TC-0035 adds protected construction over the same production SQLite
owner and C ABI. `Owner(protectedKey:root:name:host:)` requires exactly 32 supplied
bytes, validates the pre-established `HOYADB_PHASE4_ROOT` before changing backup
attributes, applies Foundation's directory exclusion hint and verifies readback
before `open_secure`. The default hint is true; false is explicit opt-in. Failed
policy validation prevents opening. Only closed owners can revalidate their own
replacement, and no import/conversion/rotation or arbitrary path API is added.
The older scenarios explicitly use an unprotected test factory unless the
protected test mode is selected. There is no fallback on a protected-open error.
Application Support is deployment guidance; this synthetic fixture remains
confined to its parent-created canonical temporary root.

The current SQLCipher/CommonCrypto static library requires native clients to
link the existing Apple Security and CoreFoundation frameworks. The earlier
plain `swiftc` command without these flags fails with unresolved
`SecRandomCopyBytes`/`CFBundle` symbols; explicit flags are now included above.
Swift code that imports both frameworks also gets their link directives, but
C/C++ and plain Swift consumers must supply the equivalent flags. No checked-in
shell/CI C-client linker command was found; the Rust CI jobs use Cargo's existing
framework metadata. Linux provider/link instructions belong to the SQLite crate.

With the reviewed current native archive already built, run from the repo root:

```sh
python3 spikes/common/command_http.py --port 8775
```

In a separate terminal:

```sh
swiftc -target x86_64-apple-macosx26.5 -parse-as-library -warnings-as-errors -framework Security -framework CoreFoundation -import-objc-header spikes/cross-platform/native/include/command sync.h spikes/cross-platform/native/tests/commands.swift spikes/cross-platform/native/target/debug/libhoyadb_native_spike.a -o spikes/cross-platform/native/target/native-commands
spikes/cross-platform/native/target/native-commands phase-six-backup
spikes/cross-platform/native/target/native-commands phase-six-security
HOYADB_PHASE5_HTTP=http://127.0.0.1:8775 spikes/cross-platform/native/target/native-commands phase-six-protected
HOYADB_PHASE5_HTTP=http://127.0.0.1:8775 spikes/cross-platform/native/target/native-commands
xcrun swift-format lint --strict spikes/cross-platform/native/tests/commands.swift
```

The supplied key is a fixed external synthetic input in this test executable,
not a library-generated key or a production key-management example. The
`phase-six-protected` runner explicitly chooses the protected factory at child
startup and propagates that mode to independent writer/readers. Successful
runs remove their owned root. Protected cases check real SQLCipher status,
wrong/missing/plaintext-key rejection without file changes, raw store/sidecar
canaries, exact frozen recovery bytes, independent process reopen, and actual
Foundation policy/replacement behavior. All original H/E assertions also run
under the encrypted adapter. See [security evidence](SECURITY_RED.md).

The flag is an OS backup hint; it cannot suppress arbitrary external copies.
Foreign Swift/JSON/allocator copies of key material cannot be guaranteed erased.
This remains a desktop reference host, without new mobile, packaging, hostile
runtime or FIPS claims. Old Swift scenario sources are unchanged; their existing
strict-format findings are recorded separately from the passing current owner
file. No platform SDK behavior is established by these tests.
