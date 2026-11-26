# React Native iOS consumer (Hermes, simulator)

Partial Phase7 evidence for TC-0036 G23 and the RN rows of API-0011. This is
not Phase7 completion: RN Android has not run yet, and qualifiers apply.

## Result

[run-c](run-c/summary.json) installed and ran build-g's app (debug dylib
build `c870b32a…`) on the task-owned simulator
`CC525A65-D03E-4F9D-8936-DD16BBFD9664` only. The app was uninstalled afterwards
and the simulator's final state is Shutdown.

- `all` mode: [51/51 OK](run-c/all-report.json) in actual Hermes
  (`HermesInternal` defined).
  - 16 native-memory and 31 native-protected copies of the shared
    conformance, walkthrough and migration cases.
  - The original-frame/key/root guards.
  - The RN public memory factory: retained store, Busy for a duplicate open
    and for a drop while open, a failed authored reopen that preserves the
    original, the authored migration, an ephemeral default and an explicit drop.
  - Mixed Swift SDK + RN admission. RN waits while the Swift scope holds
    admission; a foreign finish emits the post-C-ABI release before the
    suspended false reply resumes; the reverse direction holds too.
- `writer` then `reader` as independent processes in the same app container:
  [writer](run-c/writer-report.json) and [reader](run-c/reader-report.json)
  both GREEN. Covered: encrypted profile, durable sent queue, wrong key
  rejected, lossless i64 and Float kind, exact frozen retry and guarded mapping.
- The app posts its result to an owned loopback receiver on
  127.0.0.1:8781. That receiver only records the result. It is not command
  transport, and no mobile push-HTTP case is claimed.

## Inputs and build

- Pod graph: network-denied fresh CP_HOME install (see
  [native packaging](../phase-seven-native-packaging/README.md),
  [offline-d](pod-install-offline-d-command.json)): 76 local pods and the
  three verified prebuilt containers.
- [ios-build.py](ios-build.py.txt) runs these steps:
  1. Metro bundle under `sandbox-exec (deny network*)` with an owned TMPDIR
     and `--reset-cache`.
  2. The pod's hermesc `-emit-binary -O` (HBC magic `c61fbc03c103191f`).
  3. `xcodebuild` Debug for the iphonesimulator x86_64 target.
- `xcodebuild` cannot run under sandbox-exec. SwiftPM applies its own manifest
  sandbox, and the nested `sandbox_apply` is refused
  ([build-b](build-b/xcodebuild.txt)). The SwiftPM sandbox was not disabled.
  Network denial is therefore **not proven** for the compile step or for
  Xcode's XPC build service.
- Single native copy: the debug dylib exports exactly the four `hoya_v1_*`
  symbols once, and no extra HoyaNative framework is embedded
  ([build-g](build-g/build.json)).
- Approved npm source differences are empty for every build.

## TDD chronology (all logs retained)

| Step | Outcome |
| ---- | ------- |
| build-a | Metro could not resolve `@babel/runtime`: approved packages are symlinked and Metro crawls only the project. The example `metro.config.cjs` now watches the real roots of linked node_modules entries; an ordinary install adds nothing. |
| build-b | Metro and hermesc GREEN under the sandbox; xcodebuild failed on the nested sandbox (above). |
| build-c | Packaging RED: `redefinition of module 'HoyaNative'`. Fixed by the module map layout (native packaging README). |
| build-d | Consumer harness: `HoyaForeignControl` missing from the generated `-Swift.h`. With these flags only public `@objc` declarations are emitted, confirmed with a swiftc probe. The example class and its four statics are now public. |
| build-e, run-a | 47/51. The retained factory failed (expected RED). Both G19 cases failed with `future retry deadline`. The mixed case failed with `undefined is not a function`. |
| build-f, run-b | `retained.mjs` extended first (Busy, drop, failed migration, ephemeral, explicit drop). Genuine RED: `duplicate active open of the selected store: missing Busy`. Error stacks located the mixed failure: the test called `scope.query(...).fetch()`, but the Scope API is `scope.fetch(query)`. |
| build-g, run-c | 51/51, writer and reader GREEN. |

Root causes fixed in `bindings/react-native`:

- `openMemoryDatabase` hard-coded `store:null`. It now forwards `store`; the
  default null is a new ephemeral store.
- Both factories took store-open options as `storeOptions`, whereas the shared
  `OpenOptions`, `@hoyadb/testing` and the copied conformance use `options`.
  The RN factories silently dropped `options` (G19 retry delays), so they now
  forward `options`.
- `index.d.ts` keeps `store?: Handle|null` and declares
  `options?: OpenOptions['options']`.
- The consumer test `mixed.mjs` now uses `scope.fetch(db.query(...))`.

## Final source and artifact

After run-c, only `bindings/react-native/README.md` changed: it now documents
`store` and `options`. The final repack is `6a2ca07b…`
([final-repack.json](final-repack.json)). It differs from run-c's tgz
`1ac8d113…`, which is preserved under `target/phase-seven-rn/final-artifacts/`,
in exactly one member, `package/README.md`. All 16 packed non-artifact files
equal the current source, and the packed XCFramework equals the recorded
SwiftPM artifact ([final-rn-artifact.json](final-rn-artifact.json)). The app
code that ran is therefore byte-identical to the final package.

## Qualifiers

- The x86_64 simulator only. No physical device, and the arm64 simulator and
  arm64 device slices are build-only.
- Debug configuration with the approved debug prebuilt RN/Hermes containers.
  A release build was not exercised.
- Declarations were reviewed but not typechecked: no TypeScript exists in the
  approved graph.
- The example `ForeignControl.swift` has 14 swift-format findings, the same
  set as before this change. It is outside the Swift strict gate.
