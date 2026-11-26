# Android plugin addendum: preliminary remote source research

This is neither an approved dependency bundle nor a completed resolved graph.
Only remote web reads and local documentation writes occurred during the native
benchmark quiet window. No dependency archive, plugin, Gradle build or local
browser was activated. Exact archive bytes, additional component counts,
selected variants, licenses and advisory impact remain unverified.

The [reviewed source-only comparison](../../../bindings/react-native/ANDROID_BUILD_OPTIONS.md)
selects an exact official-build addendum over editing upstream checks or
introducing an alternative compiler recipe. ADR-0035's original approval is
preserved; these missing components need separate review/approval before use.

## Confirmed published roots

The official [ktfmt0.22.0 plugin page](https://plugins.gradle.org/plugin/com.ncorti.ktfmt.gradle/0.22.0)
identifies marker coordinate
com.ncorti.ktfmt.gradle:com.ncorti.ktfmt.gradle.gradle.plugin:0.22.0.
Its [release commit](https://github.com/cortinico/ktfmt-gradle/commit/c318821f268aacf356a565c4e2b312d2f11ec432)
is c318821f268aacf356a565c4e2b312d2f11ec432. Tag/source inspection is not a
checksum verification of its published binary.

The [Foojay1.0.0 convention plugin page](https://plugins.gradle.org/plugin/org.gradle.toolchains.foojay-resolver-convention/1.0.0)
identifies marker coordinate
org.gradle.toolchains.foojay-resolver-convention:org.gradle.toolchains.foojay-resolver-convention.gradle.plugin:1.0.0.
Its1.0.0 release notes say implementation dependencies are shaded. A selected
POM dependency graph alone therefore cannot establish its embedded component
or notice closure. The precise release source ref is not verified yet.

## Additional inspection required

The pinned ktfmt [plugin build](https://raw.githubusercontent.com/cortinico/ktfmt-gradle/0.22.0/plugin-build/plugin/build.gradle.kts)
uses a separate classloader for the formatter dependency, rather than putting
its full transitives on the plugin's implementation classpath. The pinned
[version catalog](https://raw.githubusercontent.com/cortinico/ktfmt-gradle/0.22.0/gradle/libs.versions.toml)
selects java-diff-utils4.15 and ktfmt0.54. Capture and classify any lazily resolved
formatter configuration as well as marker/implementation variants. Do not infer
that a dependency was executed because it exists in this source catalog, or
that it is absent because it is missing from the plugin's implementation POM.

Inspect the exact Foojay JAR's embedded components and license metadata before
classification and scanning. Do not guess shaded versions from the current
main branch or an old issue. Keep native engine/runtime pins unchanged and
separate original approved build components from newly selected ones.

Gradle's [current toolchain documentation](https://docs.gradle.org/current/userguide/toolchains.html#sub:disable_auto_provision)
describes disabling JDK auto-provisioning and setting explicit installed paths.
That page currently reports9.8.2; the9.3.1 documentation URL was inaccessible
through the remote tool. Verify the relevant behavior against the installed
9.3.1 sources/configuration before applying it. The restriction is unchanged:
use the existing reviewed JDK, no toolchain download or source fallback.

## Next evidence procedure

After the host quiet window ends, use an owned metadata-only Gradle project,
following the already reviewed resolver pattern. Do not apply the new plugins
merely to resolve their metadata. Retain exact requested roots, repository
origins, selected attributes, component/edge lists and failures. Check the union
against the original approved graph, then capture exact URLs/checksums/bytes,
source provenance, notices and current advisory results for each addition.
Review the concrete graph and restricted commands before requesting operator
approval; unknown closure size is not reported as zero or as two artifacts.

Direct plugin POM URLs and the pinned Gradle documentation could not be read
through the remote web tool. Preserve that limitation; no complete graph or
hash claim follows from this preliminary research.
