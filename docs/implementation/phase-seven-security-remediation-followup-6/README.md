# Security remediation follow-up 6: R2 packet corrections (B1–B7)

This packet answers reviewer findings B1–B7 on the R2 fixed-version closure
packet and narrows the protobuf/LZ4/lang3 study claims. It contains evidence
only. Nothing was activated:
- no Gradle, compiler or JAR was executed;
- no dependency graph was resolved by a build tool;
- nothing is staged.

SR-32 was not started.

Predecessors stay frozen, each under its own index: initial `ffab198d`,
follow-ups 1 and 3, the follow-up 2/4 git views, and follow-up 5. The
superseded R2 artifacts are preserved under the ignored
`target/phase-seven-security-remediation/` and are not rewritten:
- `closure.json`, `artifacts.json`, `quarantine/`;
- `quarantine-fetch-log.json`;
- `r2-signatures.json` and `r2-signatures-run1-uidless-keys.json`.

The allowlist's `quarantine_path` values are relative to that directory.

## Contents

| Path | What |
| --- | --- |
| `evidence/r2-roots.txt` | The 17 exact R2 roots |
| `evidence/closure-v2.json` | Resolver v2 output: 21 components, 44 edges, parents, `import_boms` |
| `evidence/artifacts-v2.json` | Per-file bytes, SHA-1/256/512, sidecar comparison, `.asc` presence |
| `evidence/quarantine-v2-fetch-log.json` | All 336 v2 requests (262 × 200, 74 × 404) |
| `evidence/quarantine-v2-aborted-1-fetch-log.json` | The 3 requests from the aborted first v2 run |
| `evidence/r2-signatures-v2.json` | gpg status lines per file, key lookups, key hashes |
| `evidence/r2-allowlist-v2.json` | **Proposed** exact allowlist (B7) |
| `evidence/r2-blast-radius-v2.json` | Repo-v2 versions affected by upgrade-only selection (B6) |
| `evidence/r2-netty-notice-v2.json`, `evidence/netty-4.1.139.Final-NOTICE.txt` | Netty NOTICE record (B5) |
| `osv/` | OSV querybatch inputs and results for the R2 coordinates and junit-bom |
| `keys/` | Public keys fetched for verification (public material only) |
| `resolver/` | Resolver v1 and v2 sources, `test_closure.py`, RED/GREEN logs, v1/v2 byte comparison |
| `tools/` | `artifacts_v2.py` and `r2_gpg_v2.py` as run (`.txt`, not executable here) |

## B1: junit-bom 5.13.1 metadata

`commons-compress` 1.28.0 → `commons-parent` 85 imports `org.junit:junit-bom:5.13.1`
(`scope=import`, `type=pom`). This is metadata that a Gradle resolution also
reads. It is now fetched, hashed and verified:

| File | Bytes | SHA256 | Sidecars | Signature |
| --- | --- | --- | --- | --- |
| `junit-bom-5.13.1.pom` | 5649 | `fa68451e…a65f3` | md5/sha1/sha256/sha512 match | GOODSIG, `FF6E2C00…1F425EC61B51` (Marc Philipp) |
| `junit-bom-5.13.1.module` | 6994 | `33c07ab9…c75de8` | md5/sha1/sha256/sha512 match | GOODSIG, same key |

The POM carries the Gradle-metadata marker, so Gradle prefers the `.module`.
Both files are therefore in the allowlist. An OSV query for
`org.junit:junit-bom:5.13.1` returned `{"results":[{}]}`
([osv/junit-bom-osv-result.json](osv/junit-bom-osv-result.json)), which is a
point-in-time result.

## B2: resolver dropped namespace-less POMs

v1 matched elements only in the Maven POM namespace. BouncyCastle and jdom2
POMs have no `xmlns`, so their dependencies, licenses and parents were
silently lost.

**TDD.** The tests are in [resolver/test_closure.py.txt](resolver/test_closure.py.txt).
They are synthetic and offline, with no network.
- RED 1 ([log](resolver/logs/red-test_closure.txt), `-O` the same) failed for
  the expected reasons: `AssertionError: Lists differ: [] != [('x', 'dep', '2')]`
  and `AttributeError: module 'closure' has no attribute 'imports'`.
- The fix strips the namespace from every tag before lookup and records
  import BOMs in `imports`.
- The first real v2 run aborted on a namespaced `<properties>` child. That
  abort is preserved: the log is in `evidence/` and the directory is
  `quarantine-v2-aborted-1/` in target.
