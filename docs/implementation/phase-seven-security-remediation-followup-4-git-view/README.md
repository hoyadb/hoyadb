# Security remediation follow-up 4: review corrections, real gate data, R2 signatures

These are the corrections for the five issues confirmed by the focused Claude
review of the initial guards, plus the minor scanner-CLI item. All stay within
SPEC-0034 / TC-0038 under root's GO. No Gradle, compiler or JAR was executed.

Earlier cuts are frozen and unchanged:

| Cut | Index |
| --- | --- |
| initial | `ffab198d` |
| follow-up 1 | `1dcf7f71` |
| follow-up 2 | `fc60b725` |
| follow-up 3 | `35561dde` |

This cut has its own `files-sha256.json`.

## Presentation correction (follow-up 2 README, line 9)

Follow-up 2's README renders the root reproduction key as plain
`systemProp.idea.use.native.compression=true`. That is **wrong**.

- The actual probe key contained a **literal backslash followed by `u0069`**:
  the bytes `5c 75 30 30 36 39` in place of the `i` of `idea`.
- Java `Properties` decodes that escape to `i`, which is why the initial guard
  missed it.
- The test in `test_kotlin_guard.py`
  (`test_properties_unicode_escaped_key_and_value`) uses the literal escaped
  form.
- This record supersedes the rendering only; follow-up 2's bytes and index are
  preserved.

## Review issues → RED → fix → GREEN

RED logs are in [logs/](logs/); each failed under both normal and `-O`.

1. **Unicode-escaped key.** Fixed in follow-up 2 (the parser port). This cut
   only corrects the presentation, above.
