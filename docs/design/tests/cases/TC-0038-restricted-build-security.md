---
id: TC-0038
title: Restricted mobile builds remediate applicable findings without runtime drift
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-25
updated: 2026-10-26
parents: [TP-0001]
verifies: [REQ-0046, SPEC-0034]
tags: [phase-seven, security, build-tooling]
---

# TC-0038: Restricted build security

Acceptance before implementation. The triage's SR labels are retained. No
listed result is a passing security or compatibility claim. Use synthetic data,
isolated output and explicit normal/optimized-Python checks where applicable.

```gherkin
Feature: Approved build remediation preserves exact runtime packages
  Scenario: SR-01 compile alignment uses already reviewed fixed components
    Given the official unchanged RN plugin and a frozen approved graph
    When the pinned init script resolves its named compile classpath
    Then Guava33.3.1-jre and Gson2.11.0 replace the old compile-only versions
    And the entire named closure including all new support transitives matches approved coordinates and bytes
    And any unapproved transitive stops before compilation rather than passing on the two direct pins
    And unrelated configurations and app runtime coordinates and bytes do not change
    And the prior graph first fails the fixed-selection assertion

  Scenario: SR-02 unchanged official plugin remains compatible
    Given the exact approved final graph and scoped constraint script
    When the real official plugin compiles from unchanged reviewed sources
    Then compilation succeeds and before-after source hashes remain identical
    And the script cannot silently modify an unnamed configuration

  Scenario: SR-10 new AGP dependency fixes have a reviewed exact closure
    Given fixed-version candidate metadata and its transitive graph
    When the final approved build graph is resolved offline
    Then all selected versions and bytes equal its reviewed allowlist
    And missing unknown unresolved or unapproved components stop before compilation
    And an older affected version first fails the fixed-selection assertion

  Scenario: SR-11 advisory reporting cannot hide remaining findings
    Given exact resolved and embedded component identities
    When primary-source advisory checks and applicability classification run
    Then applicable findings are fixed or remain explicitly open
    And failed unavailable or unknown-version scans never report a clean result
    And known advisory fixtures and unsupported lookup controls are reported accurately

  Scenario: SR-12 actual Android build preserves the approved runtime
    Given the final verified graph and current certified Android AAR
    When the real RN Android consumer assembles including debug keystore generation
    And its owned emulator executes parity and independent restart cases
    Then the app succeeds using the exact certified artifact
    And approved debug and release runtime graph coordinates and bytes remain unchanged
    And build-only fixes do not enter the app or cause source fallback

Feature: Vulnerable embedded paths and inventory gaps are verified
  Scenario: SR-20 unsafe Kotlin compression settings reject before launch
    Given consumer and included build configuration and relevant JVM options
    When incremental compilation is enabled or native compression is enabled
    Then the secure runner rejects before Gradle or compiler launch
    And the default secure configuration applies the reviewed settings to both builds
    And only relevant flag names are reported without unrelated environment data
    And each of org.gradle.jvmargs kotlin.daemon.jvmargs GRADLE_OPTS JAVA_OPTS JAVA_TOOL_OPTIONS _JAVA_OPTIONS and JDK_JAVA_OPTIONS has its own enabling-property rejection control
    And explicit launch arguments and project properties cannot bypass that check
    And prefixed Gradle environment properties system-property forwarding and nested daemon options have rejection controls
    And every observed compiling JVM's effective arguments and system properties satisfy the same restriction

  Scenario: SR-21 the selected workflow cannot reach vulnerable LZ4 storage
    Given fresh owned build outputs and the reviewed nonincremental configuration
    When bounded actual compilation and relevant snapshot transforms execute
    Then evidence shows vulnerable LZ4 entry points are not invoked
    And the Kotlin execution strategy observed JVM identities and JDK observation mechanism are recorded
    And an otherwise valid path-enabling control demonstrates the detection works
    And any surviving reachable path leaves the security gate open

  Scenario: SR-22 embedded Jansi native-load behavior is dispositioned
    Given the exact embedded native inventory and bounded observed compiler JVM
    When the selected actual compilation executes with JDK observation enabled
    Then Jansi native loading or its absence is recorded for that process
    And any observed payload is matched to the reviewed native inventory
    And an unobserved process or incomplete capture keeps the load residual explicit
    And class-load logging without native-library observation is an incomplete capture
    And native loading alone is not relabelled as a demonstrated vulnerability

  Scenario: SR-22a system-cache identity is distinct from file-backed payloads
    Given separate reviewed file inventories and an exact host-specific shared-cache record
    And independently collected current host and cache evidence
    When a complete native-library capture includes an absent-as-file system image
    Then its exact reviewed path image identity and common bounded aligned slide must match
    And it is reported separately as a system-cache load
    And every observed entry is classified exactly once
    And pre-capture host capture and post-capture host share one boot-session UUID
    And slide alignment is the reviewed cache architecture's mapping alignment
    And every observed JVM contains the reviewed mandatory cache-image subset including libSystem.B.dylib
    And a reviewed alias satisfies its canonical mandatory image while mandatory entries cannot themselves be aliases
    And file-backed payload and JDK baseline SHA256 checks remain mandatory
    And treating an unregistered real file or an unknown missing-file path as a cache image rejects
    And a Version1 record rejects every real-file cache path without exception

  Scenario: SR-22b stale incomplete or self-enrolled cache identities reject
    Given a reviewed shared-cache baseline and synthetic complete native capture
    When the OS build cache file hash main or subcache UUID architecture or image identity changes
    Or a required identity address or host observation is missing
    Or the pre-capture capture and post-capture boot sessions differ
    Or the mandatory image subset is empty unregistered or absent from any captured JVM
    Or images have different negative unaligned or out-of-bounds slides
    Or a path prefix wildcard unreviewed alias or unreviewed image is supplied
    Then the gate rejects rather than accepting a missing digest
    And discovery of a new image cannot update its own trusted allowlist
    And UUID-only identity cannot replace complete cache-file hashes

  Scenario: SR-22c loader environment cannot redirect the reviewed cache
    Given the otherwise valid secure launch configuration
    When any DYLD_ environment variable is supplied
    Then the runner rejects before launch and reports its name without its value
    And the effective captured environment is checked against the same restriction

  Scenario: SR-22d an explicit successor binds dual-resident disk copies separately
    Given a Version1 record and a cache image path that is a regular file
    When host validation runs as a preserved characterization control
    Then it rejects and cannot classify the path as a cache image
    Given a separately reviewed Version2 record with an explicit canonical dual_resident_images subset
    And independent pre and post dual_resident_files sizes and hashes exactly match that subset
    And each subset path is regular while every other reviewed path and alias is absent
    When a digest-less observation matches its cache identity and common bounded aligned slide
    Then it is classified once as a system-cache image
    And its on-disk hash is host evidence only and never a claim about mapped cache bytes
    And a reviewed alias still resolves to its canonical image
    And an empty dual subset grants no real-file exception
    And Version1 retains all prior behavior and rejects Version2-only fields
    And a Version1 host carrying dual_resident_files rejects
    And each Version2 host missing dual_resident_files rejects
    And pre and post must each match the reviewed record even when their wrong values agree
    And missing malformed non-canonical aliased unregistered or wildcard subset entries reject
    And Boolean zero negative or non-integer sizes and malformed digests reject
    And a symlink changed missing extra or incomplete pre or post file observation rejects
    And a digest-bearing cache observation rejects even when it matches the disk hash
    And a different mixed unaligned negative or out-of-bounds slide still rejects
    And omitted mandatory images still reject for each observed JVM
    And unknown or candidate record schemas and automatic enrollment still reject
    And cache UUIDs cannot replace full cache-file hashes
    And partial native lifetime capture and missing effective properties remain incomplete

  Scenario: SR-30 absence and fork claims have concrete evidence
    Given exact approved JAR entries and advisory-affected symbol fixtures
    When the KAPT log4j protobuf and commons-lang3 applicability checks run
    Then absence claims hold only when the affected symbols are actually absent
    And synthetic presence controls reject the claimed absence
    And present forks require patch or call-path and input-boundary evidence
    And unsupported residual acceptance is never inferred

  Scenario: SR-31 embedded components are part of the permanent dependency gate
    Given an approved JAR fixture with the known shaded commons-lang3 metadata
    When embedded and relocated inventory checks run
    Then that component and its advisory are detected separately from external coordinates
    And source-declared versions are distinguished from byte-bound versions
    And an added untriaged component missing identity or failed scan rejects
    And the present missing embedded gate first fails the detection requirement
```

