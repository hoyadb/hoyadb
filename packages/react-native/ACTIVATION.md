# platform SDK restricted React Native consumer activation

Authority: ADR-0035 approval1a914cd; API-0011/TC-0036 and addendumdfe6f92.
No new dependency identity is approved by this plan. The installed tools are
recorded before execution in target/phase-seven-native/rn-ruby-tool-closure.txt
(Ruby, CocoaPods1.16.2 and xcodeproj, including loaded gem paths/versions), with
command binaries and loaded Ruby source hashes captured in the run manifest.

## Inputs and confinement

Use only an independently verified, scripts-disabled dependency installation.
Create a task-owned external consumer and record its canonical path before writes.
Copy the approved manifest/lock, adding only locally packed HoyaDB/client artifacts.
Link approved top-level dependency entries; commands must not modify their targets,
whose source hashes are checked before and after. Keep a separate local `@hoyadb`
directory for `npm pack --ignore-scripts` output. Existing caches do not authorize
a new registry install or dependency activation. Bind the packed native artifact
and complete source inventory to the consumer's current verification record.

Use installed Ruby xcodeproj to generate a minimal owned iOS app project and the
official installed React Native Pod helper/autolinking/Codegen. No dependency
hook is enabled merely to install packages. Exact Codegen commands already run:
node <approved-codegen>/lib/cli/combine/combine-js-to-schema-cli.js <owned-schema> packages/react-native/src/NativeHoyaDB.ts
node <approved-react-native>/scripts/generate-specs-cli.js --platform ios|android
  --schemaPath <owned-schema> --outputDir <owned-generated> --libraryName HoyaDBSpec
  --javaPackageName dev.hoyadb --libraryType modules
Both outputs use the supported void admission-release event.

## iOS Pod installation and build

Run from the external owned ios directory with isolated CP_HOME_DIR and CP_CACHE_DIR
(the supported CocoaPods1.16.2 configuration variables); retain actual command/environment and no shared build
cache. Each local tarball must match both approved SHA256 and SHA512 before use.
Use all of these exact variables (verified in installed official source):

- RCT_USE_PREBUILT_RNCORE=1
- RCT_TESTONLY_RNCORE_TARBALL_PATH=<approved reactnative-core-debug.tar.gz>
- RCT_USE_RN_DEP=1
- RCT_USE_LOCAL_RN_DEP=<approved reactnative-dependencies-debug.tar.gz>
- HERMES_ENGINE_TARBALL_PATH=<approved hermes-ios-debug.tar.gz>
- RCT_BUILD_HERMES_FROM_SOURCE=false
- RCT_SYMBOLICATE_PREBUILT_FRAMEWORKS=0
- RCT_NEW_ARCH_ENABLED=1
- RCT_NO_LAUNCH_PACKAGER=1
- COCOAPODS_DISABLE_STATS=true

Before Pod execution, reject inherited source/version overrides including
REACT_NATIVE_OVERRIDE_HERMES_DIR, HERMES_COMMIT, RCT_TESTONLY_RNCORE_VERSION and
RCT_DEPS_VERSION; explicitly require RCT_HERMES_V1_ENABLED=1. The checked Hermes
selector prioritizes the local-source override ahead of the local tarball, so
merely setting the approved tarball variable does not close that path. Record
these checks and the installed CocoaPods plugin/tool closure in the run manifest;
stop on any unexpected plugin or source. Leave HOME and global configuration
unchanged; only the verified CP_HOME_DIR/CP_CACHE_DIR isolate CocoaPods state.

Sources: react-native/scripts/cocoapods/{rncore,rndependencies}.rb and
react-native/sdks/hermes-engine/hermes-utils.rb in the verified installation.
Before invoking pod install, require all three existing paths and hashes; inspect
these helper branches to confirm local tarball selection and no source fallback.
Use the official template use_native_modules!, use_react_native! with Hermes,
and react_native_post_install(...,ccache_enabled:false). Install with existing
pod install --no-repo-update, preserving full output and Podfile.lock. Accept only
local reviewed React Native podspecs, the local HoyaDB pod, and those three exact
prebuilt containers. Compare every actual pod name/version/source/checksum with
the approved static podspec inventory and prebuilt metadata before compilation.
Stop on any extra external source, fallback-to-source message, unexpected pod
version, install hook requiring an unapproved package, or dependency mismatch;
report it rather than updating the graph silently. No source provider download
is allowed. An isolated empty cache prevents accidental hidden substitution.

Hermetic offline install: CocoaPods1.16.2 adds the public trunk CDN source
whenever the Podfile declares no `source` (installer/analyzer.rb), even when
every dependency is a local path/podspec. A network-denied fresh CP_HOME_DIR
attempt failed on exactly that clone; it and the earlier metadata-only CDN
contact are retained. The example Podfile therefore uses an explicit spec
source from `HOYA_POD_SPEC_SOURCE` when set; unset keeps CocoaPods' ordinary
default. Hermetic runs set it to a task-owned local Git repository containing
only a README and an empty Specs directory (Git placeholder file only), so no
spec is resolved from it and no registry metadata is fetched or fabricated.
Run pod under `sandbox-exec` with `(deny network*)` and require an in-profile
loopback connect probe to fail with EPERM first; DNS resolution over system
IPC is not proven denied by that profile. Acceptance: fresh CP_HOME_DIR and
CP_CACHE_DIR, exit0, zero approved source differences, CP_HOME containing only
the local source clone, and the same pod names/versions/sources/checksums and
archive hashes as the reviewed graph.

Official Metro bundling, approved Hermes bytecode compilation and Xcode build
execute only after the verified graph check. Keep actual commands, hashes and
source maps. Build an ordinary app for the task-owned simulator only; run general
schema CRUD/observation, suspended handlers and mixed Swift/RN release, entity
pull/merge/reset, protected persistence/restart, wrong/missing key and backup
profile cases through the public package. A launch/build alone is not conformance.
Uninstall the owned app and shut down only the recorded task-owned simulator.

## Android

Use the same packed public JS package and approved React Native/Hermes runtime,
plus the final local Maven AAR supplied by the Kotlin lane (preliminary AARs are
build-development inputs only). Use the existing approved NDK27.1.12297006,
installed Gradle9.3.1/JDK17 and exact reviewed AGP/Kotlin build graph. Remote/shared
Gradle caches are disabled. Inspect resolved runtime/build coordinates and hashes
against the approval packet before execution; retain actual locks. The RN module
uses HoyaNativeTransport from that one AAR, including its shared post-native
release broker; never link a second JNI runtime or bypass the broker. Run the
ordinary Hermes consumer on the recorded owned16KiB emulator, coordinating it
with the Kotlin lane, then clean only owned application data/processes.

Acceptance is recorded per actual platform/scenario. No simulator result implies
physical arm64-device support, and no preliminary artifact is a completed SDK.
