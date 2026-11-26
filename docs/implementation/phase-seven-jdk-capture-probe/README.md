# TC-0038 SR-23 bounded pinned-JDK capture facility probe: manual results

This is diagnostic evidence only, run per SPEC-0034 and TC-0038 SR-23
(`1f55153`). It is **not** native-baseline enrollment and **not** a security
PASS.

It does not:
- approve the images it discovered or update an allowlist;
- accept any advisory, activate a dependency or release a compiler hold;
- count as SR-21/22 evidence.

What was not run: Gradle, a compiler, a helper class, an external JAR, an
agent, a new native payload, the network, collector code or wiring. No
security-source change was made, SR-32 was not started, and nothing was staged
by me.

## Inputs and pins

The pins were checked before any JVM ran and again afterwards, and all match
(`outcome.json` `pins_before` / `pins_after`):

| Input | SHA256 |
| --- | --- |
| realpath of `bin/java` | `2c663a2f…` |
| `lib/server/libjvm.dylib` | `15213df7…` |
| `bin/jfr` | `e1425ac9…` |
| `lib/modules` | `1902c850…8b91` |
| `release` | `c112e6b5…` |

| Artifact | SHA256 |
| --- | --- |
| Probe-only settings [`sr23-probe.jfc`](sr23-probe.jfc) (configuration 2.0; the six requested events; not an approved collector input) | `bb746681…c653` |
| Harness [`scripts/jdk_probe.py.txt`](scripts/jdk_probe.py.txt) | `a37528cc…eb4ca` |
| Byte walker [`scripts/jfr_bytewalk.py.txt`](scripts/jfr_bytewalk.py.txt) | `838541c0…1f4a` |
| Analyzer v3 [`scripts/jdk_probe_analyze.py.txt`](scripts/jdk_probe_analyze.py.txt) | `70547b58…72b6` |

**Analyzer versions.** v1 and v2 were analysis-only bugs; their outputs are
kept locally as `*-superseded.json`.
- v1: the decorator regex rejected space-padded thread IDs.
- v2: "unload" was counted by substring and matched the JNI method
  `getUnloadedEventClassCount`.

**Raw data.** Raw outputs stay local under
`target/phase-seven-security-remediation/jdk-capture-probe-1/`. That covers
environment and property values, decoder output and recordings.
[raw-hashes.json](raw-hashes.json) lists all 54 raw files by name, size and
SHA256. [outcome.json](outcome.json) is the versionable outcome: it holds
names, digests, counts, PIDs, reviewed paths, environment and property names,
allowlisted JDK facts and booleans. A scan found no environment or property
values in it.

## Procedure as executed

**Isolation, applied to every invocation (help, version runs and decoders):**
- each invocation gets its own fresh directory under the probe directory;
- the environment is exactly `PATH=<jdk>/bin:/usr/bin:/bin`, `JAVA_HOME`,
  `LANG=C`, and fresh `HOME` and `TMPDIR` beneath that directory. Nothing is
  inherited;
- the run is wrapped in `sandbox-exec` with
  `(version 1)(allow default)(deny network-outbound)(deny file-write* (require-all (require-not (subpath "<invocation realpath>")) (require-not (subpath "/dev"))))`;
- the timeout is 30 s, with kill/reap of the harness's own spawned PID only.
  No invocation timed out;
- stdout and stderr go to separate files.

**Runs:**

| Step | Spawned PID | rc | Elapsed | Outcome |
| --- | --- | --- | --- | --- |
| `java -XX:-UsePerfData -Xlog:help` | 56829 | 0 | 0.045 s | 171 tags |
| version-A (options in argv) | 56830 | 0 | 0.547 s | **failed**: 5 × `No tag set matches selection` |
| version-A-retry1 | 56845 | 0 | 0.592 s | OK |
| version-B (options only in `JAVA_TOOL_OPTIONS`) | 56846 | 0 | 0.537 s | **failed**: same 5 warnings |
| version-B-retry1 | 56868 | 0 | 0.531 s | OK |
| `jfr summary` / `jfr print --json --events <six>`, per final recording | 56885, 56907 (A); 56908, 56910 (B) | 0 | — | Decoders with their own directory and `JAVA_TOOL_OPTIONS=-XX:-UsePerfData -Djava.io.tmpdir=<decoder>/tmp` |

