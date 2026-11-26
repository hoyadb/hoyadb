# Project review remediation — 2026-10-30–31

This work follows SPEC-0046 / TC-0050 and the operator's request to fix the
project review findings, cross-review with Claude Fable 5.1, and report remaining
work. The reviewed baseline was the existing dirty tree on
`035bd0f180ca73c017813f03c454d4ad85936927`, not a clean checkout of that commit.
The pre-fix contents and hashes were saved before editing. No commit, push or
publication is part of this work.

## Changes

| Review finding | Record | Result |
| --- | --- | --- |
| Explicit application sync survives reset/close | DEF-0044 | Track and cancel explicit work, await submitted native work before disposal, reject late results and permit subsequent synchronization. |
| Busy repeats transport/mapping | DEF-0045 | Reuse release-aware admission; retain exact responses, captured mapper context and encoded proposals. |
| Failed tagged factory leaks an owner | DEF-0046 | Validate and capture sync configuration before acquisition; dispose after later startup failure. Direct owners retain cleanup retryability. |
| RN Android owner publication races invalidation | DEF-0047 | Serialize publication and retirement; close acquired owners outside the lifecycle lock. |
| Swift checker command omits trust anchor | DEF-0048 | Document independently captured compilation-parent digest and packed-copy checking, preserving the trust gate. |
| Stale browser recovery guidance | DEF-0049 | Update active SDK and public HTML guides to match fresh-worker recovery and permanently closed old handles. |
| Clean JavaScript CI lacks WASM | DEF-0050 | Build ordinary assets with the exact CLI and supply them through an explicit CI artifact dependency; document contributor prerequisites. |
| Quality evidence includes excluded/generated sources | DEF-0051 | Derive input inventory from the four root-workspace members and detect actual member/source changes. |
| Trace validation silently accepts invalid inputs | DEF-0052 | Reject unusable roots, reads and unterminated frontmatter; RTM generation refuses partial input without overwriting evidence. |
| Active roadmap requires private process rules | DEF-0053 | Link active workflow instructions to the public contribution guide. |
| Pending list/context settlement race, found during fixes | DEF-0054 | Omit a stale key only after fresh core confirmation; retain lifecycle and genuine context errors. |
| Kotlin native sync test assumes app-wide blocking | DEF-0055 | Replace the superseded cross-database Busy assumption with actual same-database contention and an independent-progress twin; both passed against current-source JNI. |

The five pre-existing Rust formatting failures were corrected with the pinned
formatter. Review of their complete diff found only layout and trailing-comma
changes, with no change to database behavior.

The existing Kotlin DEF-0040 correction adds the declared redacted database sink,
terminal observation delivery and automatic failure reporting. Eighteen focused
groups passed on the task-owned Android runtime, including sink containment,
epoch retirement and preservation of awaited failures. The absent Kotlin
Stop/Discard/onResolution surface and an automatic merge driver remain explicit
limits. Nearby suites with freshly rebuilt JNI first passed 20 of 21 groups;
the K10 failure exposed the stale cross-database test assumption in DEF-0055.
Both replacement cases passed, along with the existing fake-transport case and
all 18 background groups, in the rebuilt final packet. The other 19 neighboring
groups passed the preceding packet with identical runtime logic.

## Regression and verification evidence

Behavior regressions preceded the corresponding fixes. The JavaScript REDs
reproduced hung explicit waits, premature disposal, repeated mapping, retained
owners, mutable mapper-context corruption and the list/context race. The Android
Java race failed deterministically at owner publication. Tooling REDs reproduced
false source drift and successful validation of unusable input. The CI regression
failed on the missing generated-asset dependency before it was added.

| Check | Observed result |
| --- | --- |
| `cargo test --workspace --offline --locked` | 702 tests passed across 77 targets; zero failures or ignored tests; exit 0. |
| `cargo clippy --workspace --all-targets --offline --locked -- -D warnings` | Passed. |
| `cargo fmt --all --check` and formatting policy fixture | Passed. |
| `cargo build -p hoyadb-core -p hoyadb-storage-memory --target wasm32-unknown-unknown --offline --locked` | Passed. |
| Ordinary web bridge build with wasm-bindgen 0.2.129 | Passed. |
| `npm test` | 187 client and 11 testing tests passed. |
| Clean source copy, offline npm install, supplied ordinary WASM artifact | Install passed; the same 198 JavaScript tests and 10 RN host tests passed. |
| npm build/check and packed TypeScript/WASM consumer | Passed. |
| Actual RN Java module with controlled host dependencies | Four new lifecycle regressions passed; normal and optimized Python invocations passed. This is not JNI/device certification. |
| RN Android tooling / Apple tooling | 20 / 53 tests passed. |
| Kotlin background reporting on Android | 18 groups passed; controlled native replies test host behavior. Final manifest and all 50 SDK/test source hashes match. |
| Neighboring Kotlin SDK suites | 22 groups passed across the two packets described above, including both current native admission cases. JNI was rebuilt from current source; this is not AAR/consumer certification. |
| Android build/package tooling | 52 tests passed. |
| Public traceability / packaging / quality tooling | 13 / 4 / 32 tests passed. |
| Public documentation tooling | 45 tests passed; mocked runner output is not actual tour execution. |
| Browser retirement regression | Existing QUALITY10 host case passed. |
| Design trace / RTM | Zero violations; generated matrix current. |

