# Kotlin 2.1.20 compiler build addendum (preparation only, awaiting approval)

This packet prepares one combined decision instead of repeated one-file
approvals. The unchanged official RN 0.86.3 Android includeBuild and the RN
consumer app's Kotlin compilation need Kotlin compiler build-process inputs that
neither ADR-0035 (dc5469e/1a914cd) nor the plugin addendum (03048bb/8a87389)
covers. **Nothing here is approved or active.**

- Nothing has been added to the active local repository
  (`target/phase-seven-rn/android/repo`) or to any build cache.
- No marker was applied.
- No new JAR was loaded or executed.
- No Gradle compile ran.

Every byte is quarantined under `target/phase-seven-rn/compiler-review/` and
`target/phase-seven-rn/marker-review/`. App, SDK and core runtime pins are
unchanged.

## Why this is needed

The offline graph step stopped on the missing plugin marker
`org.jetbrains.kotlin.jvm:org.jetbrains.kotlin.jvm.gradle.plugin:2.1.20` (see
`../phase-seven-rn-ios/README.md` and `marker/PACKET.md`). Reading the approved
KGP 2.1.20 sources, recorded in [kgp-source-evidence.json](kgp-source-evidence.json),
shows that compiling Kotlin also resolves three configurations whose contents
are not approved.

| Configuration (KGP 2.1.20, gradle85 variant) | Declared dependency | Attributes |
| --- | --- | --- |
| `kotlinCompilerClasspath` | `kotlin-compiler-embeddable:${pluginVersion}` (default dependency) | none |
| `kotlinBuildToolsApiClasspath` | `kotlin-build-tools-impl`, strictly `2.1.20`; feeds classpath-snapshot and script-discovery transforms | none |
| `kotlinCompilerPluginClasspath[Main/Debug…]` | `kotlin-scripting-compiler-embeddable:${pluginVersion}` from the unconditional scripting subplugin | Category library, Usage java-runtime, LibraryElements jar, GradlePluginApiVersion |

Both the official plugin build (Kotlin sources, `jvmToolchain(17)`) and the
consumer app (`.kt` sources under `org.jetbrains.kotlin.android`) compile
through these configurations. The native/multiplatform commonizer classpath is
excluded, because JVM/Android builds never resolve it.

## Exact additions proposed (full detail in [bundle.json](bundle.json))

1. **One marker POM:**
   `org.jetbrains.kotlin.jvm:org.jetbrains.kotlin.jvm.gradle.plugin:2.1.20`.
   It is 1397 bytes with SHA256
   `5ccb905a796717218c1bb4eed8bb31fc674c5803cdd54c23753ead380f7a11b5`, and its
   only dependency is the approved `kotlin-gradle-plugin:2.1.20`
   ([marker/](marker/)).
2. **Kotlin compiler closure, 11 JAR components:**
   - `kotlin-compiler-embeddable`
   - `kotlin-daemon-embeddable`
   - `kotlin-build-tools-impl`
   - `kotlin-scripting-compiler-embeddable`
   - `kotlin-scripting-compiler-impl-embeddable`
   - `kotlin-scripting-common`
   - `kotlin-scripting-jvm`
   - `kotlin-script-runtime` (all `2.1.20`)
   - `kotlin-reflect:1.6.10` (declared by the compiler POM; distinct from the
     approved `kotlin-reflect:2.1.20`)
   - `trove4j:1.0.20200330`
   - `kotlinx-coroutines-core-jvm:1.8.0`

   Already-approved members are not repeated: `kotlin-stdlib:2.1.20`,
   `annotations:13.0`, `kotlin-build-tools-api`, `kotlin-compiler-runner` and
   `kotlin-daemon-client`.
3. **Official RN plugin compile classpath, 6 JAR components:**
   - `gson:2.8.9`
   - `guava:31.0.1-jre`
   - `failureaccess:1.0.1`
   - `error_prone_annotations:2.7.1`
   - `j2objc-annotations:1.3`
   - `checker-qual:3.12.0`

   The plugin's `libs.versions.toml` pins Gson and Guava at these versions, and
   plain Gradle compile resolution keeps them. Its runtime classpath still
   selects the approved Gson 2.11.0 and Guava 33.3.1-jre, so these six JARs are
   compile-only inputs.
4. **Metadata-only:**
   - `kotlin-gradle-plugins-bom:2.1.20`;
   - `kotlinx-coroutines-bom:1.9.0`;
   - the `kotlinx-coroutines-core:1.9.0` multiplatform root, which redirects to
     the approved `-jvm` artifact;
   - 6 parent POMs whose paths are already present in root's reviewed captured
     metadata.

The 17 JARs total 67,130,536 bytes; `kotlin-compiler-embeddable` alone is
56,909,275 bytes.

## How the graph was obtained ([metadata/](metadata/))

The consumer mirrors the earlier reviewed metadata-only pattern:

- it applies only Gradle's builtin `java` and `java-gradle-plugin` plugins;
- it uses the same Gradle 9.3.1 distribution (`bin/gradle` SHA256 `e8aca393…`,
  as in 8a87389) and the same explicit JDK, 17.0.1-tem;
- auto-detect and auto-download are disabled;
- it runs with a fresh isolated GRADLE_USER_HOME and no build or configuration
  cache;
- no Kotlin, AGP or RN plugin is applied.

Runs:

