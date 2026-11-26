# Phase 7 approved bundled dependencies and restricted builds

**Explicitly approved on 2026-10-24 under ADR-0035's restricted build policy.**
[Approval and reviewed-byte hashes](approval.json) record the exact authorization.
Approval is not install, build, runtime or compatibility evidence. The review
snapshot below was prepared before activation. Read-only archive vetting and trusted Gradle
metadata resolution ran under `target/`; no dependency hook, CLI, build plugin,
compiler from an npm package or dependency JAR was loaded/executed.

## Recommended frozen inputs

| Surface | Exact proposal | Observed review |
| --- | --- | --- |
| RN JavaScript/test consumer | React19.2.3 / RN0.86.3; RN Babel+Metro0.86.3; CLI+Android+iOS20.2.0; Babel core7.29.7/runtime7.29.10 | npm477 package locations /451 identities;0 known registry advisories. Exact graph+declared licenses/scripts/SRI in `npm/` |
| RN Android app runtime | React Android0.86.3, Hermes V1 `com.facebook.hermes:hermes-android:250829098.0.17`, Kotlin stdlib2.1.20, explicit Okio3.4.0 constraint | **87 selected runtime components** per debug/release; no known runtime OSV match after the Okio change |
| Standalone Kotlin SDK runtime | Kotlin stdlib2.1.20 + org.jetbrains annotations13.0 | **2 selected components**; no mandatory SDK coroutine/serialization library |
| Native build tooling | Official RN Gradle-plugin0.86.3 sources, AGP8.12.0, Kotlin plugin2.1.20, Gson2.8.9, Guava31.0.1-jre, JavaPoet1.13.0 | **145 selected build components**, unchanged official graph; known build-only advisory matches explicitly require risk acceptance below |
| iOS runtime | SRI-verified RN/codegen source plus exactly the six debug/release prebuilt containers listed below | Exact official SHA256/SHA512 pins captured; **no source fallback** and no fictitious Podfile.lock |

The selected native Maven union is **229 coordinates /222 concrete variant
artifacts**. All222 artifact entries have SHA256 pins. Publisher module/sidecar
SHA256 is retained where supplied; for88 legacy artifacts only weak or missing
publisher SHA256 was available, so exact archive bytes were fetched over official
repository HTTPS and a local SHA256 computed. These archives were not executed.
The artifact index distinguishes those evidence types; it does not call a
registry checksum an independently authenticated signature.

## Exact locks and artifact index

- Recommended npm manifest: `npm/package.json`.
- Recommended npm lock: `npm/package-lock.json`, SHA256
  `c05cb6a927e86bd25a1a076df5e7d911745bad9773f21dca5e9149b153df8b2b`.
- Recommended Maven lock: `native/gradle.lockfile`, SHA256
  `20f0ff0bd6d259e9faa28fc5606ca028411ba6e0490fdcbe79d8e84531f865b1`.
- **Full native artifact URL/hash index:** `native/artifact-index.json`.
- Every selected Maven component, declared license and configuration membership:
  `native/selected-inventory.json`.
- Actual selected dependency edges, variants, constraints, versions and selection
  reasons: `native/resolution.json`.
- Compact machine-readable bundle: `bundle.json`.

Gradle9.3.1 (the pinned template wrapper version) and installed9.4.1 selected the
same original components. The final proposed graph uses cached9.3.1/JDK17.0.1
with a task-authored build and builtin Java plugin only, isolated user-home,
metadata-only configurations and the source-checked Kotlin androidJvm→jvm
compatibility rule. AGP/Kotlin plugins were not applied. Metadata resolution and
an offline repeat pass; this is an equivalent metadata consumer, **not an actual
AGP consumer lock or successful mobile build**. Parent selection of a supported
build JDK is separate from this observed resolution input; no global toolchain
was changed. RN itself brings coroutine1.8.1 transitively; the standalone Kotlin
SDK need not expose or depend on it.

## Why the two narrow upstream deviations

The official template is frozen at
`dc3587fd74ab8cd31be7f817e644d874a884c623`. Its CLI20.1.0 graph is preserved as
`baselines/npm-package-lock.json`:473/446, seven moderate affected entries from one XML-parser
advisory. The separately labelled CLI20.2.0 graph has zero known npm advisories.
It is the same CLI major, but the versioned compatibility table stops at RN0.85;
actual0.86 build/runtime compatibility remains a validation gate.

