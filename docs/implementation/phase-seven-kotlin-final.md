# Phase 7 Kotlin/Android final local certificate and runtime evidence

Recorded 2026-10-25 by the Kotlin/Android Opus lane. This continues the Codex
handoff `target/phase-seven-kotlin/HANDOFF_OPUS.md` (SHA256 `e7f15b36…`). Its
manifest `2a3ab56b…` was rechecked before any edit: 52 source files and 8 tool,
2 runtime and 8 artifact hashes were unchanged. The working evidence is under
ignored `target/phase-seven-kotlin/opus/`. Durable text copies are in
[phase-seven-kotlin-final/](phase-seven-kotlin-final/), and
[package/final-hashes.json](phase-seven-kotlin-final/package/final-hashes.json)
binds the exact source cut. No staging, commit, publication or customer store
was touched. Root owns Git, and the independent final review is still pending.

## Final local Maven coordinate

The repository root is
`target/phase-seven-kotlin/opus/maven-final/dev/hoyadb/hoyadb-android/0.1.0/`.
It was produced by `python3 bindings/android/tools/package-aar.py --classes
target/phase-seven-kotlin/opus/final-classes/classes.jar --out
target/phase-seven-kotlin/opus/maven-final`.

| File | SHA256 |
| --- | --- |
| hoyadb-android-0.1.0.aar | f2e8a16fcc1692d8484d8cc819a85aa27a9d2ce754b30321cd89ec1f689e0bc7 |
| hoyadb-android-0.1.0.pom | 146de9a71014e8ffc2399d104ed318d07d11375073be35f53dac7ec46c2def1b |
| hoyadb-android-0.1.0-sources.jar | d7f84b9ef487ed732a11e9bbfa2dba922d14a18c8037ea21479891a76a048a98 |
| hoyadb-android-0.1.0-notices.zip | e37623f3a6ef606219040b72a22f74a76637c880785f8a33c9730cd5eaad6d43 |
| certificate.json | 4d27da86c5124ebce929e6eb71ab23c157d438adbacdfa9b54736a430d1498fb |
| production classes.jar | a40ab5307ae209e8629aad3d6a1dfdff10fb12012522b8d10004f8c898f75646 |

- **JNI slices:** the native-lane bytes are unchanged (x86_64 `3b8c7f5a…`,
  arm64-v8a `2ca0467d…`). The packager's own llvm-readelf run found ELF64 DYN
  with the expected machine, exactly the 8 `exports.map` symbols, NEEDED
  libc/libdl/libm and every LOAD at 0x4000 for both ABIs.
- **Compiler:** the certificate records the installed compiler 2.2.20 with
  language/API/metadata 2.1 and a manual SDK build, not an AGP build.
- **ABIs:** x86_64 was executed; arm64 is build-only. The certificate was written
  before the consumer run, so its x86_64 status reads "runtime pending
  independent consumer". Runtime proof is the separate P07 record below.
- **Superseded artifacts:** the development AAR `843e8d71…`, the first candidate
  `b5e931c7…` (packaged before the tool/notice changes) and the second candidate
  `e056ee58…` (protected-open defect) are not final.

## TDD record, in order

1. **TEST_TOOLING case 8: build.py source closure.**
   - RED (`tooling/build-source-closure-red.txt`): a source or tool added during
     compilation produced a success manifest, and a removed source crashed with a
     raw FileNotFoundError.
   - GREEN after re-enumerating the provenance set: 4/4 normal and `-O`.
2. **Final certificate in package-aar.py.**
   - The first failure was setup-only, a missing patch seam
     (`package-final-happy-setup-missing-seam.txt`). The behavioral RED was
     `final certificate not implemented`, and a minimal final path made it pass.
   - 26 guard tests then went RED: 23 bad inputs were certified and 3 crashed
     unstructured, including a rename onto an existing destination and a
     symlinked destination.
   - GREEN after the P02/P03/P04/P06 guards (`package-guards-{red,green}.txt`).
3. **Supplemental toolchain notices (root-directed).**
   - Doc-first contract in PACKAGING_CASES.
   - RED (`toolchain-notice-red.txt`): the new `toolchain_aggregate` role was
     unclassified, and the missing, corrupted, source-mismatched and changed
     link-input cases were unmet.
   - GREEN normal and `-O`.
4. **Real-app protected-open defect, found only by the APK.**
   - Consumer run 1: every protected open in the installed app threw
     InvalidRequest. Diagnostic runs 2–4 added a static throw site and located it
     at `Database.kt:98`: `base.canonicalFile != base.absoluteFile` for the
     OS-provided Context directory.
   - A plain owned directory did not reproduce it
     (`regression/…plain-dir-not-reproducing.txt`). The aliased-ancestor
     regression did (`…alias-red.txt`).
   - The fix canonicalizes the OS base once, then still requires a canonical,
     non-symlink owned `hoyadb` child. The symlinked-child guard passed both
     before and after the fix. GREEN 4/4 (`…alias-green.txt`).
   - **Root may want to allocate a DEF for this; I have not.**
5. **Fixture corrections, with no assertion weakened and no SDK change.**
   - K09 configured entities after live commands. The core rejects that
     (`entity_owner.rs` returns Protocol while live recovery exists), so the
     fixture now configures first.
   - K09 rolled back a Rejected command before reconciling it. Core `inverse`
     requires Rejected and reconciled (`tests/conformance/command_sync.rs`), so
     the fixture now reconciles first.
6. **Test-source warnings.** Two parameter-name warnings were fixed in OwnerTests
   and ReleaseFailureTests. Production and test builds now emit zero compiler
   output.