- RED 2 ([log](resolver/logs/red2-test_closure.txt)) reproduced it as
  `IndexError: list index out of range`. The fix is `split('}')[-1]`.
- GREEN: 3/3 tests pass, normal and `-O`
  ([green2](resolver/logs/green2-test_closure.txt),
  [green2 -O](resolver/logs/green2-O-test_closure.txt)). I re-ran both modes
  before writing this packet: OK.

**Effect.**
- Edges went from 42 to 44 (`bcpkix→bcutil` and `bcutil→bcprov`, compile).
- Licenses were recovered:
  - BC: "Bouncy Castle Licence";
  - jdom2: "Similar to Apache License but with the acknowledgment clause
    removed".
- The component set is unchanged at 21.

## B3: v1 fetch-log gap (acknowledged)

26 v1 POMs in `quarantine/` have **no primary-fetch record** in
`quarantine-fetch-log.json`.

**Root cause.** v1 wrote its log only at the end of a run. The first v1 run
aborted, so its requests were never logged, and the second run read those
POMs from disk without a request.

**No records were invented.** v2 writes the log after **every** request, uses a
fresh `quarantine-v2/`, and re-fetched everything.

The comparison is in
[resolver/logs/v1-v2-byte-comparison.txt](resolver/logs/v1-v2-byte-comparison.txt):
- v2 has 262 files and v1 has 252; 251 are common, and **all 251 are
  byte-identical**.
- Only in v2: the 11 junit-bom files (B1).
- Only in v1: the stray `bcprov-jdk18on-1.85.2.pom` (B7).

The 26 unlogged v1 POMs are listed in the comparison file. Their v2
counterparts each have a logged 200 GET.

## B4: signatures (retained, identity unknown)

There are 23 verifications: 21 JARs plus the junit-bom POM and module. A fresh
public-only gpg homedir was used
(`--no-options --no-autostart --no-auto-key-retrieve`) and removed afterwards.

- **21 GOODSIG**: 19 JARs plus the 2 junit-bom files.
  - Netty (11 JARs) and slf4j-api 1.7.36 signatures use a SHA-1 digest
    (`digest_algo 2`).
  - All others use SHA-512.
- **2 EXPKEYSIG with VALIDSIG.** In both cases the signature date is before
  the key expiry.

  | Artifact | Signed | Key expired |
  | --- | --- | --- |
  | jose4j 0.9.6 | 2024-03-06 | 2024-05-31 (`3051D450…F5316C5`) |
  | jdom2 2.0.6.1 | 2021-12-06 | 2023-12-06 (`78DF98EF…BD98C013`) |

- **Identity trust is UNKNOWN for all 23.** Keys came from public keyservers
  (keys.openpgp.org, with keyserver.ubuntu.com as fallback for UID-stripped
  keys), and no independently trusted publisher key list was used. A valid
  signature proves consistency with that key only.

## B5: Netty NOTICE

[evidence/r2-netty-notice-v2.json](evidence/r2-netty-notice-v2.json) covers
the upstream `NOTICE.txt` at tag `netty-4.1.139.Final` (SHA256 `64e368e7…`). None
of the 11 closure JARs carries its own NOTICE or LICENSE entry.

**Modified or contained third-party code.** Presence was checked by class
prefix in the closure JARs, using a zip listing only:

| NOTICE entry | Found in |
| --- | --- |
| Robert Harder Base64 | netty-codec |
| jbzip2 | netty-codec |
| libdivsufsort | netty-codec |
| jfastlz | netty-codec |
| Protocol Buffers portion | netty-codec |
| Webbit | netty-codec-http |
| HPACK (Twitter, Cory Benfield, Tatsuhiro Tsujikawa) | netty-codec-http2 |
| SLF4J portion | netty-common |
| JCTools | netty-common |
| JSR-166 extensions, Apache Harmony, Apache Commons Lang portion, Maven Wrapper, dnsinfo.h | **not mapped** (no unambiguous prefix) |

**Optional dependencies** (JZlib, Compress-LZF, lz4, lzma-java, zstd-jni, BC,
Snappy, JBoss Marshalling, Caliper, Commons Logging, Log4J, Aalto, Brotli4j)
are absent from `closure-v2.json`, with one exception. BC 1.85 is present only
because it is an explicit R2 root, not through a Netty edge.

