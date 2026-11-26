# Native, Swift and React Native public surface snapshots

These are inert baselines for TC-0036 G24 (API compatibility), taken on the
current cut. Producing them involved no builds beyond the existing artifacts
and executed no package code. The Kotlin and JavaScript surfaces belong to
their own lanes.

- C ABI1 ([snapshot.json](snapshot.json)): `bindings/native/include/hoyadb.h`
  (`2f61e513…`, unchanged since the native foundation) declares the four
  `hoya_v1_*` functions. Every Apple static slice (macOS x86_64, iOS arm64,
  universal simulator) exports exactly those four symbols. The module map now
  lives at `include/HoyaNative/module.modulemap` (`33326b17…`). Both Android
  JNI slices (`x86_64` `3b8c7f5a…`, `arm64-v8a` `2ca0467d…`) export exactly
  eight versioned symbols: the four C exports and the four `NativeBridge` JNI
  exports.
- Swift: `xcrun swift-api-digester -dump-sdk` dumps of `HoyaDB` and
  `HoyaDBTesting`, taken from the current `swift test` build modules, with the
  macOS slice headers for `HoyaNative`. The raw JSON is kept, alongside
  readable declaration outlines ([HoyaDB](HoyaDB.api.txt): 400 lines,
  [HoyaDBTesting](HoyaDBTesting.api.txt): 36 lines). Those modules come from
  a debug build compiled with testing enabled. `-dump-sdk` (as opposed to
  `-abi`) lists public declarations.
- React Native: [index.d.ts](react-native-index.d.ts) (`8ea711a2…`). The
  runtime exports are `createNativeRuntime`, `defineSchema`, `j`,
  `openMemoryDatabase`, `openNativeDatabase` and `query`. The Codegen
  TurboModule spec `NativeHoyaDB.ts` has the members `open`, `call`,
  `namespace`, `uptime` and the `onAdmissionReleased` `EventEmitter<void>`.
  The snapshot also records the hashes of `package.json`, the podspec and
  `react-native.config.cjs`.

No earlier published baseline exists to diff against, so these files are the
first baseline for the unpublished 0.1.0 packages. The RN factory option name
changed from `storeOptions` to `options` before this baseline was taken (see
[rn-ios](../phase-seven-rn-ios/README.md)).