## Verification on the final source cut

- **Tool tests:** 52/52 under normal Python and `-O` (`tooling/tools-final*.txt`).
- **Full suite, ordinary complete D8:** 18 classes, 50/50 PASS, every class exit
  0 (`suite-final/`). Every log binds DEX `e43b00ec…` and manifest `1cb40434…`.
  Test-build sources equal the current files, and production-build sources equal
  their non-test subset.
- **Pre-fix suite:** 17 classes, 46/46 PASS on the earlier cut. It is preserved in
  `suite-pre-fix/` and superseded by the run above.
- **Full versus split:** SdkTests 6 and TransportTests 2 pass in split mode
  (`split-final/`). Classes.jar is identical to the full build; split SDK DEX is
  `b18e2c18…` and the validated runtime DEX is `6403125c…`.
- **Outside-checkout APK (P07, C01–C06):** run 7 passed 23/23 against AAR
  `f2e8a16f…`.
  - Groups: 21 full, 1 seed and 1 fresh-process restart, with PIDs
    8155, 8406 and 8450. Pages were 16384.
  - Build: APK `85d20837…`. JNI was stored uncompressed, zipalign `-P16` was
    verified and the APK was signed with a synthetic key.
  - Run: on owned emulator-5588 (API37, 16 KiB pages); the owned app was
    uninstalled after success.
  - Runs 1–6 are preserved in `consumer/` with their failure manifests. Run 6
    hit the 30 s `am start -W` bound while the host load average was about 150
    from the user's own unrelated iOS build. That is a harness setup timeout,
    and the retry passed unchanged.
- **Cleanup (`package/cleanup.txt`):** only the 6 owned synthetic apps were
  uninstalled; owned emulator-5588 was stopped and no qemu process remained.
  The owned remote test root was retained.

## Notice closure (P04) and attribution limits

- **Index and summary:** `bindings/android/third-party/index.json` (`28b425a3…`)
  and `bindings/android/NOTICE.md` are packaged into the AAR under
  `META-INF/hoyadb-android/` and into the notices zip.
- **Rust crates:** all 13 `active_native` tuples equal both Cargo.lock files, the
  phase-six list and the JS corpus. The texts were cross-checked against the
  checksum-verified `.crate` entries and the rusqlite git checkout 661107f.
  - Roles come from the actual `cargo tree` for the Android targets: 7 linked
    Rust crates, 5 build-only tools, and sqlcipher-src, which is a build-time
    source of C code.
- **SQLCipher/SQLite/LibTomCrypt:** texts are bound to sqlcipher-src crate
  entries.
  - The committed `licenses/sqlcipher/LICENSE-libtomcrypt` (`1ba8f800…`) differs
    from the crate entry (`8f196cb1…`) by trailing whitespace. Android uses the
    exact crate bytes. The root file was not edited, and no legal significance is
    inferred from the difference.
- **Kotlin:** neither runtime JAR embeds a license. The texts are tag-bound
  v2.1.20 upstream files fetched read-only, with provenance in
  `notices/kotlin-upstream-provenance.json`: LICENSE (identical to
  apache.org LICENSE-2.0), NOTICE, and the GWT, ThreeTen-Backport, Guava and
  Boost texts that the README attributes to stdlib sources. annotations 13.0 uses
  its POM-declared Apache 2.0 text.
- **Toolchain aggregates:** link inputs were verified from the hash-bound
  staticlib members and the NDK clang `-###` link plan
  (`notices/toolchain-link-evidence.json`).
  - Rust 1.98.1: 17 sysroot crates per ABI, with rlib hashes.
  - NDK r27b: compiler-rt builtins, libunwind and bionic crtbegin/crtend_so.
  - Texts are byte-exact local copies of the Rust `COPYRIGHT-library.html` and
    the NDK `NOTICE.toolchain` and `NOTICE`.
  - **Limits:** these are aggregate notices. Not every item they list is linked,
    and this is not a compiler SBOM or a legal certification. The Rust text does
    not name compiler_builtins, miniz_oxide, panic_unwind, std_detect or
    rustc_std_workspace_alloc/core individually, and the NDK texts do not name
    the crt objects.
- **Project license:** none is claimed, and the validator rejects one.

## Independent checkpoint review

[Claude's Android SDK/package review](phase-seven-android-review.json) passes
with no confirmed defect. It recomputes package/certificate/ZIP/source/tool
bindings and checks the recorded50-test suite and23-case consumer against the
certified AAR. It does not rerun devices or compilation and is not the final
whole-Phase7 review. The protected-open defect is recorded as DEF-0024.

The consumer README gained results and a corrected evidence link after the APK
run. Its old template digest therefore differs; it is documentation, not an APK
build input. The four actual consumer inputs remain matched. The packager also
uses host-specific installed-tool paths, so re-certification on another host is
unverified. Neither qualifier changes the certified AAR bytes.

## Git integration byte check

DEF-0027 preserves exact upstream notice and raw evidence bytes through Git.
Before the milestone commit, all225 staged files matched their working bytes.
The complete whitespace check reports upstream notice formatting and the raw
CRLF cleanup capture; it is not a passing check. Authored source/tool/docs
checks are clean. No notice, raw log, certificate or binary was normalized to
silence those findings. See the [regression and review](phase-seven-git-preservation/README.md).

## Remaining open items

- Root owns selective Git integration; the independent final whole-Phase7
  review still requires the remaining matching-artifact gates.
- The RN Android Gradle consumer still has to consume this same AAR (native/RN
  lane). arm64 has no runtime execution. No performance measurement was taken.
