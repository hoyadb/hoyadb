# DEF-0030 AN01-AN04 static review (read-only; Claude Opus 5.5, claude-opus-5-5)
No edits, staging, network, build, JVM, Gradle or repo-code execution. Own static scripts: macho_syms.py
(pure-Python Mach-O/ar LC_SYMTAB parser, no tool execution), closure.py (independent closure recompute).

Hashes: apple_notices.py 29826438, build-artifacts.py ce1b89fa, check_artifacts.py 581c1fde, corpus index
65a591b5 (28 files), NOTICE 5178e861, Cargo.lock a96a5481, parent native-build.json c19233a6 (anchored in
committed docs/implementation/phase-seven-native-rn-integration/root-verification.json): STABLE throughout.
Drift DURING review (expected AN05 lane): test_apple_notices.py 6ed11237 -> cf10162b (AN05 class + shared
pack_layout Git fixture only), pack.py 48588bae -> 614de1ba. AN01-04 judged on cut-44 copies.

Verified: corpus partition 35 lock pkgs = 13 attributed (source+checksum match) + 22 excluded (wasm-only chain);
26 texts + NOTICE + index, none unindexed; framework 40 files = 10 native (== pre-notice copy == parent) +
parent copy + 28 notice members + packaging.json; packaging.json exactly regenerated; nothing under Headers;
Swift source copy, RN package-source copy and invalid attempt-2 extracted copy all identical closure
(packaging 47e27eb8). Static symbol parse: every slice/arch imports exactly 11 OS symbols (9 _CC*, 2 Security);
10 from sqlcipher.o, _CCRandomGenerateBytes from Rust std (NOTICE wording matches); 0 LibTomCrypt symbols incl.
an extended LTC name list; member counts 363/458 match make-corpus. TDD: red-2 named not-implemented stubs;
green-3 44/44 normal and -O on cut-44 hashes; real artifact RED missing-packaging, GREEN finalize/validate,
check_artifacts 5/5 incl. two-copy typecheck on Swift source copy.

No blockers for AN01-AN04 within these limits. Residuals:
R1 Rebuild path depends on untracked controller: library-hash-bound provider evidence makes a real rebuild fail
   (stale-provider-evidence, fail-closed) until target/phase-seven-rn/def-0030/make-corpus.py (untracked,
   ~/.cargo, ~/.rustup absolute inputs) regenerates the corpus. Track/document it before the next native rebuild.
R2 Validator trusts index graph/provider evidence/toolchain link inputs (no subprocess by design); derivation is
   make-corpus.py. LibTomCrypt wrong-platform check is name-substring ('tomcrypt') plus evidence count==0;
   evidence 'architectures' field is not cross-checked.
R3 finalize writes members before its final validate; post-write failure would leave a partly finalized tree
   (pre-write checks make this unlikely).
R4 Malformed-index inputs can raise non-NoticeError exceptions (traceback, exit 1); still fail-closed.
R5 check_artifacts.py accepts unittest name filters; RN pack calls validate_framework directly (same function).
R6 AN04 extracted-RN-tarball check exists only for INVALID attempt-2; fresh tarball after AN05 still required.
   The "old 10 imports" record was not located; the new 11 count is independently confirmed.
R7 bindings/native/third-party is untracked; staged-blob byte preservation is root's check.
