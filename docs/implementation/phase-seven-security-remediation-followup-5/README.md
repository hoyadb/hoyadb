# Security remediation follow-up 5: independent Properties module wiring

Root decision A: the guard now uses `java_properties.py`, written by a
fresh-context agent from the public `Properties.load(Reader)` API only. That
module and its tests are unchanged from root's delivery:

| File | SHA256 |
| --- | --- |
| `java_properties.py` | `1520ebe4…` |
| `test_java_properties.py` | `5d52254e…` |

No legal clean-room certification is claimed. No Gradle, compiler or JAR was
executed, and nothing is staged.

## Regression before wiring

The new test `test_guard_uses_independent_properties_module` requires three
things:
- `kotlin_guard.MalformedProperties is java_properties.MalformedProperties`;
- the translated `_logical_lines`, `_convert` and `_properties` are absent;
- `check_launch` actually calls `java_properties.load_reader`, verified with a
  spy.

RED: [logs/red-test_kotlin_guard.txt](logs/red-test_kotlin_guard.txt) and the
`-O` log show 1 failure under both modes ("MalformedProperties is not
java_properties.MalformedProperties").

## Wiring

`kotlin_guard.py` changed from `78beb273` to `d88d53f5`:
- The translated parser was deleted.
- It now does `import java_properties`, aliases
  `MalformedProperties = java_properties.MalformedProperties` and calls
  `java_properties.load_reader(text)`.
- The guard's own byte-decoding policy, `_property_decodings` (ISO-8859-1, plus
  a UTF-8 cross-check for non-ASCII bytes), is unchanged.
- Root's escaped-key reproduction still rejects.

## GREEN, bound to the exact sources

[final-green/green-run-identity.json](final-green/green-run-identity.json):
five suites under normal and `-O`, 81 tests per mode, all OK.

| Suite | Tests |
| --- | --- |
| `test_kotlin_guard` | 27 |
| `test_symbol_guard` | 9 |
| `test_embedded_scan` | 11 |
| `test_init_script_guard` | 9 |
| `test_java_properties` | 25 |

That is root's 80 plus the new integration regression. Python 3.14.8 (the
interpreter SHA256 is recorded); sources were identical before and after the
run.

[final-manifest.json](final-manifest.json) lists all 14 final source SHA256s,
the R1 init-script pin `74aa545a…f9a0`, and the git-view indices.

## Frozen cuts and git-ready views

**Raw cuts.** Follow-ups 1–4 are preserved byte-exact, each verified against its
own `files-sha256.json`, only under the ignored
`target/phase-seven-security-remediation/frozen-cuts/`.

**Follow-ups 1 and 3** contain no GPL material and stay in `docs/` unchanged.

**Follow-ups 2 and 4** are replaced in `docs/` by DERIVED views. These are not
byte-identical to the cuts:
- `phase-seven-security-remediation-followup-2-git-view/` omits two verbatim
  JDK `Properties.java` excerpts and the translated-code `kotlin_guard.diff`.
- `phase-seven-security-remediation-followup-4-git-view/` omits the JDK source
  header excerpt and the copied JDK `ASSEMBLY_EXCEPTION`.

Each view carries `VIEW.md` and `OMISSIONS.json`. The omissions file records
each original path, SHA256 and size, the reason, the official OpenJDK jdk17u
`jdk-17.0.1+12` location, and the local raw path. Each view also has its own
`view-files-sha256.json`; the original indices are not rewritten. The raw
follow-up 2/4 directories were removed from `docs/` only after verification.
The local `target/` code snapshots that contain the superseded translation are
not for tracking.