The Rust log's synthetic failed-child assertion belongs to a passing supervisor
regression; counts above use the final result of each Cargo target. Full log
SHA256: `56d703e10bdb2ccd83f668c203f77c54dac11d3fb0e3f18df906116c9b01b772`.
These runs shared the host with Android builds and are verification, not timing
benchmarks. A final byte comparison preserved all 2,264 existing decision,
reference, notice and implementation-evidence files checked against the saved
baseline. HEAD remains unchanged. The task-owned emulator was stopped after its
identity and JNI digest were rechecked.

The corrected Swift checker invocation supplied a correctly formatted digest,
but the current local framework failed `missing-packaging` before the actual
parent-hash comparison. Four of five checks passed; this is not an end-to-end
artifact pass. Source inspection confirms the finalizer copies the trusted
parent bytes verbatim, and synthetic tooling tests cover valid/invalid anchors.
Fresh provider/packaging evidence is still required for current Apple artifacts.

The Kotlin packet was built with
`python3 bindings/android/tools/build.py --out /private/tmp/hoyadb-fixes-20261030/kotlin-admission-reviewed --tests --split-runtime`
and executed through `bindings/android/tools/run-tests.py` for
`BackgroundErrorTests` and `SyncDriverTests`. The final runtime records bind DEX
`68bf5d1f6fab115a9531e4305a6db5b01306fd3fd559be12ab3fb4fe6233accf`
to manifest `d969dfbeb11bd83f04b98fc7b93f786ef024c9b890bf2c90cacd8d42086ab94c`.
Before/after/current source hashes agree. Initial missing-fact REDs and the
throwing-sink process failure preceded implementation; the two cross-review
cases separately failed before their corrections.

The JNI used for nearby suites has SHA256
`a030509cbf68e64e37214e2b5dd63b8386f617cbeffd7f0c6196d5e2d1d66450`,
matched locally and in the task-owned runtime. Current source hashes and fresh
provider evidence bind its build in `android-native-current.json`. The old K10
failure is retained as evidence; its replacement waits for a real Busy reply,
and the independent-owner twin requires completion before releasing the blocker.

## Claude Fable 5.1 cross-review

The independent Paseo session is `0b48cccf-9809-4e39-90aa-ce9291333283`, titled
`Cross-review hoyadb remediation`, using `claude-fable-5-1`. It compared against
the saved working-tree baseline, inspected the implementation and tests, and
independently ran JavaScript, Java lifecycle, formatting and public tooling checks.

The first review found no JavaScript, tooling or CI correctness defect. Its
Swift evidence wording correction is incorporated above and in DEF-0048. The
implementation record and changelog were pending at that review and are now
present. The Kotlin review found a stale sync adapter invocation after an inline
sink retired its epoch, and overly broad reporting of manual unsubscribe failure.
Both have observed RED/GREEN regressions. Fable's final rereview independently
matched all 50 source hashes, the manifest and DEX to the 18 passing groups,
confirmed both findings resolved, and found no further actionable Kotlin defect.
Its subsequent DEF-0055 review independently verified the final 18+3 packet and
all source/artifact hashes, confirmed the superseded global-blocking assumption,
and found that the new barrier-based contention/progress tests strengthen the
contract checks. These cases exercise native memory stores; they do not certify
every adapter's admission path.
Nonblocking RN notes were kept distinct from defects: promise settlement remains
inside the publication/retirement ordering, and concurrent explicit close plus
retirement can make a harmless duplicate close against a never-reused handle.
The Java lifecycle regression is currently a documented manual host check.
Kotlin's pre-existing count-listener retirement after timer scheduling failure
does not deliver a terminal callback; the current contract leaves that case
unspecified. Fable recorded it as a nonblocking limit.

## Remaining work and limits

- **DEF-0034, SPEC-0037 Phase 3 R9–R11:** bounded ledger transaction/delta
  persistence, format 3 and the reversible offline converter remain engineering
  work. ADR-0038 already approves the format/converter policy; SPEC-0038 is
  recorded as landed. The remaining sequence is phase-specific case review,
  behavioral RED, implementation and execution evidence. Existing Phase 1/2
  admission overlays and memory undo guards are already implemented. This
  remediation does not claim to remove the remaining whole-ledger cost.
- **Factory double failure:** an injected post-acquisition host startup failure
  combined with a native close failure has no caller-visible cleanup handle
  through the current factory contract. Normal invalid configuration is now
  rejected before acquisition; direct Database cleanup retry is tested. Fable
  agreed that an invented error API or unbounded retry loop is not justified.
- **Kotlin breadth:** Stop/Discard/onResolution and TC-0044 CP13 remain separate
  unimplemented integration work. Awaited pull/merge failures return to callers;
  no automatic Kotlin merge path is claimed.
- **Contract and compatibility gates:** API-0011 still leaves TC-0016's
  unprefixed custom command-ID example pending RFD-0006 clarification. The
  native/Swift/RN API snapshots remain inert historical baselines; automated
  compatibility enforcement and fresh release-surface evidence remain work.
- **Distribution decisions:** project license, security-reporting channel,
  supported-release/update policy, registry ownership, release target claims and
  remote Apple delivery remain open. Packages stay private/unpublished.
- **Execution/certification:** remote Linux/GitLab execution, actual RN Android
  consumer execution, wider arm64/device coverage, matching rebuilt artifacts,
  full performance/coverage/mutation baselines and the complete platform security
  matrix are not established by these host checks. Historical evidence is not
  recertified. No user store was migrated, reset, deleted or used as a fixture.

Detailed command logs, RED/GREEN reports, clean-copy input hashes, Fable's review,
the exact baseline and task-only diff are in the local task evidence directory
`/tmp/hoyadb-fixes-20261030/`. They describe this source state, not a release.
