# Security remediation follow-up 3: inert compiler-version/BTA study and licenses

Inert study: primary sources and metadata only. No compiler, Gradle or JAR was
executed. Predecessors stay frozen: initial `ffab198d`, follow-up 1 `1dcf7f71`
and follow-up 2 `fc60b725`. Every request is logged in
`target/phase-seven-security-remediation/fetch/fetch-log.txt`.

## Question

Does a supported Kotlin compiler version fix the two **applicable** embedded
protobuf advisories (CVE-2024-7254 and CVE-2022-3171) for the unchanged
RN 0.86.3 build? The official RN plugin uses KGP 2.1.20 with `apiVersion 1.8`
and `jvmTarget 11`.

## Facts

1. **Fork patch** (follow-up 1). JetBrains KT-80597 backports upstream
   CVE-2024-7254 commits into `protobuf-relocated` **2.6.1-2**, which first
   ships in Kotlin **v2.3.0**. v2.2.20/v2.2.21 still use 2.6.1-1. The patch
   changes only `CodedInputStream` recursion accounting.
2. **CVE-2022-3171 precondition is present in every Kotlin version checked.**
   - Kotlin's generated lite metadata parsers re-merge each repeated occurrence
     of a singular embedded message with `toBuilder()` → `mergeFrom` →
     `buildPartial()`. That is the mutable/immutable round-trip the advisory
     describes.
   - Sites: **27 at v2.1.20** and **30 at v2.3.21**
     ([sources/cve-2022-3171-precondition.json](sources/cve-2022-3171-precondition.json)).
     The fork patch does not touch this, so **no Kotlin release fixes it**.
3. **Language/API bounds** (`compiler/util/.../LanguageVersionSettings.kt`;
   [sources/language-version-bounds-by-tag.txt](sources/language-version-bounds-by-tag.txt)):

   | Compiler | FIRST_API_SUPPORTED | FIRST_SUPPORTED |
   | --- | --- | --- |
   | 2.1.20 | 1.6 | 1.6 |
   | 2.2.21 | 1.8 | 1.8 |
   | 2.3.21 | **1.9** | 2.0 |

   A 2.3.x compiler therefore **cannot compile the unchanged official plugin**,
   which sets `apiVersion 1.8`.
4. **BTA** (KGP 2.1.20 sources + current Kotlin docs;
   [sources/](sources/)):
   - KGP 2.1.20 exposes the experimental `kotlin { compilerVersion }`, honored
     with `kotlin.compiler.runViaBuildToolsApi=true`.
   - The documented support is three previous majors and **one major forward**
     (KGP 2.2.0 → up to 2.3.x). KGP 2.1.20 therefore reaches at most 2.2.x,
     whose fork is unpatched.
   - The docs also warn that compiler plugins (KGP adds the scripting plugin
     unconditionally) may fail with a different compiler version.
5. KGP 2.1.20 registers `BuildToolsApiClasspathEntrySnapshotTransform`
   unconditionally. Whether it executes in the selected workflow must be
   observed under SR-21.

## Result

| Route | CVE-2024-7254 | CVE-2022-3171 | Unchanged RN 0.86 plugin |
| --- | --- | --- | --- |
| KGP 2.1.20 + BTA 2.2.x | not fixed (2.6.1-1) | not fixed | compiles (apiVersion 1.8 deprecated but supported) |
| Compiler ≥ 2.3.0 (needs KGP ≥ 2.2 via BTA, or KGP ≥ 2.3) | fixed in the fork | **not fixed** | **cannot compile** (apiVersion 1.8 below 1.9) |

Within RN 0.86 and the unchanged official plugin, **no supported compiler
version fixes either applicable protobuf advisory.** Both stay **OPEN**.

Closing CVE-2024-7254 by version would need an RN-level upgrade whose official
plugin targets apiVersion ≥ 1.9, together with KGP ≥ 2.3. That is a separate,
larger compatibility scope, proposed here and **not executed**. CVE-2022-3171
has no Kotlin fix at any version.

## Possible compensating guard (needs a SPEC/TC addition before any code)

**SR-32 (proposed): a pre-compile protobuf-precondition scan of the exact pinned
compile inputs.**
- Decode every Kotlin protobuf payload the compiler will parse from the
  selected classpath: `@kotlin.Metadata` d1, `META-INF/*.kotlin_module` and
  `*.kotlin_builtins`.
- Reject if unknown-group nesting or repeated singular-message counts exceed
  reviewed bounds.
- This gives input-specific evidence that the exact pinned inputs do not
  trigger either precondition. It is not a fix, and it must not be presented as
  non-applicability. It needs a re-scan whenever the inputs change.

## Licenses (version-bound where the primary source exists; [licenses/](licenses/))

| Component | Found |
| --- | --- |
| JLine 3.24.1 | BSD-3-Clause `LICENSE.txt` at tag `jline-parent-3.24.1` (the earlier `jline-3.24.1` tag path does not exist) |
| protobuf 2.6.1 (base of the relocated fork) | BSD-3 `LICENSE` at upstream tag `v2.6.1` |
| log4j 1.2.17 (base of the JetBrains fork 1.2.17.2) | Apache-2.0 `LICENSE` + `NOTICE` at upstream tag `v1_2_17`; the fork's own POM is not on Maven Central |
| javax.inject 1 | Apache-2.0 header from the previously reviewed sources JAR (copied unchanged) |

**IntelliJ core 233.13135.128 is not exact.** No public intellij-community tag
exists for that build; the nearest tags are 233.13135.103 and .65. The
233.13135.103 `LICENSE.txt` and `NOTICE.txt` are included and labeled
**non-exact candidates**.

**JDOM is not 2.0.6.1.** The compiler/KGP embed `org/jdom/*`, IntelliJ's
JDOM fork in the JDOM-1 namespace (KGP even has IntelliJ's
`ImmutableAttribute`), not `org.jdom2`. The fetched `jdom-2.0.6.1-LICENSE.txt`
is therefore **not** attributed to the embedded component, whose version and
license source remain open.
