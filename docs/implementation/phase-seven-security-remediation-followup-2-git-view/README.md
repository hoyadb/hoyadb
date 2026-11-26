# Security remediation follow-up 2: SR-20 Java Properties correction

Correction inside approved SR-20 (root finding). Frozen predecessors are
unchanged: the initial cut (`ffab198d`) and follow-up 1 (`1dcf7f71`).
No Gradle, compiler or JAR execution.

**Finding (root).** Initial handback `kotlin_guard.py` `5ea74e2d` returned
`ok=True` for
`check_launch(secure_launch_args(), {}, {'consumer/gradle.properties': 'systemProp.idea.use.native.compression=true\n'})`.
- The pinned JDK 17.0.1's `java.util.Properties.load0`/`loadConvert` decode
  `\uXXXX` in both keys and values, so the real key enables native compression.
- Valid whitespace-delimited keys were also unsupported.
- `logs/root-repro-after.txt` shows the same call now rejects.

**Preserved.** The exact initial 12-file source hashes are in
[initial-handback-source-sha256.json](initial-handback-source-sha256.json).

**RED.** [logs/red-test_kotlin_guard.txt](logs/red-test_kotlin_guard.txt) and
`red-O-…`: 24 tests ran, 6 genuinely failed, under both normal and `-O`.

| Test | Failure |
| --- | --- |
| unicode-escaped key/value | assertion |
| whitespace/colon separators | assertion |
| even-backslash literal | assertion |
| malformed `\u` fail-closed | assertion |
| bytes ISO-8859-1/UTF-8 input | error |
| explicit native-baseline contract | error |

**Fix.** [kotlin_guard.diff](kotlin_guard.diff) replaces the parser with a port
of JDK 17 `Properties.LineReader.readLine`, `load0` and `loadConvert`.
- Source: the pinned JDK's own `lib/src.zip` (SHA256 in
  [jdk17-src.zip.sha256](jdk17-src.zip.sha256); excerpts included).
- Bytes decode as ISO-8859-1, as `Properties.load(InputStream)` does. A
  non-ASCII file is also checked as UTF-8 and rejects if either decoding enables
  a setting.
- A malformed escape fails closed.
- The last value wins, matching `put`.
- `check_observed_jvms` gains an explicit `native_baseline` (trusted
  JDK/platform libraries). Every observed native library must match the
  reviewed payload inventory (`native_loads`) or the baseline
  (`baseline_loads`) by name and SHA256; anything else keeps the gate open.

**GREEN.** `logs/green*`: all 46 tests pass under both normal and `-O` (24 + 9
+ 8 + 5; the other three suites are unchanged).

**API.** [README-api.diff](README-api.diff). The caller must pass every Gradle
properties source: consumer root, included-build root, the isolated
`GRADLE_USER_HOME` and the Gradle installation. The pre-launch check is
required in addition to the authoritative observed-JVM check.