- **Candidate selections.** All seven candidates had every individual tag
  present in help, so all were requested.
- **Narrowed retries.** There was one narrowed retry per mode, as permitted.
  It omitted exactly the five rejected selections: `library+load`,
  `library+unload`, `os+library`, `os+library+load` and `os+library+unload`.
  `jni+resolve=debug` and `library=trace` were kept. No other selection was
  invented.
- **Failed attempts.** They are preserved. Their recordings also walk cleanly,
  but they are failed evidence.

## Observations

Results are for the final A and B runs; the two modes are identical unless
noted.

**Recording and PID identity**

- **JFR container (Python walk):**
  - one chunk, version 2.1, magic `FLR\0`;
  - the big-endian size covers the file exactly (A: 202414 bytes, B: 204837);
  - state byte 0, flag byte 3 (the final bit 0x2 is set);
  - durations 18.3 ms (A) and 17.6 ms (B).
- **PID agreement:** the spawned PID equals the `JVMInformation.pid`, the
  recording file name, the class-log file name and the class-log decorator
  (A 56845, B 56868).

**Events**

- **Requested events (print = summary):**

  | Event | Count |
  | --- | --- |
  | NativeLibrary | 1040 (520 names × 2) |
  | JVMInformation | 2 |
  | InitialSystemProperty | 30 (15 names × 2) |
  | InitialEnvironmentVariable | A 12 (6 × 2); B 14 (7 × 2) |
  | Shutdown | 1 |
  | ProcessStart | 0 |

  With everyChunk on a single chunk, each event appeared at both chunk begin
  and chunk end. The begin and end values are equal for every key (booleans
  only in `outcome.json`).
- **Non-zero extras in the full summary table:** `jdk.CheckPoint` 7 and
  `jdk.Metadata` 1, both recording-format records. Every other listed type was
  0.
- **NativeLibrary.**
  - 520 unique names in both modes, the same set in A and B.
  - `topAddress` is 0 for every entry. `baseAddress` is non-zero and stable
    between begin and end.
  - The `java` launcher is absent, consistent with the index-0 skip.
  - `libSystem.B.dylib` and `libjvm.dylib` are both present.

  | Group | Count | Notes |
  | --- | --- | --- |
  | JDK-home files | 7 | `libjava`, `libjimage`, `libjli`, `libnet`, `libnio`, `libzip`, `server/libjvm` |
  | Absent as files | 510 | dyld cache only |
  | Outside the JDK **and present as regular files** | 3 | `/usr/lib/system/libsystem_{kernel,platform,pthread}.dylib`. Universal (`cafebabe`) binaries that are **also** in the studied cache image list; their SHA256s are in the outcome |

  The image count is far larger than the prep plan's "minimal set" wording
  suggested.
- **Environment:** the recorded names equal the constructed names, plus
  exactly one launcher-added name, `__CF_USER_TEXT_ENCODING`. B additionally
  shows `JAVA_TOOL_OPTIONS`. No constructed name is missing, and every
  constructed value was recorded unchanged (boolean).
- **System properties:**
  - 15 JVM-level initial names, such as `java.home`, `java.io.tmpdir`,
    `java.class.path`, `sun.java.launcher` and `java.vm.*`;
  - the effective `java.io.tmpdir` equals the owned run's `tmp`, and
    `java.home` equals the pin;
  - this is not the full Java property set.
- **JVM arguments:**
  - in **both** A and B, `jvmArguments` contains every capture option as an
    exact space-separated token, so `JAVA_TOOL_OPTIONS` options are visible
    there;
  - A and B are equal after normalizing the run path;
  - `javaArguments` is empty for `-version`, and `jvmFlags` is empty;
  - `jvmVersion` contains 17.0.1+12.
- **Shutdown:** one event, reason "No remaining non-daemon Java threads".

**Logs**

