# Official Android plugin addendum — awaiting approval

The unchanged RN0.86.3 Gradle build applies two plugins missing from ADR-0035's
exact packet. This review proposes their **six additional Maven coordinates**:
two marker POMs and four JARs. Foojay's JAR also embeds Gson2.13.1. Nothing here
changes app runtime, Kotlin SDK runtime, Rust core or cipher dependencies.
Neither plugin has been applied or executed during this review.

| Additional build component | Exact version | Actual reviewed JAR SHA256 |
| --- | --- | --- |
| com.ncorti.ktfmt.gradle:plugin | 0.22.0 | `547cbe949294cd15da68f860a13999abb78a9e9f15a3a3fca0a98b6a06c88279` |
| io.github.java-diff-utils:java-diff-utils | 4.15 | `964c69e3a23a892db2778ae6806aa1d42f81230032bd8e4982dc8620582ee6b7` |
| org.gradle.toolchains:foojay-resolver | 1.0.0 | `78b86a47dfdf7697c9bd15da78983fd80da7247d6e02fc106bdf07e0388b60a8` |
| org.jetbrains.kotlin:kotlin-stdlib | 2.1.10 | `5f2ac1ca8dc8b37a3f4314e716d36969ebf0227a75181d32699d0a8f645b1c21` |

The marker coordinates are
com.ncorti.ktfmt.gradle:com.ncorti.ktfmt.gradle.gradle.plugin:0.22.0 and
org.gradle.toolchains.foojay-resolver-convention:org.gradle.toolchains.foojay-resolver-convention.gradle.plugin:1.0.0.
Their exact POM bytes/hashes are preserved in repository-responses.json.
Annotations13.0 is already approved and is the only other selected component in
the plugin closure. Kotlin2.1.10 above is a build-plugin dependency; the approved
application/SDK Kotlin2.1.20 pin is unchanged. Actual shared classpath selection
must remain inside the complete approved union before compilation.

## Observed review

The task-owned metadata consumer applied only Gradle's builtin Java plugin.
Online resolution passed, followed by an offline repeat with byte-identical
resolution and lock. The original debug/release runtime87, SDK runtime2 and
build145 component sets are unchanged. Exact attributes, variants, edges,
commands, isolated Gradle home, JDK17.0.1 and observed exit codes are in metadata/.
This is metadata resolution, not a successful official Android build.

All four JARs and their published source archives were fetched as inert data
over official repository HTTPS. Three JAR hashes match publisher module SHA256
declarations. Java-diff-utils had no publisher SHA256 declaration available;
its pin was computed from the actual reviewed archive bytes. These pins are not
independently authenticated publisher signatures. Archive inventories and hashes
are preserved; executable archives remain under ignored target/.

The Foojay1.0.0 source tag resolves to
2a6cc60ed8d35e025e44193c895502996c8edf4e. Its build declares Gson2.13.1 and
shading/minimization; the actual shaded Gson/ReflectionHelper class bytes contain
that version. This is component evidence, not a reproduced binary build.
The ktfmt0.22.0 source commit is
c318821f268aacf356a565c4e2b312d2f11ec432. Full source-license text is retained:
MIT for ktfmt plugin; Apache2.0 for Foojay, Gson, Java-diff-utils and Kotlin,
with Kotlin's upstream NOTICE. Published POMs lacking licenses are supplemented
from these exact upstream sources, not treated as permission-free.

Seven exact OSV queries (six coordinates plus embedded Gson) returned no known
matches at retrieval. This is a registry version check, not proof of safety or
unreachability. ADR-0035's original47 build-only advisories, including3 critical,
remain unchanged and explicitly disclosed; this addendum does not rescan or
silently supersede their accepted risk.

## Restricted activation proposal

Preserve the unchanged official includeBuild and its checks. Use only trusted
owned inputs, the original approved artifact set plus this exact addendum,
isolated GRADLE_USER_HOME, no shared/remote build cache and no HOME override.
Verify every archive before activation; prepare a verified local dependency
repository/cache, then run reviewed commands offline with no build or
configuration cache. Compare actual selected components and hashes before
compilation; stop on an extra component, artifact, download or source fallback.
No publication or customer-store operation is included.

Require the explicitly selected existing JDK17 with auto-detection and
auto-download disabled. The actual Gradle9.3.1 builtin javaToolchains command
reports both Disabled and only the specified JDK. Inert disassembly also shows
the download flag is checked before invoking a toolchain resolver. The final
consumer must capture and verify the same effective controls; no JDK fetch is
authorized if the existing toolchain is absent or incompatible.

The plugin creates a separate lazy ktfmt0.54 formatter classpath. Its metadata
graph (18 components) is disclosed in resolution.json but **is outside this
proposed activation**. No formatter archives were fetched for this packet.
Do not run ktfmtFormat or activate that classloader. The unchanged upstream RN
build already disables its KtfmtCheck tasks; this project did not disable them
and does not claim those upstream tasks passed. Project formatting and tests
remain required. An unexpected formatter resolution/activation stops the build
for separate review rather than expanding this approval silently.

The original exact approval explicitly requires separate review/approval for
newly selected components. This packet is the concrete addendum for that gate;
it is not approval received and proves no mobile compatibility or runtime gate.
Rollback removes unpublished build inputs/artifacts only; no store is changed.

## Evidence files

bundle.json defines the proposed scope and restrictions. repository-responses.json
preserves exact POM/module text and byte hashes; archive-review.json records
actual URLs, fetch outcomes and archive hashes. artifact-inventories.json and
embedded-gson-evidence.json cover actual JAR contents. reviewed-source-and-licenses.json
preserves relevant published source files and full upstream license text.
Metadata-only formatter results stay distinguished from proposed plugin inputs.
The preliminary RESEARCH.md remains historical; its previously unknown source
ref and graph are resolved by this later evidence.

Original Gradle JSON whitespace and trailing log blank lines are preserved
losslessly in resolution-original.json and *.txt.json wrappers. The readable
resolution.json has identical parsed content; presentation.json records both
byte hashes. Original online/offline equality refers to the original bytes.

## Claude review and publisher-attestation follow-up

[Claude's focused review](claude-review.md) found this packet sufficient for an
operator decision, with a nonblocking recommendation to attempt publisher
signatures and the legacy checksum. Its source/data consistency scope and lack
of independent upstream byte inspection are preserved in the report.

The follow-up fetched all four detached JAR signatures and the legacy
Java-diff-utils SHA1; that checksum matches. All four signatures verify
cryptographically using issuer-matched public-keyserver keys in task-owned GPG
homes. This does **not** establish independent trust in publisher identities;
the unknown-trust qualification remains explicit. Exact signatures, public keys,
commands, initial failures and successful verification statuses are retained in
publisher-attestation*.json. Initial keys.openpgp import had an unavailable UID
for the legacy key; a bounded second keyserver retrieval resolved the public
key. The initial long GPG home produced an agent socket-path error; public-only
verification completed in a fresh short owned home with autostart disabled, and
that home was removed. No user keyring or plugin execution was involved.

This closes the recommended attempt/provenance gap without claiming a signed,
independently authenticated publisher identity or changing the exact JAR pins.
