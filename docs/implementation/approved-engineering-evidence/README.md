# Approved engineering evidence — 2026-10-31

This directory retains evidence for SPEC-0047's approved implementation work.
It does not replace the historical Phase Seven snapshots or certify publication,
legal compliance or execution on every built platform.

The earlier Apple files listed under **Apple provider evidence refresh** below
bind the frozen conflict-policy checkpoint, not the later runtime source. A
source inventory comparison on 2026-10-31 found that
`crates/hoyadb-bindings/src/engine.rs` changed after this build: the retained
parent records SHA256
`302857e0443cb5d585d11bcdd397fba36f5e7cffccedd3054d7d8915e7f77eaa`,
while the checked later source had SHA256
`c66c90490012b92aff0636a3f3f0233b95b42887b22dade4ecd5e5f32752dd4d`.
The other 66 recorded inputs matched at that comparison. Further changes may
follow; rebuild and repeat the relevant runtime and package checks after the
next source freeze. Keep the retained parent and derivation unchanged.

## Apple provider evidence refresh

The ordinary `bindings/swift/build-artifacts.py` compiled macOS x86_64, iOS arm64
and both simulator architectures against one frozen source inventory. It created
the XCFramework and a new compilation parent, then correctly refused notice
finalization with `stale-provider-evidence`: the previous provider proof referred
to older archive bytes. The source index requires actual new evidence, not a
replacement hash copied from an older certificate.

The retained files are:

- [Previous source index](apple-notice-index-before-20261031.json): exact bytes
  before the refresh, SHA256
  `65a591b52a3a632526c1b1f930980cc8a7f2bd561af020b7e6e8fb19eb72bde6`.
- [New compilation parent](apple-native-build-parent-20261031.json): exact
  `bindings/swift/Artifacts/native-build.json` from this build, SHA256
  `2fbf43373ba0d0cd03ce5da068d3d0c6f4cff89f045c1d9f218201977ceb21a3`.
  It records 67 source inputs, ten native artifact members and actual build
  commands. Inputs matched before and after compilation.
- [Provider derivation](apple-provider-refresh-20261031.json): actual per-library
  and per-architecture hashes, tool identity/version, exact inspection commands,
  output hashes, imports and rejected old-index result. Raw command captures
  remain in `/tmp/hoyadb-followup-20261031/apple-provider-symbols/`.
- [Capture script](refresh-apple-provider-evidence-20261031.py.txt): the exact
  one-off derivation source, SHA256
  `4219340cd6cd4ae6299572686723334a5b5af0d43df32dae038139cfa83cae98`.
  It is an evidence capture with explicit local paths, not a portable build tool.

The compatible Rust LLVM 22 inspector read each thin architecture, including
both halves of the fat simulator archive. Every architecture imported the same
nine CommonCrypto and two Security symbols, defined none of those provider
symbols, and contained no matches for the two recorded LibTomCrypt symbol
patterns. All 76 retained Rust toolchain link-input hashes still matched. The
crypto-provider role and dependency/license texts are unchanged; only the
current provider archive bindings and derivation reference in
[the source index](../../../crates/hoyadb-native/third-party/index.json) changed.
No notice validator, export checker or historical API snapshot was weakened.

The unmodified notice finalizer then accepted the fresh current index and the
new compilation parent: 10 native members, 28 notice members and one copied
compilation parent (39 members, excluding `packaging.json`). Its packaging
record SHA256 is
`ad222db4d0e3f71febbaa99e7e21550478dfcc3418a9c101a54caa7155657e1a`.

The macOS archive SHA256 is
`b92e3f272c9d7d9d300b929dea395e95f328699c1fd00c66d386cf7920220fa1`,
exactly the archive used by the passing 102-test Swift suite at that policy
checkpoint in its separately pinned host-only stage. The iOS libraries are build
evidence; this capture does not claim iOS execution, arm64 macOS support or remote
packaging.
Later source changes require fresh corresponding runtime/package evidence.

## R9/R10 Apple checkpoint — 2026-10-31

