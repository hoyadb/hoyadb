# Native packaging: uniform XCFramework basename and hermetic Pod install

Partial Phase7 evidence for TC-0036 G23 packaging (Swift/RN lane). This is not
Phase7 completion: no RN app has been compiled or run from this cut yet.

## XCFramework slice basename (packager correction)

CocoaPods rejected the vendored static XCFramework because slice binary names
differed ([attempt 2](pod-install-2-basename-failure.txt):
`libhoyadb_native` vs `libhoyadb_native_simulator`). SwiftPM requires the `lib`
prefix, so every slice now uses `libhoyadb_native.a`.

1. New read-only packaging test `bindings/swift/check_artifacts.py` (claimed
   slices only, one basename per slice, headers/module map, recorded hashes).
   Genuine RED against the existing artifact and the RN copy fails only the
   basename assertion: [red-swift.txt](red-swift.txt), [red-rn.txt](red-rn.txt).
2. Minimal change in `bindings/swift/build-artifacts.py`: lipo writes the
   universal simulator archive to a separate staging directory
   (`target/phase-seven-native/xcframework-simulator/libhoyadb_native.a`) and
   `xcodebuild -create-xcframework` packages it normally. No archive bytes were
   renamed or edited by hand; no ABI or source change.
3. [Rebuild](build.txt): all four cargo builds were already fresh (no
   recompilation). The 60 recorded source hashes are identical
   ([before](native-build-before.json), [after](native-build-after.json)).
   Library bytes are identical: macOS `5aba91a0…`, iOS arm64 `64d6fd9e…`,
   simulator `bf491fe3…` (renamed only). Only `Info.plist` changed
   ([before](Info-before.plist), [after](Info-after.plist)).
   [GREEN](green-swift.txt) 4/4.
4. Affected Swift checks rerun on this artifact: [44 tests](swift-test.txt)
   with warnings as errors passed; [strict format](strict-format.txt) including
   Package.swift returned 0 with empty output; the outside-checkout macOS
   consumer [built](macos-build.txt) and its independent writer/reader both
   exited 0 ([results](macos-results.json)).
5. RN: only the generated `bindings/react-native/Artifacts` copy was replaced
   and repacked with `npm pack --ignore-scripts`; the owned unpacked package was
   replaced. All three copies pass the check and match the recorded hashes
   ([rn-repack.json](rn-repack.json), `rn-check-*.txt`).

## CocoaPods attempts (chronology preserved)

| # | Result | Evidence |
| - | ------ | -------- |
| 1 | podspec filename mismatch (pre-handoff) | target/phase-seven-rn/first-podspec-name-failure-* |
| 2 | basename rejection; printed `Adding spec repo trunk with CDN` | [log](pod-install-2-basename-failure.txt) |
| 3 | exit0, 76 pods, reused attempt-2 CP_HOME | [log](pod-install-3-green.txt), [graph](pod-graph-review.json) |
| 4 | network denied, fresh CP_HOME: genuine RED on implicit trunk clone | [log](pod-install-4-network-denied-failure.txt), [command](pod-install-4-command.json) |
| 5 | exit0 but harness layout error (see below) | [log](pod-install-5-wrong-codegen-layout.txt), [comparison](pod-install-5-graph-comparison.json) |
| 6 | network denied, fresh CP_HOME/CP_CACHE: GREEN | [log](pod-install-6-network-denied-green.txt), [command](pod-install-6-command.json), [comparison](pod-install-6-graph-comparison.json) |

Attempts 2 and 3 are not demonstrated offline. Attempt 2 contacted the CocoaPods
CDN for version metadata only (`.url`, `CocoaPods-version.yml` and its etag;
empty `Specs`); attempt 3 reused that home. Separately verified: no SPEC REPOS
section, no spec or pod bytes from any registry, and the three prebuilt
containers came from file:// archives whose SHA256 was rechecked.

Attempt 4 cause, from installed source: cocoapods-1.16.2
`lib/cocoapods/installer/analyzer.rb:170-173` adds `TRUNK_REPO_URL` whenever the
Podfile declares no source, even if all 77 dependencies are local path/podspec.
Correction (root-approved, documented first in
`bindings/react-native/ACTIVATION.md`): the example Podfile declares
`source ENV['HOYA_POD_SPEC_SOURCE'] if ENV['HOYA_POD_SPEC_SOURCE']`. Leaving it
unset keeps CocoaPods' default. Hermetic runs point it at a task-owned local Git
fixture with only a README and an empty `Specs` placeholder
([identity](spec-source-fixture.txt): commit `ece3c227…`, tree `62c8fec4…`).
It contains no podspec and no CDN metadata, was created without hooks or global
Git configuration, and CocoaPods resolves no spec from it.

