# Final manual AAR certificate acceptance

Source preparation only: the development packager has not yet implemented these
final-certificate checks, and these cases have not run during the benchmark quiet
window. No final AAR, Maven consumer, licensing or native-runtime pass is claimed.
The approved API-0011/TC-0036 packaging gate and Android PLAN determine the scope;
this adds neither a dependency nor a new public SDK operation.

1. **P01 — current native artifact binding.** For both required ABI slices,
   compare the current SO SHA256 with the matching native evidence's
   `artifact_sha256`; verify its ABI/minSdk and every `inputs_sha256` file against
   current bytes. Missing or changed C/header/export-map/Rust-archive inputs,
   changed SO, malformed evidence, unexpected ABI or symlink rejects before
   creating a final artifact. Recording an evidence JSON hash alone is insufficient.
   The archive digest does not show which Rust sources it was compiled from: the
   evidence also records the complete Rust source closure, and a changed, added or
   removed manifest, lockfile, crate source or header, a missing closure, or a
   missing/malformed evidence file rejects statically.
   The synthetic artifact/input mismatch tests must first reproduce acceptance by
   the present development packager, then reject after the final guard is added.
2. **P02 — actual ELF, not inferred evidence.** Independently invoke the recorded
   installed NDK llvm-readelf on each hash-bound SO: expected x86_64/AArch64
   machine, each LOAD alignment at least0x4000, the committed exact8 exported C/JNI
   boundary symbols and NEEDED limited to libc.so/libdl.so/libm.so. A bad alignment,
   unexpected export/dependency or failed tool exit rejects. Store exact command
   arrays/tool/output hashes; fake tool output is tooling-test evidence only.
   The separate real Android consumer still verifies packaged ZIP16KiB/runtime.
3. **P03 — verified classes and source closure.** Reject test classes, stale
   classes.jar digest, missing/mismatched source-before/source-after hashes or
   source drift. Include the Kotlin/Java/manual-tool/native source and matching
   Rust lock/profile/provenance digest maps in the certificate. No class from a
   development artifact or split test DEX is substituted into final packaging.
4. **P04 — actual notice closure.** Include a notice index and actual license texts
   for the exact active native Rust dependencies, SQLCipher/SQLite, LibTomCrypt,
   Kotlin stdlib2.1.20 and annotations13. Reuse an existing license file only after
   package version/source/archive checksum and text SHA agree. Distinguish
   build-only tool components from linked/runtime dependencies. Missing/changed
   license or an unclassified active package rejects. Do not invent a hoyadb
   distribution license; the local artifact is unpublished. The index's
   `lock_sha256` must name exactly the current lock paths with their current bytes.
5. **P05 — complete local coordinate.** Produce standard resource-free AAR,
   exact-version local Maven POM, source archive, NOTICE/license archive and SHA256
   sidecars under dev/hoyadb/hoyadb-android/0.1.0. The AAR contains exactly the
   expected manifest/classes/keep rules/native slices and notice files; the POM
   declares the approved exact Kotlin runtime graph. A source/license archive is
   not a replacement for retaining required notices with the binary.
6. **P06 — no partial final certificate.** Fresh output only; reject existing or
   symlinked output destinations. An interrupted/failed native inspection, missing
   input or any packaging failure cannot leave an apparently successful final
   manifest. Verify produced archive entry hashes and complete source closure
   again before publishing the local certificate. All guards work under Python -O.
7. **P07 — independent use.** Build the outside-checkout consumer from the local
   AAR/POM only, ordinary complete D8, both native slices, real ZIP16KiB validation
   and actual owned16KiB emulator execution. The RN Gradle consumer must also use
   this same refreshed AAR; standalone manual packaging is not an AGP-build claim.
8. **P08 — honest certificate.** Record actual compiler2.2.20 with
   language/API/metadata2.1, manual SDK build, source/artifact/tool hashes and
   executed x86_64 versus build-only arm64. Development builds remain explicitly
   development-only and cannot be mistaken for a final certificate. Prior native
   or Kotlin source-only checks cannot certify a changed packaged artifact.

## Concrete final layout (Opus continuation, before final implementation)

`package-aar.py --classes <production build>/classes.jar --out <fresh repo>`
(no `--development`) builds in a sibling `<out>.pending-<uuid>` directory and
renames it to `<out>` only after every check and the post-write re-verification
pass; a failure leaves the pending directory with a `failure.json` (phase and
static reason) and never a `certificate.json`. The fresh repo contains:

- `dev/hoyadb/hoyadb-android/0.1.0/hoyadb-android-0.1.0.{aar,pom}`,
  `-sources.jar` (main Kotlin/Java sources only) and `-notices.zip`, each with a
  `<file>.sha256` sidecar holding the bare lowercase hex digest.
- `certificate.json`: compiler/language/manual-build status, ABI executed/build-only
  status, AAR entry hashes, classes/source/tool before-after maps, native SO/
  evidence/input hashes, exact llvm-readelf command arrays with tool/output
  SHA256s, notice index digest and every produced file digest.

AAR entries are exactly AndroidManifest.xml, classes.jar, consumer-rules.pro,
both `jni/<abi>/libhoyadb_jni.so` slices, and `META-INF/hoyadb-android/NOTICE.md`
plus every `third-party/**` file of `bindings/android/third-party/index.json`
under `META-INF/hoyadb-android/third-party/`. The notices archive has the same
notice files at its root. The POM declares packaging aar and exactly one
dependency, `org.jetbrains.kotlin:kotlin-stdlib:2.1.20` (compile scope: the public API exposes Kotlin types), whose
own POM supplies annotations13.0; it declares no project license element.

P04 index contract: every package names `role` — `linked_rust` (normal edge in
the Android target graph), `build_tool` (build-only, no code in the SO),
`build_native_c_source` (sqlcipher-src: build role but supplies C bytes),
`linked_native_c` (SQLCipher/SQLite/LibTomCrypt compiled from that source) or
`kotlin_runtime`. Rust tuples must equal both Cargo.lock files and the
phase-six `active_native` set exactly; reused text must equal the JS corpus
entry with the same tuple; provider texts are bound to entries of the
checksum-verified sqlcipher-src crate archive; Kotlin entries bind the approved
JAR and publisher POM digests plus version-tagged upstream text URLs/digests.
Hoyadb workspace crates are first-party and get no invented license.

P04 supplemental toolchain attribution (root-directed extension): packages with
role `toolchain_aggregate` cover the pinned Rust 1.98.1 sysroot crates and the
NDK 27.1 driver-added compiler runtime. Each records `verified_link_inputs`:
the exact sysroot rlib / compiler-rt / libunwind / crt object paths with SHA256,
established from the hash-bound staticlib archive member names and the selected
clang driver's `-###` link plan, never assumed. Texts are the locally shipped,
version-bound toolchain files (Rust share/doc/rust COPYRIGHT-library.html and
licenses/*; NDK NOTICE.toolchain and NOTICE) copied byte-exact, with origin path
and digest. These are aggregate notices: they do not assert that every item they
list is linked, and they are not a compiler SBOM or legal certification. The
packager rejects a missing, corrupted or source-mismatched supplemental text,
or a changed recorded link input.

The first narrow executable guard regressions live in
tools/test_package_aar.py. Remaining archive/ELF/notice/controller cases are added
before their respective implementation; actual packaging and device results are
reported separately from synthetic tooling tests.