The unchanged native runtime graph is also preserved:86 components, including
Okio2.9.0 with GHSA-w33c-445m-f8w7. The proposed graph replaces only that coordinate
with `okio:3.4.0` and `okio-jvm:3.4.0` through an explicit Gradle constraint; all
other selected native components are unchanged. Both new coordinates returned
no OSV matches. Official Okio3.0 release notes describe strong2.x compatibility
but flag `Timeout.intersectWith`'s return ABI change. RN's source has no call.
OkHttp4.9.2's source uses the Kotlin inline method; static JDK `javap` of its real
MultipartReader PartSource shows the body inlined, with no call to the changed
method. This addresses that particular ABI concern; it does not replace actual
RN/HTTP/mobile execution after approval. Source archives/changelog/disassembly
and graph delta are retained under `reviews/okio/`.

## Build-only advisories: explicit scope and risk acceptance

The original native scan matched13 coordinates and48 unique nonwithdrawn
advisories. After the runtime Okio patch, **12 build-only coordinates /47 unique
advisories remain:3 critical,18 high,24 moderate,2 low**. They are Netty4.1.110,
Commons Compress1.21, jose4j0.9.5, BouncyCastle1.79, JDOM2.0.6 and Kotlin Gradle
plugin2.1.20. This is a registry version-match result, not demonstrated
exploitability in the proposed commands. It is also **not a blanket claim of
unreachability**. They are selected only by nativeBuildTools, not packaged app
runtime configurations. Exact per-coordinate affected ranges, primary advisory
references and actual dependency paths are in the bundled JSON and
`native/advisory-impact.json` / `native/advisory-details.json`.

Keep the official tool graph unchanged rather than blindly upgrading plugin
majors or adopting a beta solely to clear scanners. Approval must explicitly
accept its bounded build-only risk: use only owned/trusted projects and reviewed
inputs, fresh isolated Gradle homes, no remote/shared/untrusted build cache,
prefer `--no-build-cache`, and run offline after all exact inputs are verified.
Kotlin's build-cache deserialization advisory lists a2.4.20-Beta1 fix; the
candidate does not claim2.1.20 is patched. Archive/XML/network/crypto advisory
matches similarly remain recorded; trusted inputs and omitted unneeded tooling
commands reduce exposure but do not prove all affected paths unreachable. No
managed-device server, unreviewed plugin or arbitrary project is authorized by
this packet. Actual approved command paths must be reviewed before execution.

## Exact iOS prebuilt allowlist

The official RN0.86 source defaults to prebuilt React-Core/Dependencies and
Hermes V1. Only the following containers are proposed. Repository base is
`https://repo.maven.apache.org/maven2/`; full URLs/SHA512/SHA1 are in
`ios/prebuilt-metadata.json`.

| Container | SHA256 |
| --- | --- |
| react-native-artifacts-0.86.3-reactnative-core-debug.tar.gz | `95283572f0d46919c8e208811d7470eca174d47b6152505f205d805867f99e1f` |
| react-native-artifacts-0.86.3-reactnative-dependencies-debug.tar.gz | `fa019419384f6f859655fec80b2d20736bb0441bb911cb1944b91719322ae512` |
| hermes-ios-250829098.0.17-hermes-ios-debug.tar.gz | `01ff9328903b2d8e4184b6efd9003eb33959ba1c1913c13258fefc42e16c20c2` |
| react-native-artifacts-0.86.3-reactnative-core-release.tar.gz | `05359752100c0db4be9229423fc673eb3b981ba97aa6210707b878a708320a98` |
| react-native-artifacts-0.86.3-reactnative-dependencies-release.tar.gz | `5e06070fdad3315f9f3020283d16875395f93e8a59e175be36d8971e64095892` |
| hermes-ios-250829098.0.17-hermes-ios-release.tar.gz | `2967ee51b41bd433b3027dda875e96194354e2b4d715f9f186fd0c04ccef309d` |

`ReactNativeDependencies` release archive was fetched and its SHA256 verified;
its1827-file inventory is retained. The other five hashes are publisher
metadata declarations; archive-byte verification remains a required activation
step. HEAD requests returned403 while ordinary checksum/archive GETs succeeded;
those distinct observations are recorded. Full prebuilt internal source SBOM is
not supplied/proved: do not substitute a guessed source-Pod graph. The81 local
Podspec source files are frozen by the RN npm archive and separately inventoried,
including all declared dependencies, version/license expressions and hooks.
Generated ReactCodegen/ReactAppDependencyProvider specs depend on the final
consumer and require later reviewed codegen. **No evaluated Podfile.lock exists.**

