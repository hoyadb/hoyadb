# React Native Android follow-up verification

Recorded 2026-10-31 under SPEC-0047/TC-0051. This is current-source Codegen and
Java compilation evidence, not an ordinary React Native Android consumer result.
The task record is `/tmp/hoyadb-followup-20261031/rn-android/`.

## Executed checks

The existing scripts-disabled npm installation matched all 15,205 files in its
reviewed source snapshot before and after Codegen. Its lock SHA256
`c05cb6a927e86bd25a1a076df5e7d911745bad9773f21dca5e9149b153df8b2b`
matches the approved installation record. No reinstall, lifecycle hook or new
third-party input was used. A separate post-run inventory records 17 pre-existing
Gradle cache/report files outside that source snapshot; nine match the earlier
plugin-cache record and eight are under React Native's `.gradle/9.8.1`. Codegen
does not use those files, and this follow-up ran no Gradle command or enrolled
them as approved inputs. The permanent repository verifier also rechecked all
926 approved repo-v3 entries (264 binaries, 662 metadata); its plan digest is
`62ced3f9a2505aab20d9ebfcf0ee7ada03a852c388c7dcb0abfa84e325e9c23d`.

The two exact Codegen commands from ACTIVATION.md ran with network denied,
using approved Node22.18.0/RN0.86.3 tools and the current NativeHoyaDB.ts. They
produced the Android Java spec and four C++/CMake files. The generated Java
admission-release event takes no payload. `codegen.json` SHA256
`b7b1c2e4109a2dc1cd7f17fcdcd4dfe5cfbd68c015e1dcd5b40f23c7b2865d0a`
records commands, source/tool identity, generated hashes and unchanged inputs.

Installed reviewed JDK17 javac then compiled HoyaDBModule.java, HoyaDBPackage.java
and the freshly generated NativeHoyaDBSpec.java with `-Werror`, Java17 targets,
network denied and annotation processors disabled. All 80 runtime inputs were
rehashed against the independent approved artifact index. The classpath used the
fresh Kotlin policy-checkpoint AAR
`fa1658c258ab2b0eac08b70ed6656e3c722c42063a5a3dc00c794635078237a1`.
Compilation exited zero with six class files, no diagnostics and unchanged
source/input hashes. `javac.json` SHA256:
`6d1a8704e85be3ee4305a36ad9d80ed1656348e96a823b9dd850ef90ba7b8b0c`.
The initial task-local inventory reader encountered differing approved hash-field
names before launching javac; it was corrected to match coordinate plus the
recorded publisher hash. This was a tooling setup error, not a product RED.

## Remaining ordinary-consumer requirements

The [build-delta approval](phase-seven-build-delta-approval.json) authorized the
exact additional inputs, while preserving the separate compiler/advisory and
observation requirements. The current source audit confirms:

- SPEC-0034 still requires effective settings and native-library evidence for
  every compiling JVM. The independently reviewed
  [observation proposal](phase-seven-observation-contract-proposal/README.md)
  states that no complete mechanism for the stronger guarantee has been
  established. D1 would change that assurance; D2 separately accepts a scoped
  native-lifetime residual. Both are still recorded as pending. This follow-up
  does not adopt either change or enroll a baseline.
- The approved record leaves compiler-embedded CVE-2024-7254 and CVE-2022-3171
  findings open and requires independent native/cache baseline review. An
  ordinary dependency upgrade cannot be inferred from the current pinned RN
  graph; no new version or advisory acceptance was chosen here.
- The existing ignored Android controller still uses historical source paths
  and repo-v2, and does not wire the later observed-JVM/native-cache guards.
  Its security-handback flag is not evidence that those guards passed. No
  Gradle/compiler acceptance run was launched through it.
- The RN package tool independently requires non-framework archive members to
  match a pinned committed HEAD. Current relocated/modified working-tree files
  do not meet that release identity gate. No commit, source substitution or
  relaxation of that gate was made; the checks above compile explicit current
  source and are not a certified npm archive.

An ordinary Hermes APK, actual Gradle selection, JNI module installation and RN
consumer conformance remain unverified in this follow-up. The separate Kotlin
25-group actual AAR consumer is recorded in the
[Kotlin policy report](phase-seven-kotlin-policy-followup.md). Its result cannot
substitute for those RN requirements. The Codegen and Java checks establish
compatibility at their tested boundaries only; generated C++ was not compiled.
