# RN Android bootstrap addendum, version 2 (inert quarantine review; awaiting approval)

Version 2 corrects the initial 50-file cut (index `8a3e8ee3…`) after an
independent review: `target/phase-seven-remediation-delta-review/FINDINGS.md`,
by Claude Opus 5.5. That review found scope A not ready solely because of the
signer wording. The corrections are:

- **A1:** the signer statement is fixed (see below).
- **A2:** the missing OSV query for junit-bom was run.
- **A3:** the module signature was requested and verified.
- **A4:** the R1 wording was added.

No proposed addition, byte or resolved graph changed. The initial README,
bundle and index are preserved byte-identically in [initial-cut/](initial-cut/),
and [presentation.json](presentation.json) maps every original file.

This is preparation only. Nothing here is approved or active:

- the active repository (`target/phase-seven-rn/android/repo-v2`) and caches
  are unchanged;
- no plugin was applied and no new JAR was loaded;
- no Gradle graph retry of the real build and no compile was run.

The compiler hold and the security-lane handback still apply. This packet is
meant to be batched with the security remediation delta, so the operator makes
one decision instead of repeated one-artifact approvals.

## Why it is needed

The bounded included-build graph run (`target/phase-seven-rn/android/build-c`,
evidence in [gradle-inspection.json](gradle-inspection.json)) failed while
configuring the official `@react-native/gradle-plugin` 0.86.3 root buildscript
classpath. No task ran. The failure reported:

- `Could not resolve org.jetbrains.kotlin:kotlin-stdlib:2.2.21`, required by the
  buildscript of root project `gradle-plugin-root`;
- the missing parent POM `io.github.java-diff-utils:java-diff-utils-parent:4.15`.

Cause, from the installed Gradle 9.3.1 bytecode and resource (inert `javap`):

- **Pins:** Gradle's Kotlin DSL adds dependency constraints with
  `strictly(version)` ("Pinned to the embedded Kotlin") on Kotlin DSL script
  classpaths. The pinned modules are `kotlin-stdlib=2.2.21`,
  `kotlin-reflect=2.2.21` and `annotations=13.0`.
- **Effect:** the approved `kotlin-stdlib` 2.1.20 (from KGP) and 2.1.10 (from
  ktfmt) are therefore forced to 2.2.21 on that classpath.
- **Attributes:** script classpaths request Usage `java-runtime`, Category
  `library`, LibraryElements `jar`, Bundling `external`, the current JVM and the
  Gradle plugin API version.

## Closure proof (builtin `java` plugin only; [metadata/](metadata/))

Both mirrors used the reviewed Gradle 9.3.1 script, JDK 17.0.1-tem and fresh
isolated homes. Each ran online with `--write-locks` followed by offline
repeats, which produced identical locks.

- **Official included build** ([included](metadata/included/)):
  - The settings classpath, with the Foojay convention marker, has 2
    components and none new.
  - The root buildscript classpath, with the `kotlin.jvm` 2.1.20 and ktfmt
    0.22.0 markers plus the embedded pins, has 27 components. **New:**
    `kotlin-stdlib:2.2.21` and `error_prone_annotations:2.27.0`, the latter from
    approved `gson:2.11.0` through approved `kotlin-build-statistics`.
  - `kotlin-reflect` is not in this graph, so its pin has no effect.
  - The subprojects add no versioned plugins, and the official plugin builds
    are Kotlin DSL.
- **Consumer** ([consumer](metadata/consumer/)): the Groovy buildscript
  classpath gets no embedded pins. It has 145 components and **none new**; it
  matches the approved native-tools components exactly. Its one gap is
  metadata: `junit-bom-5.10.2.module` (the POM is already reviewed).
- **Missing metadata:** across both mirrors, the metadata absent from the
  active repository is exactly `java-diff-utils-parent-4.15.pom`,
  `kotlin-stdlib-2.2.21.pom`, `kotlin-stdlib-2.2.21.module` and
  `junit-bom-5.10.2.module` ([metadata-delta.json](metadata-delta.json) and the
  consumer `graph-delta.json`).