Source/component attribution is retained separately, with immutable upstream
commits and full license text for boost1.84.0, DoubleConversion1.1.6, glog0.3.5,
fast_float8.0.0, fmt12.1.0, Folly2024.11.18.00 and SocketRocket0.7.1. This metadata
is not proof these exact tags produced every prebuilt byte. In particular,
DoubleConversion's actual upstream BSD-style license is retained even though
RN's Podspec declares MIT. Root RN/Gradle/Codegen/Hermes MIT sources and the six
missing-POM Maven license supplements were checked; npm declarations remain
separate from actual archive notice review. Build-only JNA dual licensing,
MPL/CDDL/GPL-classpath/JDOM notices are disclosed by the inventory rather than
being mislabeled as app-runtime dependencies or given a legal certification.

Upstream can silently choose source dependencies when prebuilt availability
checks fail. **That fallback is not in this approval.** Missing/unknown/hash-
mismatched containers must stop. A later actual Podfile.lock must contain only
the approved RN/codegen/host sources and frozen prebuilt containers; new external
sources require review, not an implicit network fetch. Existing CocoaPods1.16.2
and Xcode are pre-existing tool inputs; no new Ruby Gemfile graph is proposed.

## Proposed single operator approval scope

Approve the exact npm lock, patched Maven runtime/tool locks and six frozen iOS
containers above, with the disclosed unchanged build-only advisory risk, to fetch
and verify immutable archives/notices, install npm with scripts disabled, then
run only source-reviewed official codegen/build commands in owned synthetic
consumers. Disable remote/untrusted build caches and source fallbacks; verify
actual AGP/CocoaPods results stay within the allowlist. This is question text,
**not received approval**. Actual compatibility, encrypted mobile execution,
packaging/restart/cancellation/HTTP tests and current tool/JDK manifests remain
postapproval gates. No package publication or user-store modification is implied.
Any new Android cipher provider dependency must be concretely included by the
parent in that same final approval; this packet preserves the approved Phase6
graph and does not silently choose a new provider.


## Durable packet and retained research

This directory is the reviewable approval packet. It contains no node_modules,
JAR/AAR/native archive, npm source package, executable, or active project manifest.
The npm package.json and local Gradle resolver are **proposed consumer inputs**,
not additions to hoyadb's runtime/build. Do not execute them before the operator
approves activation. `bundle.json` names the canonical inputs; `files-sha256.json`
provides a complete exact file map for this directory. The native artifact index
has been copied with explicit ignored-target archive-retention labels; its local
file hash therefore differs from the research-index hash while artifact pins do
not. Native/npm lock hashes above remain byte-identical to the tested metadata
resolution.

Large archives, raw POM/module/version responses, unmodified source trees and
transient caches remain under ignored `target/phase-seven-dependencies/`.
`provenance/source-archives.json` records actual SRI/byte hashes and hashes of
unpacked file inventories; `provenance/maven-metadata.json` and npm per-version
metadata retain primary URLs/hashes without pretending those raw caches are
included here. The81 Podspecs themselves remain inside the pinned RN archive;
the self-contained static inventory preserves their source hashes and inspected
contract expressions. Full native advisory records and small actual license
texts are included. The original template CLI/native graph is retained under
`baselines/` as historical metadata, not a second activation proposal.

All88 distinct proposed Android/Kotlin runtime coordinates were queried together
against OSV after the patch and returned no version matches:
`reviews/okio/osv-all-patched-runtimes.json`. This supersedes an inference solely
from the two changed coordinates; it remains a point-in-time registry result.
The approved Android provider sources are reused; security review of the native
provider/build integration must precede build, without introducing an unknown
additional runtime package. Root's TC16 ID-prefix decision is presented separately
alongside this single dependency approval.


## Presentation whitespace and source hashes

Copied source/license/output text is presentation-normalized for repository
whitespace checks: CRLF becomes LF, trailing spaces/tabs and excess blank lines
at EOF are removed. License grant words and all other non-whitespace bytes stay
unchanged. `presentation-normalization.json` records each raw/presentation hash
and the exact original snapshot retained under ignored
`target/phase-seven-dependencies/presentation-raw/`. Upstream/source/archive
hash declarations continue to identify the **raw fetched bytes**, not these
presentation copies. `files-sha256.json` hashes the final durable presentation.
Exact npm/native lockfiles remain byte-identical and were excluded from
normalization; artifact SRI/SHA256 pins are unchanged.
