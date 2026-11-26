# Kotlin 2.1.20 compiler build addendum, version 2 (preparation only, awaiting approval)

This packet requests one combined decision for the Kotlin compiler build inputs
needed by:

- the unchanged official React Native 0.86.3 Android includeBuild;
- the RN consumer app's own Kotlin compilation.

**Nothing here is approved or active.**

- Nothing has been added to the active local repository
  (`target/phase-seven-rn/android/repo`) or to any build cache.
- No marker was applied.
- No new JAR or native library was loaded or executed.
- No Gradle compile ran.

App, SDK and core runtime pins are unchanged. Every byte stays quarantined
under `target/phase-seven-rn/compiler-review/` and
`target/phase-seven-rn/marker-review/`. The proposed scope is unchanged from
the initial cut: one marker POM, 17 JARs and the metadata-only items.

## Versions and reviews

| Item | Evidence |
| --- | --- |
| Initial cut (43 files) | index `a194306b7a6da538881643f5c2b6ea904e65c460001aa50a21eedf57bef91a18`; every original byte remains recoverable (see Presentation below) |
| Initial independent review (initial cut only) | root-owned `docs/implementation/phase-seven-kotlin-compiler-initial-review.json`, by agent 7518debe: "needs corrections (minor)", no material blocker in coordinates, hashes, variants or graph |
| This final version | adds follow-up 1 evidence and the review corrections; its own index is `files-sha256.json`. The initial review did **not** cover follow-up 1 |
| Final validation | [followup-1/revalidate-result.json](followup-1/revalidate-result.json) from [revalidate.py](followup-1/revalidate.py.txt), run with `python3 -I -O` (asserts disabled; every check is explicit) |

The review corrections are addressed as follows:

- **C1:** the installed-aapt2 override is now an explicit, exact, *proposed*
  activation option in `bundle.json` and below. It is not authorized.
- **C2:** the counts are fixed to 222 index artifact entries, 219 unique
  coordinates, and 225 with the six 8a87389 coordinates. `bundle.json`
  computes them.
- **Q1:** the JVM 17 mirror versus the real target-11 projects is disclosed,
  and the mandatory real-graph gate is restated.
- **Provenance notes:** each is answered in the follow-up section below.

## Why this is needed

The offline graph step stopped on the missing plugin marker
`org.jetbrains.kotlin.jvm:org.jetbrains.kotlin.jvm.gradle.plugin:2.1.20`. The
approved KGP 2.1.20 sources
([kgp-source-evidence.json](kgp-source-evidence.json), checked line by line by
the initial review) show that compiling Kotlin also resolves three unapproved
configurations:

| Configuration (KGP 2.1.20, gradle85 variant) | Declared dependency | Attributes |
| --- | --- | --- |
| `kotlinCompilerClasspath` | `kotlin-compiler-embeddable:${pluginVersion}` | none |
| `kotlinBuildToolsApiClasspath` | `kotlin-build-tools-impl`, strictly `2.1.20`; used by classpath-snapshot and script-discovery transforms | none |
| `kotlinCompilerPluginClasspath[Main/Debug…]` | `kotlin-scripting-compiler-embeddable:${pluginVersion}` from the unconditional scripting subplugin | Category library, Usage java-runtime, LibraryElements jar, GradlePluginApiVersion |

The commonizer classpath (native/multiplatform only) and the official plugin's
test configurations are excluded.

## Exact proposed additions ([bundle.json](bundle.json), version 2)

1. **Marker POM**, 1397 bytes, SHA256
   `5ccb905a796717218c1bb4eed8bb31fc674c5803cdd54c23753ead380f7a11b5`. Its bytes
   and its `.sha1`/`.md5` sidecars are now inside the packet in
   [marker/bytes/](marker/bytes/). Its only dependency is the approved
   `kotlin-gradle-plugin:2.1.20`. The frozen `marker/PACKET.md` still links to
   quarantine paths outside the packet; those links are historical.
2. **Kotlin compiler closure, 11 JARs:**
   - `kotlin-compiler-embeddable`
   - `kotlin-daemon-embeddable`
   - `kotlin-build-tools-impl`
   - `kotlin-scripting-compiler-embeddable`
   - `kotlin-scripting-compiler-impl-embeddable`
   - `kotlin-scripting-common`
   - `kotlin-scripting-jvm`
   - `kotlin-script-runtime` (all `2.1.20`)
   - `kotlin-reflect:1.6.10`
   - `trove4j:1.0.20200330`
   - `kotlinx-coroutines-core-jvm:1.8.0`
3. **Official RN plugin compile classpath, 6 JARs:**
   - `gson:2.8.9`
   - `guava:31.0.1-jre`
   - `failureaccess:1.0.1`
   - `error_prone_annotations:2.7.1`
   - `j2objc-annotations:1.3`
   - `checker-qual:3.12.0`

   These are compile-only. The plugin's runtime classpath selects the approved
   2.11.0, 33.3.1-jre, 1.0.2, 2.30.0, 3.0.0 and 3.43.0.