2. **Vacuous observed-JVM certification.**
   - RED: 2 failures in `test_kotlin_guard`.
   - Now rejected:
     - empty `required_roles`;
     - an observation with a missing or empty effective `system_properties`
       map;
     - an observation with a missing `input_arguments` list ("incomplete
       capture").
3. **Scanner dropped flat two-segment packages.**
   - RED: the flat/nested accounting test.
   - Fix: the key is now the package path `rest[:-1]`, capped at three
     segments, and every relocated third-party class is counted.
   - An additional RED (`red2-*`) found a real gap: relocation prefixes were
     first-match. They are now **longest-first**, with `''` for unrelocated
     packages and `org/jetbrains/` for alternate roots. This covers the
     compiler's unrelocated `io/vavr` and `org/jetbrains/org/objectweb/asm`.
   - Real dry run ([embedded-gate/real-input-dryrun.txt](embedded-gate/real-input-dryrun.txt)):
     KGP `org/jdom/` 9 classes; compiler `javax/inject/` 6 and
     `org/picocontainer/` 3, as the review predicted.
   - Known blind spot, documented in the API README: a relocation that drops
     the original root (such as `org/jetbrains/kotlin/protobuf`) is found only
     through the explicit namespace map.
4. **Init-script guard gaps.**
   - RED: 3 failures and 1 error.
   - It now requires the **exact** reviewed constraint set; deleting the Guava
     or error_prone pin rejects.
   - Comment handling is now string-aware (single/double/triple-quoted strings,
     `//`, `/* */`). Unterminated input rejects, so a `//` inside a quoted URL
     can no longer hide a following `resolutionStrategy.force`.
   - New `verify_pinned()` checks `R1_SCRIPT_SHA256 =
     74aa545a6c82cbdc434e71328653979715659818603a6c0d60627245e8d1f9a0` before
     Gradle. This is the pin for the RN owner. It is not a general Gradle
     parser.
5. **Durable source binding.** [source-manifests.json](source-manifests.json)
   records the initial 12-file handback, the follow-up 2 handback and the final
   follow-up 4 sources, plus the init-script pin.
   [final-green/green-run-identity.json](final-green/green-run-identity.json)
   binds the final GREEN run: Python 3.14.8 (interpreter SHA256 recorded),
   exact commands, per-suite exit codes and test counts, and log SHA256s.
   Sources were identical before and after the run.

**Minor:** an unreadable or missing scanner container now exits 2 with a
structured "unreadable container" rejection; it no longer reports "open".

**Final GREEN:** all four suites pass under normal and `-O`, 55 tests per mode
(kotlin 26, symbol 9, embedded 11, init 9).

## Real embedded-gate data (KGP + compiler)

[embedded-gate/embedded-gate-plan.json](embedded-gate/embedded-gate-plan.json)
contains the namespace map, source-declared versions and triage.
- Byte-bound versions come from embedded `pom.properties`.
- Source-declared versions come from Kotlin v2.1.20 `versions.properties` and
  the catalog.
- Forks are scanned with their upstream base's advisories, labeled as such.

[embedded-gate-report.json](embedded-gate/embedded-gate-report.json) is the
truthful result: **status rejected, exit 2**.
- 32 components: 18 source-declared, 10 byte-bound, 4 unversioned.
- Rejections:
  - IntelliJ core 233.13135.128 has no OSV-scannable coordinate (×2: KGP and
    compiler);
  - the IntelliJ JDOM fork (×2), the IntelliJ picocontainer fork and the
    GWT-derived sources have **unknown versions**.
- Open findings:
  - lz4-java 1.7.1: vqf4, cmp6, mcr4, xx22;
  - protobuf-relocated 2.6.1-1: CVE-2024-7254, CVE-2022-3171.
- Dispositioned not-applicable, each with an evidence reference:
  - lz4 343h, 4v53, gm45 and 6cx8 (no embedded caller);
  - protobuf CVE-2021-22569 (lite-only use);
  - the log4j fork (affected classes absent);
  - KGP commons-lang3 GHSA-j288 (no caller).
- Nothing is mapped by invention; unknowns stay explicit.
- A first run with relative paths from the wrong working directory was
  correctly rejected as an unreadable container. It is preserved as
  `embedded-gate-report-relative-path-unreadable.json`.

## R2 signatures (authorized keyserver GET + fresh public-only gpg homedir)

[r2-signatures/r2-signatures.json](r2-signatures/r2-signatures.json):
- gpg 2.5.21 (SHA256 recorded).
- Fresh `/tmp` homedir with no options file, no agent autostart and no
  auto-retrieve; it was removed afterwards. No user keyring was touched.

Results:
- **19 of 21 GOODSIG + VALIDSIG:**
  - Netty ×11, Netty Project Bot `0D35D3F6…F3D1600878E85A3D`, **SHA-1 digest**;
  - commons-compress/-codec/-io/-lang3, Gary D. Gregory `2DB4F1EF…A11262CB`;
  - BC ×3, `7B121B76…E3A748C0`;
  - slf4j-api, Ceki Gülcü `475F3B8E…A511E325`, **SHA-1 digest**.
- **jose4j 0.9.6 and jdom2 2.0.6.1: EXPKEYSIG with VALIDSIG.** Each signature
  verifies, and each key was valid when it signed:
  - jose4j signed 2024-03-06; key expired 2024-05-31.
  - jdom2 signed 2021-12-06; key expired 2023-12-06.
- **Identity trust is UNKNOWN for all**: keys come from public keyservers.
- The first run failed for Netty and slf4j because keys.openpgp.org served
  UID-stripped keys that gpg cannot import. It is preserved as
  `r2-signatures-run1-uidless-keys.json`; the second run used the
  keyserver.ubuntu.com fallback.

## Code-reuse provenance (decision for root; no legal conclusion asserted)

The follow-up 2 parser in `kotlin_guard.py` (`_logical_lines`, `_convert`,
`_properties`) is a **Python translation written while reading** the pinned
JDK 17.0.1 `java.util.Properties` source (`lib/src.zip`
`ca831828…`). That file's header reads "Copyright (c) 1995, 2020, Oracle …
GNU General Public License version 2 only … subject to the 'Classpath'
exception" ([jdk17-Properties.java-header.txt](jdk17-Properties.java-header.txt);
the JDK's `legal/java.base/ASSEMBLY_EXCEPTION` is copied alongside).

The Classpath Exception covers linking, not translated source. Frozen
follow-up 2 also contains **verbatim** excerpts of `load0` and `loadConvert`
without the header.

Proposed handling, in order of preference:
- **(A) Clean-room reimplementation.** Rewrite the parser from the public
  `Properties.load(Reader)` API specification only, written by someone who has
  not used the translation, with the existing behavioral tests as the oracle.
  Record that clean-room provenance.
- **(B) Keep the translation** with explicit GPL-2.0-only + Classpath-Exception
  attribution and an operator licensing decision for the repo tooling.

For the frozen follow-up 2 excerpts:
- **(C)** Add a superseding record carrying the full header, or
- **(D)** have root decide whether the excerpts should be removed in a new
  commit.

I have not changed the code or the excerpts pending root's choice.

## Still open

- protobuf CVE-2024-7254 and CVE-2022-3171; lz4 vqf4/cmp6/mcr4/xx22 (pending
  SR-21 observation);
- the IntelliJ core OSV gap; unknown JDOM, picocontainer and GWT versions;
- the R2 approval decision;
- the provenance decision above;
- SR-32 is not started, per root.
