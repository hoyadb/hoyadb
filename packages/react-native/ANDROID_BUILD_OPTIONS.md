# Official Android plugin activation gap: source-only comparison

Read during the quiet window; no Gradle/Kotlin/plugin execution or metadata fetch
was performed for this comparison. ADR-0035's exact145-build-component approval
does not cover unknown formatter/toolchain-resolver dependencies.

## Confirmed source facts

Verified installed @react-native/gradle-plugin0.86.3 is an approved npm archive.
Its unchanged settings.gradle.kts applies
org.gradle.toolchains.foojay-resolver-convention1.0.0. The root and shared,
settings-plugin and react-native-gradle-plugin build.gradle.kts apply
com.ncorti.ktfmt.gradle0.22.0. These names are absent from the approved native
artifact-index and selected-inventory. Their transitive graphs/checksums/licenses
are unknown at this checkpoint. The package's official root already disables
its KtfmtCheck tasks; this project must not edit those settings or disable checks
to bypass approval.

The actual plugin implementation source under the three src/main/kotlin roots
uses Gradle APIs plus Gson, Guava, JavaPoet and, for the app plugin, AGP/KGP APIs.
The already approved selected classpath contains Gson2.11.0,
Guava33.3.1-jre, JavaPoet1.13.0, AGP8.12.0 and KGP2.1.20. Source build descriptors
request Gson2.8.9/Guava31.0.1-jre/JavaPoet1.13.0 plus those AGP/KGP versions;
matching the approved selected versions requires exact classpath/conflict
verification, not an untested binary-compatibility assertion. The official task
config targets Java11/JVM11, API1.8 and JDK17. Installed Gradle9.3.1 carries
Kotlin2.2.21 libraries; the separately inventoried installed compiler is2.2.20.
Neither fact proves compatibility with the official2.1.20 compiler build.

## Option A: exact official build-tool addendum (recommended for repeatability)

After the quiet window, resolve only metadata for the two plugin markers and
implementation closures, selecting the installed Gradle/JDK attributes. Keep
all existing runtime pins, approved build145 graph and original provenance.
Enumerate every additional coordinate/version/artifact URL/SHA256 (additional
strong hashes when declared), license/notice, script and advisory impact. Review
toolchain auto-download behavior and require the existing JDK17 with downloads
disabled. Present the exact additional build-only graph for explicit CHG4
approval before executing either plugin. Unknown graph size is not reported as
zero or as two artifacts merely because two plugin IDs were found.

Then execute the unchanged official includeBuild workflow with that exact
approved graph, no remote/shared cache, trusted owned projects and complete
actual lock/verification metadata. Check actual app/build resolution against the
whole approved manifest before compilation. Preserve all SDK/consumer checks.
This follows the official build inputs and avoids a new compiler recipe, at the
cost of another concrete approval gate. No new engine runtime is proposed.

## Option B: separate reviewed compile of unchanged implementation sources

A possible no-new-artifact route is a project-owned recipe that never evaluates
upstream settings/build scripts. Compile each unchanged official production
source root separately with the already installed compiler, explicit language/
API/JVM settings and exact approved145 plus installed Gradle API classpath;
package official plugin descriptors from their checked declarations and include
MIT/source provenance. Shared, settings-plugin and app-plugin remain distinct
artifacts/module names. Do not substitute dummy plugins or modify upstream files.

This is not yet a verified recipe. First inspect complete imports and required
resources/generated classes, prove the declared API/compiler settings and
classpath, then compile and exercise official autolink/Codegen/entry-point
outputs plus the full actual SDK app. Compare source hashes against the original
SRI-verified npm archive before/after. Record the installed compiler2.2.20 versus
upstream2.1.20 distinction honestly. An Android Java consumer can avoid applying
an app Kotlin compiler plugin, but AGP/tool native executables still require
actual graph checks; an unexpected aapt2/compiler download must stop. No upstream
check is reported as passed unless actually executed.

This route changes build reproducibility/maintenance obligations and may expose
further compatibility gaps. It is larger than Option A and should only proceed
with independent recipe review and the same end-to-end gates. The comparison
itself authorizes no activation and makes no compatibility or support claim.
