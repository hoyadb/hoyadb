# Restricted-build guard integration checkpoint

This checkpoint supports SPEC-0034 / TC-0038. It records verification of the
follow-up-5 guard cut; it is not permission to activate dependencies, a build
result, or a final Phase 7 review.

`root-verification.json` binds all 14 source files before and after the root's
81-test runs in normal and optimized Python. Both runs passed. It also records
the literal escaped-key rejection and the static production-module audit.

`parser-origin.json` records the independent parser agent's source and test
hashes and the exact derivation of its notes and RED/GREEN logs in
`independent-parser/`. Only the absolute repository prefix in those copied
files is replaced with `<repo>`; originals remain under `target/`. The account
describes implementation from the public API contract, not legal certification
or universal Java conformance.

Root verified all indexed entries in follow-ups 1, 3 and 5 and the derived
follow-up-2/4 views: 132 files, no hash mismatches or unlisted files. The two
views were also compared against the locally frozen original cuts, including
every omitted file's original size and hash. The omitted translated parser and
JDK source bodies remain outside this versioned evidence.

The focused Claude Opus 5.5 review confirmed that all five earlier findings
were resolved. It raised a further native-baseline integration gap: macOS
system images without individual files cannot meet the current SHA256-only
baseline contract. Root confirmed that the four named system paths are absent
as files on this host (macOS 26.7.1, build 25G241). Actual JFR event contents
have not been captured. A separately reviewed identity contract and regression
are needed before runner integration; no missing digest is silently accepted.

The follow-up-5 README correction and shared-cache implementation are recorded
in the separately preserved follow-up-7 successor. Its root verification is
`sr22-successor-verification.json` (102 tests per mode). Follow-up 8 then closes
the empty-cache-image vacuity gap, with a non-empty canonical mandatory subset
checked for every JVM, including matches through reviewed aliases.

`mandatory-image-verification.json` binds the final 16-source cut, the root's
109 passing tests per mode and the focused Claude Opus 5.5 PASS. No existing
test was removed or weakened. The TDD chronology explicitly distinguishes the
old setup/API errors from behavioral RED, and labels later stub experiments as
retrospective. The API is ready for controller wiring, with the mandatory macOS
cache-evidence and trusted caller snapshot contracts recorded there.

The real dependency scan, compiler observation, RN Android execution and the
two final clean reviews remain outstanding. Coverage remains unmeasured.