- **First attempt:** the first included-mirror run failed while evaluating its
  own script, because a Groovy parameter named `version` shadowed the
  `version {}` call. Nothing was resolved. It is preserved as
  `attempt1-script-error.*`.

## Proposed additions ([bundle.json](bundle.json))

| Coordinate | Files | Integrity | Signature | Licence |
| --- | --- | --- | --- | --- |
| `org.jetbrains.kotlin:kotlin-stdlib:2.2.21` | JAR 1,761,445 B `6558a3d2…`, POM, module | SHA-1 sidecars match; the JAR matches the publisher module's SHA256/SHA512; **byte-identical to the Gradle distribution's `lib/kotlin-stdlib-2.2.21.jar`** | JAR, POM and module all GOOD by Kotlin Release signing subkey `6F538074…` of primary `2FBA29D0…`: **the same pair, with a byte-identical key file, as the approved 2.1.20 artifacts in 5e771f2** (corrected A1) | Apache-2.0 |
| `com.google.errorprone:error_prone_annotations:2.27.0` | JAR 19,370 B `24c92337…` (POM and parent already reviewed) | SHA-1 sidecar matches | GOOD, Error Prone releases `EE0CA873…` | Apache 2.0 |
| `io.github.java-diff-utils:java-diff-utils-parent:4.15` | POM 10,170 B `09e7d44a…` | SHA-1 sidecar matches; equals Gradle's retrieval | **EXPKEYSIG**: mathematically valid (VALIDSIG) but by expired key `D477D518…`, the same key 8a87389 recorded for java-diff-utils | Apache-2.0 |
| `org.junit:junit-bom:5.10.2` | module 6,995 B `de23b114…` | SHA-1, SHA-256 and SHA-512 sidecars match; equals Gradle's retrieval | GOOD, `FF6E2C00…` | (BOM; POM already reviewed) |

Publisher identity trust is UNKNOWN, because the keys came from public
keyservers ([attestation/](attestation/)). Except for junit-bom, the strong
`.sha256`/`.sha512` sidecars are not published (404).

OSV exact-version queries return no matches for all four coordinates. Three
were queried in the initial cut ([osv-summary.json](osv-summary.json)), and
`org.junit:junit-bom:5.10.2` was queried in version 2
([followup-1](followup-1/osv-junit-bom-result.json); review A2). That is a registry check, not proof of
safety. Earlier advisory disclosures remain unchanged.

A JAR already being present inside the Gradle distribution does not approve
the newly selected Maven coordinate. The operator decision is still required.

Module binding (review A3): `kotlin-stdlib-2.2.21.module.asc` was requested in
version 2 and returned HTTP 200. It verifies GOOD with the same Kotlin Release
pair, using the stored key file and no network
([followup-1/module-signature](followup-1/module-signature/verification.json)).
The module is therefore bound by its SHA-1 sidecar, Gradle's independent
retrieval and its own signature; the JAR and POM signatures do not cover it.

R1 (review A4): the security lane's R1 compile-classpath constraints do not
affect the included build's root buildscript classpath. The
`error_prone_annotations:2.27.0` JAR is still needed there.

The java-diff-utils-parent `EXPKEYSIG` is disclosed; accepting it is the
operator's risk decision (review A5).

## Activation proposal (after explicit approval only)

1. Add these exact bytes to the trusted controller plan
   (`target/phase-seven-rn/android/make-plan.py`), assemble a fresh repository
   with the permanent DEF-0028 tool, and verify it before Gradle.
2. Rerun only the bounded `included-graph` step. It needs the security guard
   handback and still loads no compiler. Compare the result with every approval
   record, including this one, and stop on anything extra.
3. Compile only after the security handback.

Rollback removes the unpublished plan entries and repository; the quarantine
and logs stay.
