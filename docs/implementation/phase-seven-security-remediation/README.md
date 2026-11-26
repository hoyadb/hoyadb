# Phase 7 build-graph security remediation: triage and proposal

**Status: PROPOSAL ONLY.**
- Nothing is implemented, activated, staged or approved by this document.
- No JAR, Gradle, compiler or Kotlin daemon was executed. The only exception is
  one JDK `javap -c` parse of a single extracted class file, which loads nothing.
- Network use was read-only HTTPS: metadata, POMs, OSV records, tag-bound text
  files and one GitHub commit record. Every request is in
  [sources/fetch-log.txt](sources/fetch-log.txt), with HTTP code and SHA256.

The per-advisory machine-readable table is [triage.json](triage.json): 68 rows,
one per component/advisory pair.

## What was assessed

| Group | Source | Pairs (unique) |
| --- | --- | --- |
| ADR-0035 original native build graph, after the Okio runtime patch | `phase-seven-dependencies/native/advisory-impact.json` | 48 (47) |
| Guava 31.0.1-jre, compile-only in the compiler bundle | compiler addendum OSV | 2 |
| lz4-java 1.7.1 embedded in `kotlin-compiler-embeddable` 2.1.20 | compiler addendum embedded OSV | 8 |
| log4j 1.2.17 fork embedded in the compiler | same | 6 |
| protobuf 2.6.1 relocated in the compiler | same | 3 |
| **New:** commons-lang3 3.16.0 shaded in the approved KGP 2.1.20 JAR | this triage (byte-bound `pom.properties`) | 1 |

**Every item is build-time only.** No app or runtime configuration
(`androidDebug`, `androidRelease`, `kotlinSdkRuntime`) contains any of them.

## Applicability, from bytes and primary sources

**AGP-transitive dependencies (46 unique advisories).** All of them arrive
through AGP 8.12.0 ([sources/paths.txt](sources/paths.txt)):

| Component | Path from AGP 8.12.0 |
| --- | --- |
| Netty 4.1.110 (11 modules) | grpc-netty 1.69.1 |
| commons-compress 1.21 | sdklib |
| jose4j 0.9.5 | bundletool 1.18.1 |
| BouncyCastle 1.79 | sdk-common |
| jdom2 2.0.6 | jetifier |

- **No AGP patch release fixes any of these.** AGP 8.12.3 and 8.13.2 declare
  identical pins: grpc 1.69.1, bcpkix 1.79, bundletool 1.18.1, jetifier beta10
  ([sources/agp-patch-security-pins.json](sources/agp-patch-security-pins.json)).
- **Static caller scan** ([sources/agp-callers.txt](sources/agp-callers.txt)):
  - jose4j has a single caller, bundletool, which serves bundle and APK-set tasks.
  - jdom2 has a single caller, jetifier, and the consumer leaves
    `android.enableJetifier` unset (false by default in AGP 8).
  - Netty's only AGP caller is gRPC device/test tooling.
  - commons-compress serves SDK package handling, and SDK downloads are already
    disallowed.
  - BouncyCastle **is loaded** by ordinary debug builds (debug-keystore
    generation), but the advisory functions it is affected by are not fed
    untrusted input: GOST CTR, rfc822 name constraints, the LDAP store and
    hostile ASN.1.
  - Static caller scans are evidence of presence, not a proof of
    unreachability. Every one of these is remediated below rather than waived.