Attempt 5 is a harness error, not an SDK or CocoaPods defect. My preserved
`ios/Pods-before-offline-*` trees contained `Pods.xcodeproj`. RN Codegen's
`generateReactCodegenPodspec.js` locates the app with `find <app> -name
"*.xcodeproj"` and excludes only `/Pods/`, so it chose a preserved tree and
generated `${PODS_ROOT}/../../../src/...` inputs. Preserved trees now live
outside the consumer root (`target/phase-seven-rn/preserved-pods-root.txt`), and
the script asserts the app project is the only non-node_modules xcodeproj.

Attempt 6 used [this script](install-pods-offline.py.txt) (the RED run's script
is [preserved](install-pods-offline-red-as-run.py.txt)). Results:

- Pod and all its children ran under
  `sandbox-exec -p '(version 1)(allow default)(deny network*)'`. The man page
  marks `sandbox-exec` deprecated; it is present and works on Darwin 25.6.
- An in-profile loopback connect probe returned `Errno::EPERM`; unsandboxed it
  returns ECONNREFUSED. That demonstrates socket denial with no remote contact.
  DNS resolution over system IPC (mDNSResponder) is **not** proven denied.
- Fresh CP_HOME contains only the local fixture clone (same HEAD and files) and
  no trunk. The cache holds only the three file:// prebuilt pods.
- Approved npm source differences: none. The loaded Ruby tool source hashes
  equal the reviewed closure.
- Compared with reviewed attempt 3, Podfile.lock differs only in
  `PODFILE CHECKSUM` (the Podfile changed). All 76 local podspec JSONs are
  byte-identical, and the three extracted prebuilt trees are identical
  (4791 + 3528 + 68 files).

`CLANG_CXX_LANGUAGE_STANDARD` warning: the example project sets `c++20`. That
matches all 128 generated Pods xcconfig values and RN 0.86.3's
`Helpers::Constants.cxx_language_standard`, so the setting was left unchanged.

## HoyaNative module map layout (mixed SwiftPM + CocoaPods app)

The first RN iOS Xcode compile failed with `redefinition of module 'HoyaNative'`
([errors](../phase-seven-rn-ios/build-c/xcodebuild-errors.txt)). In an app that
uses the Swift SDK and the RN pod together, the framework headers are visible
twice: SwiftPM copies them to `include/`, and CocoaPods adds
`XCFrameworkIntermediates/HoyaDBReactNative/Headers` to the app's header search
paths. With a top-level `module.modulemap`, clang loads both copies.

1. `check_artifacts.py` gained two tests. One checks the header layout. The
   other copies the slice headers twice and typechecks
   `import HoyaNative` with both copies on the search path. That
   reproduces the exact error: [RED](modulemap-red.txt).
2. The fix moves `bindings/native/include/module.modulemap` to
   `include/HoyaNative/module.modulemap`, which declares `header "../hoyadb.h"`.
   `#include <hoyadb.h>` and `import HoyaNative` keep working, and clang stops
   at the first `HoyaNative/module.modulemap` it finds. `build-artifacts.py`
   now lists header files recursively.
3. The rebuild recompiled no Rust. Library bytes and `hoyadb.h` are identical.
   Info.plist differs only in the order of `AvailableLibraries`
   ([before](native-build-before-modulemap.json), [after](native-build-after.json)).
   [GREEN](modulemap-green.txt): 5/5.
4. Swift checks were rerun:
   - [44 tests](modulemap-swift-test.txt) passed with warnings as errors;
   - [strict format](modulemap-strict-format.txt) is empty;
   - the outside consumer was built in a fresh scratch directory; on macOS
     the independent writer and reader exited 0, and the iOS build compiled
     ([results](modulemap-consumer-results.json)).

## React Native package configuration

Both problems were found by running the official `react-native config` under
the outbound-denied sandbox.

- `dependency.platforms.ios.podspecPath` is not a valid key in a package
  config. The CLI finds `HoyaDBReactNative.podspec` by itself, so the key is
  now `ios:{}` and the warning is gone.
- Android autolinking defect: `sourceDir` was the absolute
  `__dirname+'/android'`. The CLI computes `path.join(root, sourceDir)`, so the
  dependency resolved to `android: null` and would have been silently dropped
  ([RED](rn-config-red-android-null.json)). With `'android'`, the CLI resolves
  the source directory, package import/instance and `libraryName` `HoyaDBSpec`
  ([GREEN](rn-config-green.json)).

The final RN package is described in [rn-repack-final.json](rn-repack-final.json).
The RN iOS runtime is documented in [phase-seven-rn-ios](../phase-seven-rn-ios/README.md).
