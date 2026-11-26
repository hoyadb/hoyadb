# Review packet: Kotlin JVM plugin marker for the official RN includeBuild

Status: PREPARATION ONLY. The bytes are quarantined in `bytes/`. They are not in
the active repository (`target/phase-seven-rn/android/repo`) or any Gradle
cache, and no Gradle run has used them. This packet does not approve anything.
ADR-0035's supplement (8a87389) lists six coordinates and two markers, and this
marker is not among them.

## Why it is requested

The unchanged official `@react-native/gradle-plugin` 0.86.3 build applies
exactly two versioned plugin IDs (`gradle/libs.versions.toml` `[plugins]`):

- `org.jetbrains.kotlin.jvm` 2.1.20: its marker is missing;
- `com.ncorti.ktfmt.gradle` 0.22.0: its marker is already approved in 8a87389.

The consumer applies AGP, KGP and RN through the buildscript classpath, so it
needs no markers. The offline configuration step stopped with
`could not resolve plugin artifact 'org.jetbrains.kotlin.jvm:org.jetbrains.kotlin.jvm.gradle.plugin:2.1.20'`
(`../android/build-b/build-environment.txt`). Root's earlier metadata consumer
resolved classpath coordinates without applying plugins, so it never captured
plugin markers.

## Exact quarantined bytes ([fetch.json](fetch.json), [parse.json](parse.json))

| File | HTTP | Bytes | SHA256 |
| ---- | ---- | ----- | ------ |
| `org.jetbrains.kotlin.jvm.gradle.plugin-2.1.20.pom` | 200 | 1397 | `5ccb905a796717218c1bb4eed8bb31fc674c5803cdd54c23753ead380f7a11b5` |
| `.pom.sha1` | 200 | 40 | matches the POM's SHA-1 |
| `.pom.md5` | 200 | 32 | matches the POM's MD5 |
| `.pom.sha256` / `.pom.sha512` | 404 | — | not published |

Every file came from `https://repo.maven.apache.org/maven2/org/jetbrains/kotlin/jvm/org.jetbrains.kotlin.jvm.gradle.plugin/2.1.20/`
over HTTPS with no redirect. An earlier attempt failed before contacting the
network, because zsh expanded the `=https` argument; its output is kept in
`shell-quoting-failure-no-fetch.txt`.

Strict parse (stdlib ElementTree; no DOCTYPE or entity declarations present):

- coordinate `org.jetbrains.kotlin.jvm:org.jetbrains.kotlin.jvm.gradle.plugin:2.1.20`;
- packaging `pom`;
- license Apache-2.0, developer JetBrains, scm github.com/JetBrains/kotlin;
- exactly one dependency, `org.jetbrains.kotlin:kotlin-gradle-plugin:2.1.20`,
  with no scope, type, classifier, optional flag or exclusions. That
  dependency is already in the approved `nativeBuildTools` graph;
- no parent, repositories, pluginRepositories, build, profiles, properties,
  dependencyManagement, distributionManagement or modules, and no Gradle
  module-metadata marker.

Limitations:

- Integrity rests on the publisher's SHA-1/MD5 sidecars, which are weak, and
  on HTTPS to Maven Central.
- The PGP `.asc` signature was not fetched, because it was out of this
  packet's scope. Nothing was compared with the Gradle Plugin Portal copy.

## Proposed activation change (only after explicit operator approval)

1. Add one allowlist entry to `target/phase-seven-rn/android/prepare-repo.py`:
   the exact URL and SHA256 `5ccb905a…`, copied from this quarantine to
   `org/jetbrains/kotlin/jvm/org.jetbrains.kotlin.jvm.gradle.plugin/2.1.20/`.
   Rebuild the active repository fresh.
2. Rerun only the same offline `graph` step (`build.py <label> graph`). Change
   nothing in the network profile, JDK, Gradle home policy or official files.
3. Stop again on any further unresolved or unapproved coordinate, and report
   it.

Restrictions: everything already binding (ADR-0035, 8a87389 bundle
restrictions, offline, fresh GRADLE_USER_HOME, no JDK or formatter-0.54
download, no source fallback). This marker activates no new code; it only
redirects to the approved KGP.

Rollback: delete `android/repo`, remove the allowlist entry and rerun
`prepare-repo.py`. The quarantine and all build logs stay.

## Forecast: the marker alone will not make the build compile

This is not yet observed in a Gradle run; it is inferred from inert reads.

- Applying KGP to compile Kotlin needs its compiler configurations. The
  approved KGP jar (`kotlin-gradle-plugin-2.1.20-gradle85.jar`) contains the
  strings `kotlin-compiler-embeddable` (5 hits), `kotlin-build-tools-impl` (4),
  `kotlin-scripting-compiler-embeddable` (1), `kotlinCompilerClasspath` (3)
  and `kotlinBuildToolsApiClasspath` (5).
- The approved artifact index contains none of
  `kotlin-compiler-embeddable`, `kotlin-build-tools-impl`,
  `kotlin-daemon-embeddable` or `kotlin-scripting-compiler-embeddable` at
  2.1.20. It does contain `kotlin-build-tools-api`, `kotlin-compiler-runner`,
  `kotlin-daemon-client` and `kotlin-reflect`.
- Both the official plugin build (Kotlin sources) and the consumer app (its
  `.kt` sources under `org.jetbrains.kotlin.android`) would need these.

Recommendation for root and operator: decide this marker together with a
quarantined, metadata-only resolution of the KGP 2.1.20 compiler
configurations (`kotlinCompilerClasspath`, `kotlinBuildToolsApiClasspath`,
plugin classpaths) for the official plugin build and the consumer app, in the
same addendum format as 8a87389. Approving piecemeal would stop at the next
missing coordinate. I can prepare that resolution only if root authorizes it
separately.
