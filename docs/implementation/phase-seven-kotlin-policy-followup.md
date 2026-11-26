# Kotlin policy implementation and Android verification

Recorded 2026-10-31 for SPEC-0047/TC-0051, implementing the accepted
SPEC-0040/API-0011 contract and TC-0044 CP13. No new runtime dependencies,
publication, commits or user-store operations were involved.

`Conflict.kt` supplies immutable/redacted policy and resolution models, exact
facade translation and the explicit setter/resolving/discard APIs. `Database`
installs constructor policy before publishing each owner, retains failed cleanup
for explicit close retry, preserves original constructor forms, and routes typed,
advanced and managed reconciliation through the core reporting operations.
KeptCurrent callbacks are epoch-scoped and contained; failures report through the
existing background sink without changing committed results or invoking sync
`onError`. Kotlin preserves the original configuration error if cleanup also
fails; the current JS/Swift surfaces return the cleanup error. An inline executor
runs resolution callbacks before managed mutation wakes; callbacks should be short.
Shared eligibility, guards, bounds, stored state and fallback semantics remain in
Rust. QueueState adds Discarded without inventing a remote outcome.

## Regression evidence

The task evidence root is `/tmp/hoyadb-followup-20261031/kotlin`. Before behavior
changes, inert declarations enabled compilation and the controlled runtime
packet failed 12 of 17 cases for missing behavior. Missing symbols and an initial
real-JNI fixture error (proposed IDs missing their required `local:` prefix) were
not counted as feature RED. Exact RED command:

```sh
python3 bindings/android/tools/run-tests.py --out /private/tmp/hoyadb-followup-20261031/kotlin/red-native --class-name ConflictPolicyTests --log /private/tmp/hoyadb-followup-20261031/kotlin/runtime-red-controlled.log
```

After correction, the final packet passed 103 unique cases in 24 entrypoints:
17 controlled policy/lifecycle cases, four actual JNI policy cases, 18 background
cases, four command-admission cases and 60 other SDK cases. Alias entrypoints
were excluded from the total. `final-suite.json` records every sweep command;
`final-CommandTests.json` records the additional admission run. Every log has an
adjacent JSON binding its runtime, DEX and build manifest. Build command:

```sh
python3 bindings/android/tools/build.py --out /private/tmp/hoyadb-followup-20261031/kotlin/final-tests --tests --split-runtime
```

Manifest SHA256: `ea42cf606acd677ce14198c41a356c314e9eb04e420aeb7c8778a347d15ed2a5`.
Test DEX: `9b41eefc4cfcafbcf43e0ad2343658443f063ee066e5fc663085da851c3b0042`.
All 53 SDK source/tool inputs matched the current tree at freeze. The complete
packet `final-complete-freeze.json` has SHA256
`af6b66c2e9e3a5fd788b9ce7080c6a3cf2fccbdb81042a6420eb030c48b28a20`.

The broader sweep exposed two stale global-admission tests. DEF-0055 records
the original failures and their correction to actual same-owner barriers,
independent-owner progress and cancellation release. No core behavior, runtime
assertion or timeout was weakened.

## Native package and outside-checkout consumer

Both approved SQLCipher/LTC providers and native Rust/JNI slices were rebuilt
from the recorded 74-input policy checkpoint, offline with the pinned toolchain.
`native-current-x86_64.json` and `native-current-arm64-v8a.json` bind source,
commands and providers. The test runtime's JNI SHA256 was verified on device as
`27a26bc8e667fd8b6088d656693d67aafd0c99c7cceb86d59f41649e3ee08582`.
Production packaging relinked those fresh archives into its required JNI output
paths; its different exact SO bytes are bound in the final package certificate.

`tools/build.py --out <evidence>/production` and
`tools/package-aar.py --classes <evidence>/production/classes.jar --out <evidence>/maven`
passed the final P01–P08 gate without `--development`. The certificate hash is
`1002ad8a5ec84e85c6636ed12aa604899ec62fee4b7946d6ce91dc3038ba55a7`;
the AAR hash is `fa1658c258ab2b0eac08b70ed6656e3c722c42063a5a3dc00c794635078237a1`.

`tools/build-consumer.py --maven-repo <evidence>/maven --project <owned-temp-project>`
and `tools/run-consumer.py --project <owned-temp-project> --out <evidence>/consumer-run --uninstall-after`
built and ran an ordinary complete-D8 APK outside the checkout, using only the
local AAR/POM surface. `consumer-project.json` records the exact owned temporary
path. All 25 groups passed: the previous 23 plus memory/protected CP13 policy,
resolution and throwing-callback containment cases. Fresh-process restart was
checked, and the app was uninstalled afterward. Runtime manifest SHA256:
`0a9722004298f4298bdda1e2f283fac211b91a1eeb583e4a65518b15f0cad592`.
APK SHA256: `c2346a56a4750f34c389d1d4e5a29d1f68d1880592cdaa5f91ee064ae930f007`.

The owned x86_64 API37 emulator reported 16384-byte pages. ARM64 is build/ELF
verified only. The manual Kotlin consumer does not certify Gradle, React Native
or other platforms. These native results bind the recorded policy checkpoint;
subsequent Phase 3 core changes require a fresh build and rerun before any claim
about that later source. Historical certificates and failed/initial logs remain
preserved.

Claude Fable 5.1 reviewed the policy packet and found no actionable defect;
its two documentation notes are reflected above. The separate CommandTests
follow-up is submitted for review. The full task record, including pre-RED
acceptance and intermediate results, is `implementation.md` in the evidence root.
