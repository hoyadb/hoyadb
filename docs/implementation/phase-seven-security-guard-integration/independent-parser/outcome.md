# Verification and limitations

Only java_properties.py, test_java_properties.py and this owned target notes
folder were intentionally written. Existing files and unrelated user changes
were preserved. Git status was inspected before editing; no history, staging,
commit, build, Gradle, JVM, compiler, JAR or existing guard execution occurred.

## TDD command record

1. Tests and compiling empty-result stub were written before implementation.
   An initial test scaffolding run (`python3 -I -B
   bindings/react-native/tools/security/test_java_properties.py`) exited 1 due
   to an invalid raw-string literal in the new test; that syntax was fixed.
   This syntax failure is not counted as RED evidence.
2. `python3 -I bindings/react-native/tools/security/test_java_properties.py`
   against the stub exited 1: 25 tests, 50 assertion failures across subcases.
   Full captured stdout/stderr: red.log. The stub is preserved as red-stub.py.
   Failures reflect empty/missing parsed output and absent malformed-Unicode
   exceptions, not imports, syntax, missing dependencies or JVM availability.
3. Small original implementation written, using no third-party dependencies.
4. `python3 -I bindings/react-native/tools/security/test_java_properties.py`
   exited 0: 25 tests, OK. Full captured stdout/stderr: green.log.
5. `python3 -I -O bindings/react-native/tools/security/test_java_properties.py`
   exited 0: 25 tests, OK. Full captured stdout/stderr: green-optimized.log.
6. Read all new production/test code for a focused final review. Python AST
   audit exited 0: no production imports, assert statements or top-level
   statements other than docstring/definitions. static-audit.log records it.

## Scope and conformance limits

- Tests use synthetic strings; the security-fixture input patterns were read
  from test_kotlin_guard.py but that file and its imports were never executed.
- No differential JVM oracle was used. These results establish tested behavior
  from the public Reader-format contract, not universal Java conformance.
- Reader input is already decoded str. Byte decoding, file reading, defaults,
  store/XML formats, Gradle and guard integration are outside this module.
- An unmatched terminal backslash is removed by continuation assembly. The
  public API prose does not separately specify that EOF corner case; no JVM
  result is claimed for it. Escaped Unicode units are represented with chr(),
  including surrogate code units rather than combining pairs. No surrogate
  interoperability claim is made.
- No project Python formatter/linter was invoked; no tooling dependency added.
- Parent owns integration, artifact changes, broader verification and release.
- This independent implementation record is not a legal clean-room certification.

Final file and log SHA-256 values are in hashes.sha256. Hashes were calculated
from bytes after the successful runs and final review.
