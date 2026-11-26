# reactivity native boundary — observed RED

SPEC-0027/API-0005/TC-0029 preparation was reviewed and committed in `f7f6002`
before this test bridge. The initial `reactive.rs` scaffold exported all C
symbols with the reviewed signatures, but `hoya_reactive_open()` returned zero.
No production ReactiveDatabase implementation was present in that bridge.

The actual Swift fixture compiled and linked successfully using these commands
from the repository root:

```sh
cargo build --manifest-path spikes/cross-platform/native/Cargo.toml --offline --locked
swiftc -target x86_64-apple-macosx26.5 -parse-as-library -warnings-as-errors -import-objc-header spikes/cross-platform/native/include/reactivity.h spikes/cross-platform/native/tests/reactive.swift spikes/cross-platform/native/target/debug/libhoyadb_native_spike.a -o spikes/cross-platform/native/target/native-reactive
spikes/cross-platform/native/target/native-reactive
```

The executable exited **1**, with these expected missing-behavior failures:

```text
FAIL R14A owned rows/count/columns: check("R14A production database opens")
FAIL R14B reentrant callback lifecycle: check("R14B production database opens")
FAIL R14C/R15 stale owners and timing: check("R14C production database opens")
Swift reactivity runtime: Version 26.7.1 (Build 25G241)
```

The failure occurred at the first behavior assertion of each scenario, not at
compilation, linking, dependency lookup or runtime startup. Subsequent assertions
cover owned complete rows and i64 extrema, count throttling, tracked/untracked
edits, callback re-entry/read/write, recursive-drain rejection, per-event
unsubscribe/close suppression, stale owner/token rejection and repeatable
drain-caller thread/timing observations. The bridge is a fixed synthetic fixture
over production core/memory paths, not a packaged Swift API or mobile proof.

## Observed GREEN after implementation

The same build/link/run commands now exit zero against the real production
`ReactiveDatabase<MemoryStorage>`, with all three Swift scenarios passing.
Swift copies every application field during callbacks and checks borrowed bytes
again after reentrant writes or close, proving the extracted snapshot outlives
the original registry owner through callback return. The fixed fixture uses
String/String/i64 application fields; it does not expose a general schema SDK.

```text
PASS R14A owned rows/count/columns
PASS R14B reentrant callback lifecycle
R15 native insertion-to-callback samples ns=[203060, 217451, 251883, 248235, 264354, 374069, 339913, 324091, 421259, 359805, 365353, 451542], drainCallerThread=74431824, cadence=explicit immediate drain; measured to callback entry
PASS R14C/R15 stale owners and timing
Swift reactivity runtime: Version 26.7.1 (Build 25G241)
```

Timing includes synchronous insertion, observation maintenance and the explicit
immediate drain up to Swift callback entry; it excludes subsequent Swift field
copying and callback re-query. These 12 synthetic increasing-result-size samples
are observations, not averages, latency budgets or comparative benchmarks.
Each callback's OS thread equals its drain caller's thread; no automatic UI
dispatch or persistent callback scheduling is claimed. Core receives explicit
monotonic samples for the deterministic count-throttle checks.

Observed toolchains: Rust `1.98.1 (48a229cea 2026-09-01)` and Apple Swift
`6.3.2 (swiftlang-6.3.2.1.108 clang-2100.1.1.101)`. The Swift compiler default
target is macOS 26.0, so commands explicitly select 26.5 to match the existing
bundled SQLite static library deployment floor.

Existing native regressions also passed: eight Rust tests; five old Swift
N01–N05 cases; three old Swift N07–N09 cases; separate persistence writer/reader
processes with exact restored Unicode/i64-extreme rows. Native formatting and
Clippy with `-D warnings` passed. `Cargo.lock` adds only the two local core and
memory packages; external versions/checksums remain unchanged. No new runtime
dependency, commit, remote CI run, mobile execution or publication is inferred.

## SPEC-0028 / TC-0031 Q05 — typed native error delivery RED

Reviewed preparation `671daa2` preceded the fault-control fixture and additional
Swift case. Both new exported controls initially acknowledged a request without
arming a fault or refreshing observations. The new actual Swift executable
compiled and linked, retained all three passing prior scenarios, and exited 1:

```text
PASS R14A owned rows/count/columns
PASS R14B reentrant callback lifecycle
PASS R14C/R15 stale owners and timing
FAIL Q05 projected error delivery/recovery: check("Q05 typed projected errors reach actual Swift callbacks")
Swift reactivity runtime: Version 26.7.1 (Build 25G241)
```

The full observed output, including the unchanged timing scenario, was retained
at ignored `target/phase-three-native-q05-red.txt`. The same build/link/run
commands above were used. This is missing typed-error delivery behavior rather
than a compile/link failure. The intended Q05 test uses a per-owner one-shot
projected-read fault only; it distinguishes this synthetic fault from real SQL
or filesystem failures and inspects the committed row through an actual
reentrant Swift callback.

After implementing the per-owner fault wrapper, the initial typed error and
committed-read callback assertions passed. The same executable next failed
`Q05 every failed explicit refresh delivers typed errors`, reproducing the
core's repeated-explicit-refresh suppression through the real native boundary.
The shared core correction was then applied separately from this test fixture.

## SPEC-0028 / TC-0031 Q05 — observed GREEN

Rebuilt the native static library against the corrected production core, then
recompiled and ran the actual Swift executable using the commands above. All
four scenarios exited zero:

```text
PASS R14A owned rows/count/columns
PASS R14B reentrant callback lifecycle
R15 native insertion-to-callback samples ns=[228197, 245850, 262018, 312814, 346915, 344893, 328158, 342538, 344548, 365086, 382885, 477147], drainCallerThread=74899327, cadence=explicit immediate drain; measured to callback entry
PASS R14C/R15 stale owners and timing
PASS Q05 projected error delivery/recovery
Swift reactivity runtime: Version 26.7.1 (Build 25G241)
```

Full output is retained at ignored `target/phase-three-native-q05-green.txt`.
Q05 delivers BackendFailure (`kind=2`, `error_code=7`) to both affected
registrations after a successful write, then again for each explicitly rearmed
failed refresh at 102 and 103 ms. Error callbacks reenter ordinary reads of
the exact committed Unicode/i64-extreme row on the drain caller's thread.
Successful refresh publishes the complete owned row at 104 ms while the count
waits until the original 116 ms deadline. A canceled row error is suppressed;
the remaining count-error callback rejects recursive drain, closes the owner
and causes subsequent fault/refresh/drain controls to reject the stale handle.

After this correction, eight native Rust tests, native format/Clippy checks,
the five old Swift N01–N05 cases, three old Swift N07–N09 cases, and separately
launched persistence writer/reader processes all passed again. No external
runtime dependency, physical storage fault, mobile execution or release claim
is introduced by the synthetic wrapper.