- **Class-load log** (`uptime,pid,tid`; decorators are **space-padded**):
  - 1270 lines, 0 malformed after padding-tolerant parsing, newline-terminated;
  - the positive control `java.lang.Object` loads first;
  - `java.lang.Shutdown` is present.
- **Candidate logs:**
  - `library=trace`: 107 lines, with "Loaded library" for **only 4** JDK
    libraries (`libjimage`, `libnet`, `libnio`, `libzip`). There were no
    lines for `libjava`, `libjvm`, `libjli` or any dyld dependency, and no
    "Unloaded library" lines.
  - `jni+resolve=debug`: 305 lines of JNI registration and linking, which is
    not library lifetime.

**Inventory and sandbox**

- **Owned directories:** `tmp`, `repo`, `work` and `home` are empty after
  exit; JFR removed its repository. There are no crash logs, extra recordings
  or rotated logs.
- **Sandbox:** no denial symptom appeared in the child output. No
  system-log-level detection of attempted writes was performed or is claimed.

## Expectations: observed versus unknown

**Observed:**
- a final chunk with a structurally complete container;
- the required JVMInformation, NativeLibrary, InitialEnvironmentVariable,
  InitialSystemProperty and Shutdown events;
- the class positive control;
- PID agreement;
- owned tmpdir and repository, with no leftovers;
- the capture options visible in `jvmArguments` in both modes.

The facility probe therefore did not fail.

**Unknown or not established:**
- **Native completeness.** A snapshot alone cannot rule out a library that
  loaded and unloaded between chunks. `library=trace` covers only JVM-initiated
  loads after logging starts (4 of 7 JDK libraries, no dyld dependencies), and
  the load/unload tag sets are unsupported in 17.0.1+12. Native-load
  completeness stays **unresolved**.
- **Process linkage.** Zero ProcessStart events in a version-only run proves
  nothing about child-process linkage.
- **Runtime properties.** Startup properties are only JVM-level initial ones.
  Runtime Gradle properties, compiler strategy and later mutations are not
  established.
- **`-D` visibility.** Whether arbitrary `-D` properties appear in
  `InitialSystemProperty` is not established by this run; only
  `java.io.tmpdir` was supplied.

**Disclosed correction:** the preparation `facts.json` (`11685899…`) described
InitialEnvironmentVariable and InitialSystemProperty as `endChunk`. That was
incorrect; the installed `default.jfc` uses `beginChunk`, and this probe
requested `everyChunk`. The historical file is not rewritten.

## Smallest evidence-based corrections to the collector plan

The plan `e17871cd` is not approved and its six design gaps remain; these
corrections come from this probe only.

1. **Decorator parsing.** Parse `-Xlog` decorators tolerating space padding,
   and match the PID by decorator, not by a fixed-width pattern.
2. **Unsupported tags.** Drop the five unsupported tag sets. Keep
   `library=trace` only as *partial* load-time evidence, never as completeness.
3. **Dual-resident images need an explicit decision before any reviewed record
   lists them.** Some dyld-reported images exist both in the cache and as
   regular files. Under the current guard, a real file can never be classified
   as a cache image; hashing the on-disk file binds that file, not the mapped
   cache copy.
4. **Launcher-added environment names.** Exact environment equality must allow
   an explicit reviewed list of launcher-added names (observed:
   `__CF_USER_TEXT_ENCODING`), with values kept local only.
5. **Capture-option verification.** Verify capture options as exact
   space-split tokens of `jvmArguments`; this works for argv and for
   `JAVA_TOOL_OPTIONS`, given whitespace-free paths. A `-version` run has no
   `javaArguments`, so role assignment needs real main-class JVMs.
6. **System properties.** `InitialSystemProperty` is not the effective Java
   property set. The guard's `system_properties` input needs a separately
   reviewed source; that remains a known gap.
7. **Discovery size.** Enrollment discovery from `-version` covers 520 images,
   not a "minimal" set, so the review burden is real. The plan's wording
   should be corrected.
8. **Post-run checks.** After exit, require an empty owned JFR repository and
   tmp, a single final chunk and the class positive control. All of these were
   observed here, so they are usable checks.
