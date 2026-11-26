# Independent public-specification parser task

Origin: delegated implementation of the approved SPEC-0034 / TC-0038 security
remediation slice. New parser API: MalformedProperties(Exception) and
load_reader(text: str) -> dict[str, str]. Parent retains integration and governance
artifact ownership. No staging, commits, builds or existing parser changes.

## Acceptance before implementation

- Given natural lines with CR, LF or CRLF, when loaded, then blank/comment lines
  are ignored and only space, tab and form feed receive whitespace treatment.
- Given odd/even trailing backslashes, when joining lines, then only odd parity
  continues and initial whitespace of continuation lines is discarded.
- Given escaped/unescaped separators, when split, then the first unescaped
  separator terminates the key and the optional value separator is consumed once.
- Given escapes in keys/values, when decoded, then controls, four-hex Unicode,
  escaped punctuation and ordinary escaped characters have documented results.
- Given malformed Unicode, when loaded, then MalformedProperties is raised.
- Given duplicate decoded keys, when loaded, then the final value wins.
- Given synthetic original security-fixture patterns, when parsed, then escaped
  dangerous setting names and values are exposed to the future integration owner.
- Given the compiling empty-result stub, when tests run, then behavioral failures
  demonstrate RED; the original implementation must pass normal and -O runs.

Rollback: remove only the newly introduced module, tests and owned notes after
review; no previous file is replaced by this task.

## Sources actually consulted

- AGENTS.md, /Users/boristacyniak/.codex/RTK.md, ai-governance/RULES.md,
  ai-governance/FRAMEWORK.md and rules 00-09 (excluding 10), plus 11.
- docs/design/spec/SPEC-0034-restricted-build-security-remediation.md.
- docs/design/tests/cases/TC-0038-restricted-build-security.md.
- bindings/react-native/tools/security/test_kotlin_guard.py (fixtures only;
  not imported or executed).
- Official Java SE 21 public API documentation, Properties.load(Reader), fetched
  with web.open on 2026-10-25:
  https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/util/Properties.html#load(java.io.Reader)
  Relevant API section: tool-rendered lines 257-307. No source links followed.

No existing parser, guard implementation, JDK/OpenJDK/Oracle source, archives,
source diffs, security evidence folders or implementation history was read.
Independent implementation is a process description, not legal certification.