### SR-22d RED order and diagnostic controls

The Version1 rejection is an existing characterization check, not a claimed
RED. First add the Version2 positive tests through the existing guard API and
observe that a valid dual-resident evidence set is rejected because Version2
support is absent. That result assertion is the new capability's RED; no
import, keyword, fixture or uncaught schema exception counts as one.

Every new Version2 negative test also asserts its field-specific violation
identity. Rejection solely because Version2 is unknown cannot satisfy it.
Use the `system cache record: dual_resident_images` identity for malformed or
unregistered subset/file identities, and the side-specific
`pre-capture host: dual_resident_files` or
`post-capture host: dual_resident_files` identity for missing, extra or changed
host file evidence. Wrong path kinds identify the side and
`dual-resident image_path_status`. Tests pin the expected reason for each
case rather than accepting a generic schema/record-fields rejection. Keep the
Version1, mandatory-image and capture rejection controls unchanged and run all
cases in normal and optimized Python.

## SR-23: bounded JDK capture facility probe (manual)

This FLOW-8 procedure precedes collector implementation. Synthetic parser
tests cannot establish which facilities this installed JVM actually emits;
record actual results, including failures, before designing those parsers.
This is diagnostic evidence, not native-baseline enrollment or a security PASS.

Given the already approved Temurin17.0.1+12 x86_64 installation at
`/Users/boristacyniak/.sdkman/candidates/java/17.0.1-tem`, verify these SHA256
pins before any JVM runs:

