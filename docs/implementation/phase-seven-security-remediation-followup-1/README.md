# Security remediation follow-up 1 (evidence; nothing activated)

Successor to the frozen initial triage cut (`phase-seven-security-remediation/`,
index `ffab198d…`, committed `52c45ca`), under SPEC-0034 / TC-0038 (`1915c25`,
`6b16bf6`). The initial cut is unchanged. This directory has its own
`files-sha256.json`.

**Constraints observed:**
- Quarantined bytes only. No JAR, Gradle, compiler, daemon or emulator was run.
- JDK `javap` was used only to parse single extracted class files.
- Every network request was read-only HTTPS and is in
  [fetch-log.txt](fetch-log.txt) and
  [r2/quarantine-fetch-log.json](r2/quarantine-fetch-log.json).

## Correction to the initial triage

The initial README proposed BouncyCastle **1.85.2** for bcprov/bcpkix/bcutil.
That candidate is invalid: only `bcprov-jdk18on` publishes 1.85.2, while
`bcpkix-jdk18on` and `bcutil-jdk18on` stop at 1.85 and then 1.86. The aligned
candidate is **1.85** for all three. It fixes all five BC advisories: 574f is
fixed in 1.84, and 9pwp, qp49, c3fc and the bcpkix wg6q advisory are fixed in
1.84/1.85. OSV is clean for it.

## R2: exact fixed-version closure

- **Roots** ([r2/r2-roots.txt](r2/r2-roots.txt)):
  - Netty 4.1.139.Final (11 modules);
  - commons-compress 1.28.0;
  - jose4j 0.9.6;
  - BC 1.85 ×3;
  - jdom2 2.0.6.1.
- **Resolution:** [r2/closure.json](r2/closure.json) is resolved by
  [closure.py](r2/closure.py.txt). It parses POMs only, with parents,
  properties, dependencyManagement and import BOMs, and uses compile/runtime,
  non-optional scopes.
- **Result:** 21 components with 19,808,511 bytes of JARs.
  - New transitives: commons-codec 1.19.0, commons-io 2.20.0, commons-lang3
    3.18.0 and slf4j-api 1.7.36.
  - Parent POMs: netty-parent 4.1.139.Final, commons-parent 85, apache 35,
    slf4j-parent 1.7.36 and oss-parent 9.
- **Integrity** ([r2/artifacts.json](r2/artifacts.json)):
  - Every `.sha1` sidecar matches.
  - Netty, BC and jdom2 also publish matching `.sha256`/`.sha512`;
    Apache-commons, jose4j and slf4j publish SHA-1/MD5 only.
  - Every `.asc` was fetched but not gpg-verified.
- **Contents:** no native code. Only netty-common shades another library,
  jctools-core 4.0.5, identified from `pom.properties`.
- **Advisories:** OSV over all 22 coordinates, including jctools, returns
  **zero matches** ([r2/r2-osv-result.json](r2/r2-osv-result.json)).
- **Blast radius:** upgrade-only selection also raises the approved
  commons-io 2.16.1, commons-codec 1.11 and slf4j-api 1.7.30 for every AGP
  consumer of them.
- **Status:** unapproved. Compatibility needs an actual AGP build and
  unchanged app runtime graphs (SR-10/11/12).
- **Licenses:**
  - Embedded LICENSE/NOTICE files in the BC, jdom2 and commons JARs.
  - Netty 4.1.139.Final LICENSE+NOTICE and slf4j 1.7.36 LICENSE at their tags
    ([licenses/](licenses/)).
  - jose4j 0.9.6 has no license file; its sources JAR carries Apache-2.0 headers
    (© Brian Campbell).

## R1: support closure (from approved bytes)

[r1/r1-closure.txt](r1/r1-closure.txt) shows what Guava 33.3.1-jre's
Gradle-module jre API variant and Gson 2.11.0's POM request:

| Module | Requested | Approved |
| --- | --- | --- |
| failureaccess | 1.0.2 | yes |
| listenablefuture | 9999.0-empty-to-avoid-conflict-with-guava | yes |
| jsr305 | 3.0.2 | yes |
| checker-qual | 3.43.0 | yes |
| j2objc-annotations | 3.0.0 | yes |
| error_prone_annotations | **2.28.0** (Guava) / **2.27.0** (Gson) | only 2.30.0 has a JAR |

The active repository holds 2.27.0 and 2.28.0 as POM-only metadata. The init
script therefore adds an upgrade-only `require 2.30.0` support constraint
(already approved). This is predicted from metadata: the actual resolved graph
is the SR-01 gate.

## Protobuf (compiler-embedded `protobuf-relocated`)

**The fork itself.**
- At Kotlin v2.1.20, `dependencies/protobuf/protobuf-relocated` is a mechanical
  Shadow relocation of upstream protobuf-java 2.6.1, with no patches.