**Claim limit.**
- The OSV querybatch ([osv/](osv/)) had 22 queries: the 21 closure coordinates
  plus `org.jctools:jctools-core:4.0.5`. It returned no vulnerabilities for
  any of them.
- That covers **coordinates only**. The upstream versions of the modified
  portions are not stated, and their applicability is **UNKNOWN**.
- jctools-core 4.0.5 is a proxy query, not a version bound to the shaded
  copy.
- This record replaces any reading of "JCTools only" or "0 OSV for all
  embedded code".

## B6: blast radius in repo-v2

Upgrade-only `require` constraints raise every co-resolved consumer to the R2
version. Data: [evidence/r2-blast-radius-v2.json](evidence/r2-blast-radius-v2.json).

| Module | Repo-v2 JAR versions | Repo-v2 POM-only versions | R2 selection |
| --- | --- | --- | --- |
| commons-io | 2.16.1 | 2.8.0 | 2.20.0 |
| commons-codec | 1.11 | 1.10 | 1.19.0 |
| slf4j-api | 1.7.30 | 1.7.32, 1.7.36 | 1.7.36 |
| commons-lang3 | — | — (absent) | 3.18.0 (new) |

POM-only versions were requested during conflict resolution but lost it.
Which consumer actually ends up on which version can only be proven by the
real graph, which is not run here.

## B7: exact allowlist (proposed, not approved, not activated)

[evidence/r2-allowlist-v2.json](evidence/r2-allowlist-v2.json) has 49 exact
`path` + `sha256` + `bytes` entries and **no globs**:

| Role | Count |
| --- | --- |
| binary (JARs, 19,808,511 bytes) | 21 |
| component POMs | 21 |
| parent POMs | 5 |
| junit-bom `.pom` and `.module` | 2 |

**Cross-checks.**
- All 49 entries match `artifacts-v2.json`, and the 23 signed files match the
  `jar_sha256` in `r2-signatures-v2.json`.
- No component publishes a `.module`: all 21 requests returned 404, so none is
  listed.
- Sidecars and `.asc` files are evidence, not repository inputs.

**Excluded as historical:** `quarantine/org.bouncycastle/bcprov-jdk18on/1.85.2/bcprov-jdk18on-1.85.2.pom`.
It was an invalid initial candidate; the aligned BC set is 1.85.

## Narrowed study claims

These replace the broader wording in the initial packet ("every Kotlin release
through master") and in follow-up 3 ("no Kotlin release fixes it", "no Kotlin
fix at any version"). Every claim below is limited to the refs actually
fetched (`target/phase-seven-security-remediation/fetch/fetch-log.txt`).

**protobuf-relocated**, from `gradle/versions.properties`:

| Version | Refs |
| --- | --- |
| 2.6.1-1 | v2.1.20, v2.1.21, v2.2.20, v2.2.21 |
| 2.6.1-2 | v2.3.0, v2.3.10, v2.3.20, v2.3.21, v2.4.21, master (snapshot at fetch time) |

- **CVE-2024-7254.** At the refs checked, the fork is patched (KT-80597,
  `8d859e1`) from v2.3.0. Compilers of 2.3.x and later reject `apiVersion 1.8`
  (the 2.3.21 bound was checked), so the unchanged RN 0.86.3 plugin cannot use
  them. Status: OPEN for this build.
- **CVE-2022-3171.**
  - The precondition (re-merge of repeated singular embedded messages) was
    counted at **v2.1.20 (27 sites) and v2.3.21 (30 sites) only**.
  - The fork-patch history queried at `sha=v2.3.21` (2 commits) touches only
    `CodedInputStream`.
  - Parser sources at v2.4.21, master and the other tags were **not
    scanned**.
  - The finding is "not fixed at the checked refs". It makes no claim about
    unchecked or future releases.
- **LZ4.** `versions.lz4-java=1.7.1` at v2.1.20, v2.1.21, v2.2.20, v2.2.21,
  v2.3.0, v2.3.10, v2.3.20, v2.3.21, v2.4.21 and the master snapshot. No other
  refs were checked.
- **commons-lang3 3.16.0 (GHSA-j288).** This is a statement about the approved
  KGP 2.1.20 JAR only.

## Not done / still open

- The allowlist is a proposal. Activating it needs root approval, then a
  Gradle-observed graph under the R1/SR-21 guards.
- Publisher identity for every key is UNKNOWN (B4).
- Netty's modified-code portions have unknown upstream versions and
  applicability (B5).
- Real-graph selection for the B6 consumers is unproven.