| Input | SHA256 |
| --- | --- |
| realpath of bin/java | 2c663a2f4677833e1305ea553aa188af1e22aca92a85f39e6189ae0708e5f8b4 |
| lib/server/libjvm.dylib | 15213df7162b170ef0f658004f534d52e92d8f24f67e2a7ce327ea0dc78014fb |
| bin/jfr | e1425ac98334fff59d6b2b8cffb3cf720e90a88016db4f1c67a9ea5225bf881b |
| lib/modules | 1902c850eecc989ab9d3eb28bacc4cfcc15a058e62710ab87c3b9f4314fd8b91 |
| release | c112e6b5ddf9739d7b125ddb75f00ddee4bfa889a1246bdee8f1f009a139ee1d |

1. Preserve a fresh owned output directory under
   `target/phase-seven-security-remediation/`. All invocations, including help
   and decoding, use an explicit environment with only
   `PATH=<jdk>/bin:/usr/bin:/bin`, `JAVA_HOME=<jdk>`, `LANG=C`, and fresh
   `HOME` and `TMPDIR` beneath that output directory. Inherit no shell values.
   Use sandbox-exec with `(version 1)(allow default)(deny network-outbound)`
   plus a file-write denial outside that invocation's owned run directory and
   `/dev`. Construct both sandbox subpath filters from resolved realpaths.
   Use a `require-all` conjunction of the two negated `subpath`
   filters for that denial; do not relax it on failure. The Python harness
   enforces a 30-second timeout on each spawned invocation and kills/reaps
   only its own spawned PID if necessary. A killed attempt is failed evidence
   and may lack a final chunk. No network, helper class, compiler, Gradle, external
   JAR, agent or native payload is executed. Do not change unrelated processes
   or existing files. Preserve partial outputs when any step fails.
2. Give the help invocation its own fresh `<out>/help` directory and sandbox
   boundary. Run only the pinned java's `-XX:-UsePerfData -Xlog:help` to record its
   supported tags. Inert reads of the already pinned JDK sources may clarify
   semantics; no additional fetch is needed for this probe. Never infer that
   a load-time facility is absent solely because its tag is absent from a man
   page. An unsupported candidate is a recorded limitation, not a reason to
   change JDK or disable a required check.
3. Create and hash a minimal local `.jfc` with configuration version 2.0.
   Explicitly request `jdk.NativeLibrary`, `jdk.JVMInformation`,
   `jdk.InitialSystemProperty`, `jdk.InitialEnvironmentVariable`,
   `jdk.Shutdown` and `jdk.ProcessStart`. For the first four use period
   `everyChunk`; for Shutdown and ProcessStart disable stackTrace. This
   configuration is probe-only and is not an approved collector input.
   Omitted events can retain built-in defaults; the six requests are not a
   claim that only six event types will be emitted.
