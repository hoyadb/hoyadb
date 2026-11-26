# Final Android/Kotlin evidence closure — read-only review (Claude Fable 5.1)

Date: 2026-10-31. Packet `/tmp/hoyadb-followup-20261031/kotlin-r10/packet.json` `bc190dac…621f9e`
(matches). No repository edit; nothing rerun by me. Scope per `verification.md`: manual SDK/AAR/APK
build, x86_64 API 37 16 KiB runtime, ARM64 build/ELF only; no AGP, React Native or ARM64 runtime
claim.

## Provenance chain (verified)

- `packet.artifacts`: all 125 listed files match their sha256.
- Native: `native-freeze-final.json` (77 inputs, manifest `4563be7c…`) == `native-source-after-all.json`;
  all 77 equal current main; the 66 inputs shared with root's 187-input freeze match; `ledger_txn.rs`
  is `87ea05ed…`. `native-final-{x86_64,arm64-v8a}.json` exit 0 with rust archive and JNI hashes;
  `jni-final-x86_64/libhoyadb_jni.so` = `f1172588…`, `jni-final-arm64-v8a` = `9c283382…`.
  Provider reuse: `provider-source-verification.json` records the locked SQLCipher/LTC package
  archive sha and 408 verified compilation units per ABI with matching source hashes.
- Device suite: `device-commands.json` shows AVD `hoyadb_phase7_owned_ps16k`, boot 1, SDK 37,
  `PAGE_SIZE` 16384; remote JNI sha before push `27a26bc8…` (previous file), push of
  `jni-final-x86_64/libhoyadb_jni.so`, then two post-push `sha256sum` = `f1172588…`. `final-suite.json`
  24 entrypoints, independently summed pass 103 / fail 0, all exit 0; per-entry JSON records the SDK
  and runtime DEX hashes and artifact manifest.
- SDK: `final-tests/manifest.json` (`01b07ff5…`, 53 sources before == after, 13 tools) and
  `production/manifest.json` (`bc249610…`, 30 sources) match the summary hashes; every listed source
  equals current main (the two `runtime_sha256` entries are build outputs, not repo files). The
  SDK builds predate the two-attribute cfg change, which touches no Kotlin/SDK input; their input
  maps still match main, so the claim "remain valid" holds.
- Packaging: `package-build-results.json` four stages exit 0 (two JNI relinks from the final rust
  archives, `package-aar.py` without `--development`, `build-consumer.py`). Backed-up previous
  `jni-current` hashes retained in `packaging-jni-backup.json`. `target/phase-seven-native/jni-current/
  {abi}/evidence.json` binds `artifact_sha256` `24b7af57…` (x86_64) / `9ff23089…` (arm64-v8a) to
  commands whose `--archive` is the final `native-final-{abi}` archive; the `.so` files match.
  `maven/certificate.json` `8899d7a6…` lists AAR `e8eb4567…` (file matches), POM, sources jar,
  notices zip, per-entry hashes, `development_only: false`, ABIs "x86_64 runtime pending
  independent consumer" / "arm64-v8a build-only".
- Consumer: `consumer-run/runtime-manifest.json` `f61aeeb6…` binds the same AAR and APK
  `ba3f1761…`; `full.json` 23 groups, `seed.json` 1, `restart.json` 1 (25), `failed: 0` in all three,
  `page_size` 16384, PIDs 12447 / 12608 / 12653 distinct, `process_restart_checked: true`,
  `failure: null`, final command `adb uninstall dev.hoyadb.consumer.p9009f69609844152` exit 0.
  Non-zero exits inside the command trace are `pm path` before install, `run-as cat results-*.json`
  polling before the file exists, and `pidof` after `force-stop`; all expected, none counted as a
  failure.
- Superseded pre-cfg builds and the ARM64 preflight race are recorded in
  `superseded-native-attempt.json` / `PAUSE.json` and explicitly excluded from the final chain; both
  final ABIs were rebuilt under distinct names after the refreshed freeze.

## Findings

None actionable. Evidence-scope notes:

1. **INFO.** `package-aar.log` contains only the certificate line; the ELF 16 KiB, export,
   dependency, class, notice and reverification gates are asserted by `package-aar.py` exit 0
   (stage recorded) rather than by visible log lines. Acceptable, but a verbose gate summary in
   the log would make the chain self-evident.
2. **INFO.** `evidence.json` for the packaged JNI records `runtime_verified: false`; runtime
   proof for those exact relinked `.so` files comes from the consumer APK run (25 groups), while
   the standalone 103-case suite ran the separately built `f1172588…` JNI. `verification.md`
   states this distinction correctly.

## Limits

Not rerun by me. x86_64 emulator only; ARM64 is build/ELF; manual build, no AGP/RN; whole
workspace, web, Apple and R11 are other lanes.