**KGP GHSA-r937 (CVE-2026-53914): not applicable.**
- The fix, commit `bf51df6` (KT-86604, "KAPT: Deserialize only a handful of
  classes from KAPT incremental cache"), changes only
  `plugins/kapt/kapt-base/.../JavaClassCacheManager`.
- That class is absent from all 203 approved and proposed JARs, and no
  kapt/annotation-processing artifact is in the union
  ([sources/symbol-scan-kapt.txt](sources/symbol-scan-kapt.txt)).

**Guava 31.0.1-jre: compile-only, and unused by sources.** The official RN
plugin's main sources import no Guava at all (one Gson use only). The plugin's
runtime classpath already selects the approved Guava 33.3.1-jre and Gson 2.11.0.

**Embedded lz4-java 1.7.1: no fixed Kotlin release exists.**
`versions.lz4-java=1.7.1` holds at v2.1.20, v2.1.21, v2.2.21, v2.3.21, v2.4.21
and master ([sources/kotlin-embedded-versions-by-tag.json](sources/kotlin-embedded-versions-by-tag.json)).
- **Only caller:** relocated IntelliJ `CompressionUtil`, using
  `LZ4Compressor`/`LZ4FastDecompressor`.
- **Factory selection:**
  - By default it calls `fastestJavaInstance()`.
  - `fastestInstance()` is used only when `-Didea.use.native.compression=true`
    ([javap excerpt](sources/CompressionUtil-factory-javap-excerpt.txt)).
  - So on the default path JNI is never loaded, and GHSA-mcr4 (native-library
    temp extraction) and GHSA-xx22 (JNI XXHash) are not on it.
- **Not called at all:** BlockInputStream, FrameInputStream and
  DecompressorWithLength are present in the JAR but have no embedded caller
  (GHSA-343h, -4v53, -gm45, -6cx8).
- **Reachable:** GHSA-vqf4 (CVE-2025-12183) and GHSA-cmp6 (CVE-2025-66566), via
  `CompressionUtil` ← IntelliJ `PersistentMapWal`/`CompressedAppendableFile` ←
  Kotlin incremental-compilation storage (`LazyStorage`/`AppendableLazyStorage`)
  ([sources/lz4-reachability.txt](sources/lz4-reachability.txt)). The data
  decompressed is the build's own local IC cache.

**Embedded log4j fork: not applicable.** Only `Level`, `Priority` and the lf5
config are present; every advisory-affected class is absent.

**Embedded protobuf 2.6.1-x: present.**
- It parses Kotlin metadata from compile-classpath inputs, which are all
  hash-pinned approved artifacts.
- Every Kotlin release through master ships `protobuf-relocated` 2.6.1-x.
- Whether that fork carries the upstream fixes is **unknown**.

**New, KGP-shaded commons-lang3 3.16.0 (GHSA-j288, CVE-2025-48924).**
- The approved KGP 2.1.20 JAR embeds commons-lang3 3.16.0, byte-bound by its
  `pom.properties`.
- `ClassUtils` is present, and reachable by type only from shaded
  commons-compress/io internals.
- No KGP code passes it untrusted class names.
- Kotlin's catalog declares commons-lang 3.18.0 only from v2.3.21, outside
  RN 0.86's Kotlin 2.1.20 pin, and those shaded bytes are not verified.
- The original 47-advisory scan never covered shaded components
  ([sources/embedded-poms.txt](sources/embedded-poms.txt)).

## Proposed remediations, smallest safe first

**R1 — Guava/Gson compile alignment (uses only approved components).**
- Add a reviewed, hash-pinned Gradle init script for the RN runner. It adds
  upgrade-only dependency **constraints**, with no `force` and no source edit:
  Guava 33.3.1-jre and Gson 2.11.0 on the official RN plugin's `compileClasspath`.
- Effects:
  - removes both Guava advisories;
  - aligns compile with the already-approved runtime;
  - drops six compile-only JARs (Guava 31.0.1, failureaccess 1.0.1,
    error_prone 2.7.1, j2objc 1.3, checker-qual 3.12.0 and Gson 2.8.9) from the
    approved compiler bundle, so the bundle needs a supersession record.
- The sources need no Guava API, so compatibility rests on the actual plugin
  compile.

**R2 — AGP-transitive fixed versions (new exact artifacts; operator decision).**
Upgrade-only constraints on the consumer's root `buildscript` classpath (a file
owned by the consumer/RN lane; AGP itself unchanged):

| Library | Version | Also new |
| --- | --- | --- |
| Netty, all 11 modules | 4.1.139.Final | — |
| commons-compress | 1.28.0 | commons-io 2.20.0, commons-lang3 3.18.0, commons-codec 1.19.0 |
| jose4j | 0.9.6 | slf4j-api may move from the approved 1.7.30 to 1.7.36 |
| BouncyCastle bcprov/bcpkix/bcutil | 1.85.2 | — |
| jdom2 | 2.0.6.1 | — |