The later rebuild binds the frozen R9/R10 runtime, including the two diagnostic
helper cfg attributes, through 72 native source inputs. The independent capture
also checked 45 Swift SDK inputs, 44 verification inputs, 2,508 external package
files, 76 Rust toolchain link inputs, ten tool binaries and 7,340 Apple SDK link
stub/settings files. These inputs matched before and after the checks. The only
change among the 33 captured packaging inputs was the reviewed provider index.
These counts describe the recorded inventory, not a claim that every installed
toolchain file was captured.

The first ordinary build compiled all four architectures, created the framework,
and refused the old provider proof with `stale-provider-evidence`. Fresh symbol
inspection checked every thin architecture: nine CommonCrypto and two Security
imports, no definitions of those provider symbols, and no matches for the two
recorded LibTomCrypt patterns. The unchanged finalizer then passed. A warm run of
the ordinary builder also passed after that refresh.

Retained evidence:

- [Prior index](apple-notice-index-before-r9-r10-20261031.json), exact SHA256
  `f401663e85815725b3a3719355d1195a8ee719c662e48f05c63f597302114620`.
- [First compilation parent](apple-native-build-parent-r9-r10-20261031.json),
  SHA256 `57baa8be218aff2f1f1e2c688972b8d1d8a2653585e92be3be6caba528bc3690`.
- [Provider derivation](apple-provider-refresh-r9-r10-20261031.json) and
  [exact capture script](refresh-apple-provider-evidence-r9-r10-20261031.py.txt).
  The current index points to the immutable retained first parent. The script
  records its original live capture path; the later pointer correction and both
  index hashes are retained in the verification evidence.
- [Final compilation parent](apple-native-build-parent-r9-r10-final-20261031.json),
  SHA256 `f6b92990642e5ce21daf0a048db5c17f43236ea571b22b54167b1a8332cc228e`.
- [Verification record](apple-r9-r10-verification-20261031.json), with commands,
  statuses, source and artifact hashes, first/final package linkage and limits.
  Full captures and raw logs remain in `/tmp/hoyadb-apple-r9-r10-final/`.

The warm builder reordered `Info.plist`'s `AvailableLibraries` array. It changed
the compilation-parent and packaging hashes while preserving all native archive
and header bytes. The final index additionally points to the immutable provider
parent. The finalizer intentionally refused to rewrite a framework already
carrying notices, so its ten source-bound native members were copied to a fresh
stage, finalized, and installed as generated output; the earlier packages remain
retained. The final five artifact checks passed against the final parent above.
The final packaging SHA256 is
`0432e12a075fe00b74a7051cb12375ad42d48345f22688baa8d4d0fc35662fef`.

The first checked package passed all 102 Swift tests with warnings as errors and
the isolated public consumer's independent writer/reader processes. Its macOS
archive SHA256, identical in the final package, is
`4e753c24ef8e666c94ad98ae3b3d0c4c0a4482c9d095f7dc4c1e8931cf9b80b0`.
Native checks passed nine tests, formatting and Clippy with warnings denied.
Fresh Swift API comparison, all three Apple export comparisons, source API
validation, 22 release-tool tests and strict Swift formatting passed. The fresh
native dylib passed the protected command-only downgrade probe: the old reader
opened the control, rejected discarded history without changing bytes, and the
new reader reopened the retained row/state.

Restricted SwiftPM cache and consumer backup-policy attempts failed; normal host
access passed without weakening SwiftPM sandboxing or the backup checks. Those
failed attempts are retained. Xcodebuild emitted CoreSimulator connection/log
diagnostics but returned success for framework creation. Runtime evidence is
macOS x86_64 only; iOS arm64 and both simulator architectures have build/export
evidence. This does not certify iOS execution, arm64 macOS, remote packaging,
publication, the pending R11 converter or later source changes.


## Monstera local binding checkpoint — 2026-11-07

The fresh build compiled all four configured Apple Rust targets and created the
XCFramework from 78 native source inputs. The compilation parent and the
independent 139-input native inventory still matched current source after the
build. The ordinary finalizer correctly rejected the previous provider archive
hashes before this evidence refresh.