- JetBrains KT-80597 (commit 8d859e1, 2025-09-02) backports upstream protobuf
  commits 562fc94 and 4728531 as **2.6.1-2**: a patched `CodedInputStream` that
  adds recursion checks to `skipMessage`. It first ships in Kotlin v2.3.0
  ([protobuf/kotlin-fork-versions-by-tag.json](protobuf/kotlin-fork-versions-by-tag.json)).
- Our 2.1.20 compiler's `CodedInputStream` has **no** `checkRecursionLimit`
  (javap), so it is unpatched.

**Runtime used.** The JAR ships the full runtime, but every Kotlin caller uses
the **lite** runtime: 96 metadata, 141 serialization and 184 backend users. No
Kotlin class references `GeneratedMessage` or `UnknownFieldSet`
([protobuf/protobuf-callers.txt](protobuf/protobuf-callers.txt)).

| Advisory | Status here |
| --- | --- |
| CVE-2021-22569 | No reachable caller. Its own advisory says javalite users are not affected, and the vulnerable path (full-runtime unknown-field parsing) is never called by Kotlin code. |
| CVE-2024-7254 | **Applicable and unpatched in 2.1.20.** Reachable via lite unknown-field skipping. Fixed in the fork from Kotlin 2.3.0. |
| CVE-2022-3171 | **Applicable to lite**, and the fork's patch does not address it. |

**Inputs.** The parsed input is Kotlin metadata (`.class` `@Metadata`,
`.kotlin_module`, klib/IR) from compile-classpath inputs and the build's own
outputs. Hash-pinning those inputs is a valuable guard. Per root, it is not a
fix and not proof of non-applicability. These advisories stay **open** pending
the supported compiler-version study.

## Shaded commons-lang3 3.16.0 in KGP (GHSA-j288, CVE-2025-48924)

[lang3/lang3-getclass-entrypoints.txt](lang3/lang3-getclass-entrypoints.txt):
- Only the four public `ClassUtils.getClass` overloads invoke `getClass`.
- Across all classes that reference `ClassUtils`, no class invokes any
  `getClass` overload.
- Across all 203 approved and proposed JARs (186 in the active repository),
  nothing outside the relocated `lang3` package references the relocated
  `ClassUtils`, including the reflective dotted-string form.

**Status:** not reachable by bytecode or string reference. This is evidence of
non-reachability for the packaged bytes, not a version fix.

## LZ4 entry points (R3 / SR-21)

[lz4/lz4-entrypoints.txt](lz4/lz4-entrypoints.txt) is the fixed-point
class-reference closure from `CompressionUtil`: depth 14, 269 classes. It
reaches:
- IC storage: `incremental/storage/*`, `IncrementalJvmCache`, `LookupStorage`,
  `InputsCache`;
- classpath snapshots: `classpathDiff/ClasspathChangesComputer`,
  `ClasspathSnapshotShrinker`, `AbiSnapshotDiffService`,
  `snapshots/FileSnapshotMap`;
- the incremental JVM compiler runners;
- the Kotlin daemon: `CompileServiceImpl`, `KotlinCompileDaemon`;
- the Build Tools API: `buildtools/internal/CompilationServiceImpl`.

`kotlin.incremental=false` alone is not proof. SR-20/21/22 guards now require
all of the following: the pinned in-process strategy, complete class and native
capture of every compiling JVM, absence of these entry points, and an
inventory-matched native-load record.

## Guards implemented (TDD; separate code handback)

Implemented under `bindings/react-native/tools/security/`, with RED/GREEN logs
in [guards/](guards/):
- RED: 38 tests, each failing on a named `NotImplementedError` stub, under
  normal and `-O`.
- GREEN: 38/38, under normal and `-O`.
- A static check confirms there is no `assert`, subprocess, network or `os`
  import in any module.
- The [real-input dry run](guards/real-input-dryrun.txt) reproduces the
  findings above from the actual JAR bytes. An empty namespace map correctly
  flags 21 unmapped relocated KGP namespaces (fail-closed); the real map and
  triage data are follow-up inputs.

## Still open

- **R2:** approval and an actual compatibility build.
- **Protobuf** CVE-2024-7254 and CVE-2022-3171: pending the inert
  compiler-version/BTA study (RN apiVersion 1.8; both CVEs).
- **LZ4 vqf4/cmp6:** pending the actual SR-21 observation.
- **Signatures:** gpg verification of the R2 `.asc` files.
- **Licenses still missing:** JLine 3.24.1, IntelliJ core 233.13135.128, JDOM,
  the log4j fork, javax.inject and relocated protobuf.
- **Scanner inputs:** the real namespace map and triage data.