4. **Metadata-only:**
   - `kotlin-gradle-plugins-bom:2.1.20`;
   - `kotlinx-coroutines-bom:1.9.0`;
   - the `kotlinx-coroutines-core:1.9.0` multiplatform root;
   - six parent POMs, byte-identical to root's reviewed captured metadata
     ([parents-match](followup-1/provenance/parents-match.json)).

The 17 JARs total 67,130,536 bytes.

## How the graph was obtained ([metadata/](metadata/))

The consumer used:

- only Gradle's builtin `java` and `java-gradle-plugin` plugins;
- Gradle 9.3.1 (`bin/gradle` `e8aca393…`);
- JDK 17.0.1-tem, with auto-detect and auto-download off;
- a fresh isolated Gradle home and no build or configuration cache.

Runs:

1. Online with `--write-locks`.
2. Offline: identical lockfiles, components and variants. The reports differ
   only by the lock-constraint edges.
3. A second offline repeat, byte-identical to the first offline run.

The offline runs relied on Gradle `--offline`, not an OS network block. The
graph delta against the 225-coordinate approved union is in
[graph-delta.json](graph-delta.json).

**Q1, disclosed:**

- The included-build mirror ran on JVM 17 and requested
  `org.gradle.jvm.version=17`.
- The real plugin projects target Java 11
  (`react-native-gradle-plugin/build.gradle.kts:61`,
  `shared/build.gradle.kts:30`, `settings-plugin/build.gradle.kts:51`).
- No external selected variant declares a version above 11
  ([q1-jvm-version-check](followup-1/provenance/q1-jvm-version-check.json)), so
  the selection is unchanged.

**Not authoritative:** the app `debugCompileClasspath` mirror. It shows 11
components outside the union, none of which are proposed. The app runtime/APK
graph is not mirrored by this packet. Builtin-only resolution cannot represent
AGP's attributes and alignment.

**Mandatory gate:** before any compile, the actual KGP- and AGP-applied build
and runtime graphs must equal the ADR-0035 union, plus 8a87389, plus this exact
bundle. Anything extra stops activation and may need a further decision.

## Follow-up 1: provenance, signatures, embedded contents ([followup-1/](followup-1/))

- **Strong sidecars** ([strong-sidecars](followup-1/provenance/strong-sidecars.json)):
  every closure file, every parent and the marker were checked for `.sha256`
  and `.sha512`.
  - 9 files publish them, all matching: the checker-qual and kotlinx-coroutines
    files.
  - All others, including every Kotlin 2.1.20, Google, parent and marker file,
    return 404 for both. Their publisher integrity therefore rests on SHA-1 plus
    HTTPS, now as a checked fact.
  - `.md5` sidecars were not tried for the JARs.
- **Signatures** ([attestation/](followup-1/attestation/)): all 17 JAR detached
  signatures verify mathematically (GOODSIG and VALIDSIG), with no expired or
  revoked status reported.
  - Method: existing GnuPG 2.5.21 (`76fb0766…`) in a fresh short owned
    public-only homedir, with no options file, no agent autostart and no
    automatic key retrieval.
  - Twelve signatures were verified by exact issuer fingerprint. Five carry
    only a 64-bit key ID; they were looked up by key ID and confirmed by the
    full VALIDSIG fingerprint.
  - Signers include Kotlin Release `6F538074CCEBF35F28AF9B066A0975F8B1127B83`
    (the same key as in 8a87389).
  - **Identity trust is UNKNOWN** (public-keyserver keys; no publisher-hosted
    keys found).
  - Two signatures (j2objc, checker-qual) use SHA-1 digests.
  - trove4j is signed by the **Bintray repository service key**, not its
    author.