Retained evidence:

- [Prior index](apple-notice-index-before-monstera-20261107.json).
- [Compilation parent](apple-native-build-parent-monstera-20261107.json), SHA256
  `ff37cd45ac42a2d62872a23fcb1f52aa608635f17fe2618562e102e5e1e027b4`.
- [Provider derivation](apple-provider-refresh-monstera-20261107.json) and
  [capture script](refresh-apple-provider-evidence-monstera-20261107.py.txt).

LLVM 22.1.8 inspected every archive architecture. All four imported the same nine
CommonCrypto and two Security symbols, defined none of these provider symbols,
and contained no matches for the prior LibTomCrypt patterns or the 92 symbols
from the freshly built approved Android provider. All 76 retained Rust toolchain
link-input hashes matched. Raw command output and the unchanged source inventory
remain under `target/monstera-native-20261107/`.

Only the six current provider archive hashes and their retained evidence
references changed in the source index. Provider roles, dependency/license
texts, validators and earlier records remain unchanged. This checkpoint records
local build/provider evidence; it does not establish iOS execution or full app
migration parity.

## Additive Android32 checkpoint — 2026-11-08

The Android provider and JNI recipes now accept `armeabi-v7a` and `x86` in
addition to their previous targets. The AAR packager selects them only through
explicit `--abis`; its default remains `x86_64 arm64-v8a`. ELF32 class/machine
validation is additive, with the same eight exports, no-undefined link,
dependency allowlist, 16KiB alignment and current-input checks. The compiler
mapping and packaging regression tests failed before implementation and passed
after it; all 61 Android packaging/tool tests passed.

Retained evidence:

- [Exact prior Android index](android-notice-index-before-32bit-20261108.json).
- [Actual link-input derivation](android32-link-inputs-20261108.json) and
  [capture script](capture-android32-link-inputs-20261108.py.txt).
- [Verification record](android32-verification-20261108.json), including the
  140-file before/after source inventory, build commands, full package
  certificate, probe source/artifact hashes, actual runtime results and limits.

The local `stable` toolchain directory contained both required 32-bit standard
libraries. Its compiler, Cargo and linker bytes matched the pinned Rust 1.98.1
toolchain exactly. No toolchain or dependency was downloaded. Archive member
matching identified 19 linked Rust standard-library archives per ABI; exact NDK
driver plans identified four startup/runtime inputs per ABI. Independent review
verified these files and allowed only the corresponding 38 Rust and eight NDK
entries to be appended to the current Android notice index. Existing notice
texts, dependency roles and historical records remain unchanged.

The final local repository is `target/monstera-android32-20261107/maven-final`.
Its four-ABI AAR SHA256 is
`b94dc03435418a85f327eb467f7fff67d1e2c1bffa76828f71621e50035a03af`, and its
certificate SHA256 is
`3a88f637b0445b984236735a37f7c493d7393cc0718f234b223f0b291244819f`.
The final AAR is byte-identical to the tested AAR. The previously accepted
two-ABI repository remains unchanged. Its 64-bit JNI slices were reused after
checking unchanged native source and recorded inputs; both new 32-bit providers,
Rust archives and JNI libraries were built fresh.

Each new ABI passed five actual JNI instrumentation phases: seed, parent
mapping, child mapping, final restart/forget and plaintext SQLite coexistence.
The tests verified encrypted headers, wrong-key rejection without changing
primary database bytes, pending writes across process restarts, partial patch,
atomic marker rollback, the maximum signed 64-bit integer, canonical references
and durable aliases after receipt forgetting. Every phase used a distinct PID
in a 32-bit process. The task-owned API30 emulator reported 4096-byte pages;
x86 executed directly and ARM32 through `libndk_translation.so`. The unique apps
and AVD were removed. No production application store or key was used.

This is narrow local runtime evidence, not physical ARM, 16KiB runtime, full
Monstera/Kotlin/RN parity, publication or legal certification. The existing
16KiB consumer gate remains unchanged. Raw build and runtime logs remain under
`target/monstera-android32-20261107/`; later source changes require fresh checks.
