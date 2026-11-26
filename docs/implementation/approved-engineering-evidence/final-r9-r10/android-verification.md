# Final R9/R10 Android and Kotlin verification

The final merged runtime passed fresh Android/Kotlin verification on the task-owned x86_64 API37 emulator with actual 16384-byte pages. Both native ABIs were built, all 103 SDK cases across 24 entrypoints passed, and the fresh outside-checkout AAR consumer passed 25 groups including a real process restart. Its own app was uninstalled successfully. This is a manual SDK/AAR/APK build; it does not certify AGP, React Native or ARM64 runtime support.

## Frozen inputs and artifact chain

Root imported the reviewed R10 files, then corrected the production-only build warning by gating two diagnostic LedgerTxn helpers with `cfg(any(test, feature="work-counters"))`. The final ledger_txn.rs SHA is 87ea05ed454e5f8ba194af438eb79fe0996c583e005f59f4d2ef501586d1d329. `native-freeze-final.json` captures 77 dynamically enumerated Rust/native/header/manifest inputs; all 66 shared inputs match root's final187-input capture. Every final ABI builder re-enumerated and compared the complete map before/after compilation. `native-source-after-all.json` is byte-identical to the final freeze after device, packaging and consumer stages.

The native source manifest SHA is 4563be7ca03763034ab480b69e336e0513844e795150f93f272e29af9ecef003. Builds used the pinned Rust toolchain, NDK27.1.12297006, minSDK24, max-page-size16384, offline/locked Cargo, jobs2 and incremental0. Both final Rust archives and JNI outputs were built into new directories, preserving historical policy and superseded pre-cfg outputs. `native-final-{abi}.json` binds exact source maps, commands, native tools, provider inputs, runner, Rust archive and JNI hashes.

The unchanged approved SQLCipher/LTC providers were reused after verifying locked package archive SHA, all408 recorded C compilation units per ABI, entropy shim, config, builder, headers, archives and NDK profile. `provider-source-verification.json` and `preparation.json` retain that proof; each native build additionally rechecks provider inputs and native tool hashes before/after.

Fresh SDK test and production builds passed the reviewed compiler/runtime checks with53 and30 source/tool inputs respectively; these exact maps were unchanged after all stages. The SDK-only artifacts were built before the two Rust cfg attributes changed; they remain valid because their independent complete SDK input maps match current source. No old native output was relabeled current.

## Executed checks

Exact commands, exits and durations are in native-final-build-results.json, sdk-build-results.json, package-build-results.json, final-jni-runtime.json, final-suite.json and the consumer runtime manifest.

1. `build-native-final.py x86_64` and `arm64-v8a` completed successfully. The final x86 standalone JNI SHA is f1172588d269955ae582a6e02a89a0d06189fbdd6a74f165bc67168af588e403.
2. `bindings/android/tools/build.py --out <evidence>/final-tests --tests --split-runtime` produced a fresh hash-bound test artifact; production build omitted --tests. The reviewed dependency-only runtime DEX cache was validated, not substituted for SDK/test classes.
3. `run-frozen-suite.py` verified owned AVD identity, boot, API and page size; pushed the fresh x86 JNI to the pre-existing task-only remote root; and proved its on-device SHA equals local. It ran all24 entrypoints, including CommandTests as well as policy/native/background/lifecycle/guard/queue/model/observation/sync cases. All103 passed, none failed. Native77 and SDK53 inputs and remote JNI remained unchanged through the run.
4. The existing generated packaging JNI files were backed up with hashes, then both final Rust archives were freshly relinked into the packaging tool's required `target/phase-seven-native/jni-current/{abi}` staging paths. `package-aar.py` ran without --development and passed real native input, ELF16KiB, export/dependency, production class, notice/source and archive reverification gates. Final certificate SHA: 8899d7a65cf72763971f18cbfc14a3cb38b7ca457ff582cdbb0820e7dbfad474. Final AAR SHA: e8eb45673e35d0171a09fa0e684e3b25511d66f6e7724ea8a3b607f716fd3555.
5. `build-consumer.py` built a fresh independent complete-D8 APK outside checkout using only local Maven/AAR inputs. Native ZIP storage/alignment and APK signing/verification passed. APK SHA: ba3f176145e66ff89d75fb63e9ab5c947c0530f759a4ec069c120c1403798872.
6. `run-consumer.py --uninstall-after` passed23 full-mode groups, one seed group and one restart group (25 total). All three reports have zero failures and actual16384-byte pages. Seed PID12608 and restart PID12653 differ; the runtime manifest confirms a fresh process. Its own app installation state is uninstalled, no failure is recorded. Runtime manifest SHA: f61aeeb695dc20d69bbe64aaa8f7df12023ede9fc9fd4fce1c134e44a5e7470c.

Fresh packaging relinks produced distinct JNI hashes. The actual APK consumer exercised those exact outputs; their runtime result is separate from the standalone suite. Packaged x86 SHA24b7af575ce84ce5e081cfe10b4e3ee5938f39436348798476acbf04ab2f612b; packaged ARM64 SHA9ff230891737eacf543a2e00817bfce3714d8a3b8191db86d29d6b3b406ea78c. Both reference the final native archives through their evidence.

## Preserved attempts and scope limits

The first SDK invocation used /tmp and was rejected before compilation by the existing canonical-path guard. Its command/log/result remain as sdk-build-initial-results.json and build-sdk-tests-initial.log. The caller was corrected to /private/tmp; no build guard changed.

Pre-cfg x86 and ARM64 builds remain preserved and explicitly superseded. While pausing, ARM64 crossed its preflight before the freeze marker was renamed; that race was corrected immediately in the coordinator update and PAUSE.json. Both builders ultimately returned exit0 with old matching before/after maps; the ARM64 record predates the cfg source edit by9.31seconds. Neither output was used for final runtime or packaging. Both ABIs were rebuilt under distinct final names after the refreshed freeze.

No production code, SDK behavior, test assertion, package check, timeout or permission control was changed by this verification lane. Only task evidence and generated native staging outputs were written. The task-owned emulator is left running for the coordinator; only the fresh consumer app was installed and then removed. ARM64 is build/ELF-only. Whole-workspace, web/Apple and R11 decisions remain with their owning lanes; this report makes no claim for those gates.