1. Online with `--write-locks`, which fetched POM and module metadata only.
2. Offline: identical lockfiles and identical components and variants. The
   report differs only in the strict lock-constraint edges and "enforced by
   Dependency Locking" reasons that come from reading the lock.
3. A second offline repeat, byte-identical to the first offline run for the
   resolution report and both lockfiles.

The graph delta against the approved union (222 index coordinates plus the six
8a87389 coordinates) is in [graph-delta.json](graph-delta.json).

Check-only mirrors (not part of the request unless stated):

- The included build's main compile/runtime classpaths produced items 3 and 4.
- The consumer app's `debugCompileClasspath` mirror is **not authoritative**.
  Builtin-only resolution cannot reproduce AGP's own attributes or its
  compile/runtime version alignment without executing AGP, which this review
  must not do. The lower androidx versions it shows are therefore not
  proposed; the real graph must be compared at activation.

## Integrity and provenance ([verification.json](verification.json), [fetch-log.json](fetch-log.json))

All 99 closure files and the 6 parent POMs came from exact
`https://repo.maven.apache.org/maven2/` URLs over HTTPS with no redirect (HTTP
200).

- Every POM, module and JAR SHA-1 matches the publisher's `.sha1` sidecar.
- Every file Gradle had retrieved independently into its content-addressed
  cache is byte-identical to the separately fetched copy.
- Where publisher Gradle module metadata exists (`checker-qual`,
  `kotlinx-coroutines-core-jvm`), the JAR SHA256 and SHA512 match the
  publisher's declarations.
- Kotlin 2.1.20 artifacts publish POMs without `.module` files, so their pins
  rest on the publisher SHA-1 plus HTTPS.
- Detached `.asc` signatures were fetched for every JAR but **not verified**:
  no keyserver retrieval was authorized for this step. That verification is a
  recommended follow-up, as in the 8a87389 attestation addendum.

## Licenses and embedded components

POM-declared licenses:

- Apache-2.0: all Kotlin and kotlinx components, Gson, error_prone, j2objc.
- MIT: checker-qual.
- **LGPL-2.1: trove4j.** Its role is a build-time compiler dependency; it is
  not packaged into the app.
- Guava and failureaccess declare no license in their own POMs. They inherit
  from their reviewed parent POMs, and upstream Guava is Apache-2.0.

`kotlin-compiler-embeddable` carries no license files. It embeds relocated
third-party code: IntelliJ platform core, fastutil, Guava, kotlinx
immutable collections, ASM, unrelocated `io.vavr`, JLine, Aalto, Jansi, LZ4,
StreamEx, stax2, JDOM and GWT-derived code. The class-namespace inventory is in
verification.json. Kotlin's tag `v2.1.20` (commit `658a2010…`) supplies the
evidence in [upstream/](upstream/):

- `license/README.md` and `LICENSE.txt` (Apache-2.0) cover source-derived code;
- the version catalog declares Guava 33.3.1-jre, vavr 0.10.4, ASM 9.6.1,
  jdom2 2.0.6.1 and woodstox 7.1.0.

**Open:** the exact embedded versions and licenses are catalog-declared, not
byte-verified, and the IntelliJ core version is not in that catalog.

## Advisories ([osv-summary.json](osv-summary.json), [advisories/](advisories/))

Exact-version OSV queries returned matches only for **Guava 31.0.1-jre**:

- GHSA-5mg8-w23w-74h3 (CVE-2020-8908, LOW, temp-dir information disclosure);
- GHSA-7g45-4rm6-3mm3 (CVE-2023-2976, MODERATE, insecure temporary directory).

Both are fixed in 32.0.0. This Guava is a compile-only classpath entry for the
official plugin sources, and the plugin's runtime uses the approved 33.3.1-jre.

The other 19 closure coordinates, and the catalog-declared embedded versions
(queried for information only), returned no matches. This is a registry check,
not proof of safety. ADR-0035's original 47 build-only advisories, including 3
critical, remain disclosed and are not rescanned here.

## Proposed restricted activation (only after explicit operator approval)

1. Add exactly these hashes to `target/phase-seven-rn/android/prepare-repo.py`
   as an allowlist, copying from the quarantine. Rebuild the active repository
   fresh and verify every byte.
2. Switch the RN Android runner to the reviewed JDK 17.0.1-tem
   (`~/.sdkman/candidates/java/17.0.1-tem`). The current runner selects
   Homebrew `openjdk@17` 17.0.18, a discrepancy found during this review and
   not yet corrected.
3. Run only the offline graph step under the outbound-denied sandbox. Compare
   the actual selected build and runtime graphs with ADR-0035 + 8a87389 + this
   addendum, and stop on anything extra, including the app's real
   `debugCompileClasspath`.
4. Compile only after a coordinated heavy-work handback.

Forecast, not yet observed: AGP normally resolves a platform `aapt2` artifact
from Google Maven at execution, and the approved index has only `aapt2-proto`.
AGP's official `android.aapt2FromMavenOverride` property can point at the
installed SDK `build-tools/36.1.0/aapt2`, which needs no new dependency. If AGP
requests anything else at execution, it stops for review.

Restrictions: everything in ADR-0035 and 8a87389 stays binding (no JDK,
formatter or SDK download, no source fallback, no ktfmtFormat, no
publication). Rollback removes the unpublished repository entries; the
quarantine and all logs stay.

## Status

This is a concrete packet for independent review and for the explicit operator
decision required by ADR-0035. It is not approval, proves no compatibility or
runtime result, and invents no registry ID or ADR text.