4. Run pinned `java -version` twice, in separate fresh working directories:
   A with capture options in argv, B with the same options only in the
   explicitly constructed `JAVA_TOOL_OPTIONS`. The options are
   `-XX:-UsePerfData`, `-Djava.io.tmpdir=<run>/tmp`,
   `-XX:FlightRecorderOptions=repository=<run>/repo`,
   `-XX:StartFlightRecording=filename=<run>/jfr/,dumponexit=true,settings=<jfc>`,
   and
   `-Xlog:class+load=info:file=<run>/classload-%p.log:uptime,pid,tid:filecount=0`.
   Create the destination directories first. Before either version run,
   record the candidate selections and those whose individual tags occur in
   help. The fixed candidates are `jni+resolve=debug`, `library=trace`,
   `library+load=trace`, `library+unload=trace`, `os+library=trace`,
   `os+library+load=trace` and `os+library+unload=trace`. Request candidates
   with available tags in separate non-rotating logs in both runs; a tag's
   presence alone does not establish that a combination exists. Do not treat
   JNI resolution alone as lifetime coverage. Each candidate log uses its own
   filename, `uptime,pid,tid` decorators and `filecount=0`.
   Record the exact options, settings hash, spawned PID, start/end times,
   return code, stdout/stderr hashes and output inventory. No retries overwrite
   a previous run. Either an unknown-tag error or a `No tag set matches`
   warning is unsupported logging and fails that attempt even if exit is
   zero. At most one narrowed retry per mode may omit exactly those rejected
   selections; it has a new directory and names the limitation. No additional
   logging selection is invented after seeing the capture output.
5. Decode each resulting recording with the pinned `jfr summary` and
   `jfr print --json --events` selecting exactly the six enabled events.
   Decoders use the same minimal environment with only `-XX:-UsePerfData`
   and their own `-Djava.io.tmpdir=<decoder>/tmp` supplied through a
   decoder-only JAVA_TOOL_OPTIONS;
   they are separate diagnostic JVMs, not observed children. Preserve their
   arguments, environment names, exit codes and raw output locally. Preserve
   the full summary event-count table, and report every non-zero extra event
   type; a filtered print cannot establish the complete emitted event set.
6. Independently walk the recording bytes with Python: each 68-byte-or-larger
   chunk starts with `FLR` plus NUL, its big-endian size at offset 8 fits the
   file, sizes cover the file exactly, every state byte at offset 64 equals
   zero, and the final chunk's byte 67 has bit 0x2 set. Record major/minor
   versions, actual chunk start times and durations as well. Preserve the
   byte-walk script and its hash. Check PID agreement across the spawned
   sandbox-exec PID, JVMInformation, recording filename and class-log
   decorator, and record native-library names, address
   semantics, Object class positive control, Shutdown presence and any
   launcher-added environment names. Record the effective java.io.tmpdir and
   require it to equal the owned run's tmp directory. Inventory that location,
   the explicit repository and the working directory. Report sandbox denial
   symptoms visible in the child's outputs or failure; no system-log-level
   detection of attempted writes is performed or claimed.
   Compare actual JVM arguments in A/B;
   do not assume tokenization or JAVA_TOOL_OPTIONS visibility. Inventory the
   working and temporary directories for crash logs, extra recordings,
   rotation and leftover recording repositories. Compare each captured initial
   environment/property key's begin/end values locally and record equality
   booleans only. The installed default.jfc uses beginChunk for these events;
   this probe requests everyChunk. The preparation facts.json's earlier
   endChunk description was incorrect and must remain a disclosed correction,
   not a rewritten historical fact. Equal values in this version-only run do
   not establish visibility of later mutations in a compiling JVM.
7. Report which expectations were observed and which remain unknown. A
   missing final chunk, required JVM/native/environment/property/Shutdown
   event or class positive control is a failed facility probe.
   Zero ProcessStart events in a version-only run does not prove child-process
   linkage. Startup properties do not prove runtime Gradle properties or
   compiler strategy. Snapshot-only native evidence remains partial; do not
   set a native-load-complete attestation from it. A supported tag or one
   observed load does not prove complete native lifetime coverage. No run
   approves its discovered images or updates a reviewed allowlist.

Raw environment/property values, ProcessStart commands and decoder output
stay local. A versioned outcome contains only input/output hashes, command
templates, counts, PIDs, reviewed paths, environment names and allowlisted JDK
version facts; it must not contain arbitrary environment/property values.
Collector design and its behavioral cases require a separate review after
these observations. Compiler/dependency holds are unchanged.

DEF-0028 R01–R04 separately cover exact repository assembly and prelaunch
verification. All security diagnostics are build-only; no product command,
database content or runtime key is used in the fixtures.