- OSV returns zero matches for every candidate version
  ([sources/candidate-osv-result.json](sources/candidate-osv-result.json)).
- Each candidate stays on its existing major/minor line, but **no AGP release
  ships these combinations**. Compatibility must be proven by the actual AGP
  build, not assumed. App runtime configurations must stay byte-identical.

**R3 — LZ4: remove the reachable entry point (no fixed upstream exists).**
- Set `kotlin.incremental=false` for the consumer and the included build. This
  is a supported KGP property; build directories must be fresh.
- The runner rejects any `idea.use.native.compression` setting, keeping JNI off.
- Cost: slower Kotlin rebuilds.
- This is not patching or shading the compiler. Report upstream to JetBrains
  (outside this lane).

**R4/R5 — KAPT and log4j: no action beyond regression guards.** Keep the
vulnerable symbols proven absent.

**R6/R7 — protobuf-relocated and KGP-shaded commons-lang3: no supported fix
within RN 0.86's Kotlin 2.1.20 pin.**
- Options: disclosed residual acceptance, or a separate KGP major-upgrade
  decision. A major upgrade is not recommended: it is an RN compatibility change,
  and the shaded fix bytes are unverified.

**Gap fix:** add a permanent embedded/shaded-component scan to the dependency
gate.

## Proposed TDD cases (doc-first; RED before implementation)

| Case | Remediation | Assertion | Status today |
| --- | --- | --- | --- |
| SR-01 | R1 | Included-build `compileClasspath` selects Guava 33.3.1-jre and Gson 2.11.0; no Guava 31.0.1 or Gson 2.8.9 in any resolved configuration | RED |
| SR-02 | R1 | Official RN plugin compiles unchanged with the constraint; the init script is hash-pinned and touches only the named configurations | — |
| SR-10 | R2 | `nativeBuildTools`/buildscript graph selects exactly the R2 versions; no affected version remains anywhere | RED |
| SR-11 | R2 | OSV re-scan of the full resolved build graph has zero matches for R2 coordinates | RED |
| SR-12 | R2 | Actual consumer `assembleDebug`, including debug-keystore creation (BouncyCastle path), passes. `androidDebug`/`androidRelease` runtime graphs are identical to before | — |
| SR-20 | R3 | Runner passes `kotlin.incremental=false` to both builds and rejects `idea.use.native.compression` | RED |
| SR-21 | R3 | After a fresh build, no Kotlin IC storage (`build/kotlin/**/caches-jvm`) exists | — |
| SR-30 | R4/R5 | Symbol scan: KAPT `JavaClassCacheManager` and the affected log4j classes are absent from every approved JAR; the graph gate rejects any kapt/annotation-processing artifact | GREEN guard |
| SR-31 | R6/R7, gap fix | Embedded/shaded scan (pom.properties + catalog) is part of the gate; any new match blocks until it is triaged | RED (new) |

## Missing license texts (embedded compiler components)

**Version-bound Apache-2.0 texts found** ([sources/licenses/](sources/licenses/)):
lz4-java 1.7.1, Jansi 2.4.0, StreamEx 0.7.2, vavr 0.10.4 and
kotlinx-collections-immutable 0.3.7.

**Still open:**
- JLine 3.24.1 and IntelliJ core 233.13135.128: the tag paths tried return 404.
- JDOM: no version declared.
- The log4j fork, javax.inject 1 and relocated protobuf.

## Decisions needed before implementation

1. **R1 init script:** a new runner input that changes the approved bundle
   (supersession).
2. **R2 new artifacts:** an exact list like the compiler addendum, and actual AGP
   compatibility proof.
3. **R3:** disabling Kotlin IC and banning the native-compression property.
4. **R6/R7:** residual acceptance, or KGP major-upgrade research.
5. **Adding the embedded scan to the gate.**