- **Embedded compiler contents** ([embedded/](followup-1/embedded/)). Primary
  sources at Kotlin tag `v2.1.20` (commit `658a2010…`): the exact
  `fatJarContents` list in `prepare/compiler/build.gradle.kts`,
  `gradle/versions.properties` and the `license/third_party` index.
  - [byte-presence.json](followup-1/embedded/byte-presence.json) gives a
    per-component namespace inventory, finer than the initial inventory.
  - **Present in bytes:** IntelliJ core, fastutil, Guava, ASM, vavr, JLine,
    Jansi, Aalto, stax2, LZ4, StreamEx, kotlinx-collections-immutable, JDOM,
    javax.inject and relocated protobuf. log4j is limited to `Level`,
    `Priority` and one lf5 config file. Not found: oro, JNA (excluded by the
    build), jsr305.
  - **Versions are source-declared, not byte-verified:** for example
    intellijSdk 233.13135.128, lz4-java 1.7.1, fastutil 8.5.4-9 and
    protobuf-relocated 2.6.1-1. The JAR manifest says only
    `2.1.20-release-217`.
  - **License texts available** for Aalto, ASM, fastutil, Guava, GWT, stax2 and
    Trove.
  - **Open:** no license text for IntelliJ core, JLine, Jansi, LZ4, StreamEx,
    vavr, kotlinx-collections-immutable, JDOM, the log4j fork, javax.inject or
    relocated protobuf. This is not an SBOM or legal certification.
  - **Native code** ([native-libraries.json](followup-1/embedded/native-libraries.json),
    20 files hashed): LZ4 JNI for Linux and Windows only (none for macOS), and
    Jansi including macOS `.jnilib`. Whether an in-process compile loads Jansi
    is unproven without execution.
  - **Advisories for source-declared embedded versions** (informational;
    applicability to the relocated copies unverified):
    - lz4-java 1.7.1: 8 matches, HIGH: CVE-2025-12183, CVE-2025-66566,
      CVE-2026-106451; all fixed by 1.11.4;
    - upstream log4j 1.2.17: 6 matches, 3 critical; the vulnerable classes are
      absent from the bytes;
    - upstream protobuf-java 2.6.1: 3 DoS matches;
    - IntelliJ core was not queried. No "no advisory" conclusion is drawn for
      unchecked embedded versions.
- **Closure advisories** ([osv-summary.json](osv-summary.json)): only Guava
  31.0.1-jre (CVE-2020-8908 LOW, CVE-2023-2976 MODERATE, fixed in 32.0.0;
  compile-only). ADR-0035's original 47 build-only advisories (3 critical)
  remain disclosed and were not rescanned.
- **Tools:**
  - The initial tool copies in [tools/](tools/) are unchanged. They ran without
    `-O`, so their asserts were active when the evidence was produced.
  - [tools-v2/](followup-1/tools-v2/) replaces every assert with an explicit
    check.
  - The frozen marker-review scripts keep asserts; the `-O` revalidation
    re-checks the marker independently.

## Proposed restricted activation (all part of one future exact-bundle operator decision; none authorized)

1. **Repository:** add exactly the hashes in `bundle.json` to
   `target/phase-seven-rn/android/prepare-repo.py`, copying from the quarantine.
   Rebuild the active repository fresh and verify every byte.
2. **JDK, mandatory runner correction:** use the reviewed
   `~/.sdkman/candidates/java/17.0.1-tem` (java binary `2c663a2f…`). The
   current runner selects Homebrew `openjdk@17` 17.0.18 and must change first.
3. **aapt2 override, option for the operator:**
   - `android.aapt2FromMavenOverride` set to
     `~/Library/Android/sdk/build-tools/36.1.0/aapt2`;
   - SHA256
     `347ae46c83960aed14dd88e132f80a82c1b3456000384d234cee23c412533b03`,
     version `2.20-14042983`;
   - the property name and AGP's own recommendation for a local AAPT2 are in
     the approved AGP 8.12.0 bytes
     ([aapt2-and-jdk](followup-1/activation-confirmation/aapt2-and-jdk.json));
   - it avoids fetching the unapproved platform aapt2 Maven artifact (AGP
     default `8.12.0-13700139`);
   - whether AGP accepts the local 2.20 build is unverified.
4. **Graph step:** run the offline graph step only, under the outbound-denied
   sandbox, then apply the mandatory gate above.
5. **Compile:** only after a coordinated heavy-work handback.

All ADR-0035 and 8a87389 restrictions stay binding: no JDK, formatter or SDK
download, no source fallback, no ktfmtFormat, no publication. Rollback removes
the unpublished repository entries; the quarantine and logs stay.

## Presentation (lossless, original bytes preserved)

[presentation.json](presentation.json) maps all 43 initial-cut paths, and
artifact digests are kept separate from presentation digests:

- **Unchanged:** files with no change stay byte-identical at their paths.
- **Superseded:** `README.md`, `bundle.json` and `files-sha256.json` are
  superseded. Their originals are byte-identical in [initial-cut/](initial-cut/).
- **Wrapped:** the seven originals that fail whitespace checks are kept as
  exact bytes plus SHA256 in JSON wrappers:
  - the three `metadata/*/resolution.json` files, as `resolution.original.json`;
    each also has a readable view that parses equal to the original JSON;
  - the three `output.txt` files, as `output.txt.json`;
  - `upstream/README.md`, as `README.md.json`.

  The original online/offline byte-equality statements refer to the original
  bytes in those wrappers.
- **Follow-up text:** follow-up files that fail whitespace checks, such as
  keyserver keys and license texts, are wrapped the same way.
- **Final index:** `files-sha256.json` indexes every file of this final version.

## Status

This is the final preparation packet, for root review and the explicit operator
decision required by ADR-0035. It is not approval, proves no compatibility or
runtime result, and invents no registry entry or ADR text.
