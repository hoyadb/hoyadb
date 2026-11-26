# DEF-0030: Apple native package notices — implementation evidence (AN01–AN05)

This packet records the implementation evidence for DEF-0030
(`docs/design/defects/DEF-0030-apple-native-package-notices.md`). The reviewed
sources and corpus were committed by root at `ab68ce6`. This is focused
packaging evidence. It is not the final Phase 7 review round, not a package
release and not a legal compliance certification or SBOM.

No native Rust recompilation, app rebuild, simulator/runtime execution or
Gradle run was part of this work.

## Identities

| Item | Identity |
| --- | --- |
| Reviewed source commit (34 files, including the corpus and `android/.npmignore`) | `ab68ce6e5394745abba7540cac8b8ab6b181343b` |
| Compilation parent `bindings/swift/Artifacts/native-build.json` (byte-unchanged; trusted external input) | `c19233a6d34e76628ae42c4870be5c6add4fa39c7f4fb9ed46678da95a532496` |
| `bindings/native/Cargo.lock` | `a96a5481be73dc5ab125b4cec4b441bedd256f7a1d834694073cba5f342cb951` |
| Notice index `bindings/native/third-party/index.json` | `65a591b52a3a632526c1b1f930980cc8a7f2bd561af020b7e6e8fb19eb72bde6` |
| `NOTICE.md` | `5178e861fbf3c226c42222c4bce70a440fa309fd266105b3eb9c97fffcbf23ba` |
| XCFramework `packaging.json` | `47e27eb8d68e55022e34ea227a9c5882b4a84fc56bede14c21e5ae3fb92947de` |
| Final RN archive `hoyadb-react-native-0.1.0.tgz` SHA256 | `83ba210612181fbd3b5ca3d746cde4577f5ca521df4256c4f0b6e5c54a5cbc75` |
| Final RN archive SHA512 | `fa24d745ce119f9620f8701e1b83492859a404376a71c0b23b3f38f7951f5848d7bcbe30e295aa950653cb1f1e40be9ee0a19641797ddd0b2b4880e4506476d3` |
| Archive `head_commit` (pinned by the pack tool) | `ab68ce6e5394745abba7540cac8b8ab6b181343b` |
| Full archive manifest (path, size, mode, SHA256 per entry) | [rn-pack/final-1/archive-manifest.json](rn-pack/final-1/archive-manifest.json) |

The archive itself stays at
`target/phase-seven-rn/def-0030/rn-pack/final-1/packed/` (untracked). Older
archives are unchanged:

- run-c `1ac8d113…`;
- the previous archive `6a2ca07b…`;
- the INVALID attempt-2 archive `68608f9e…`.

## Permanent sources (committed at ab68ce6)

- `bindings/native/third-party/`: `NOTICE.md`, `index.json` and 26 upstream
  texts. They are stored with the existing `-text` attribute, and root checked
  that every staged blob equals the reviewed bytes.
- `bindings/native/tools/apple_notices.py`: the shared corpus validator,
  XCFramework finalizer and framework validator. It uses only explicit raises
  and runs no network or subprocess operations.
- `bindings/native/tools/test_apple_notices.py`: 53 stdlib unittest cases.
  Every tool runs as a subprocess under `python -I` and `python -I -O`.
- `bindings/swift/build-artifacts.py`: after xcodebuild and the
  `native-build.json` write, it always calls `finalize_framework`. A
  rejection fails the build.
- `bindings/swift/check_artifacts.py`: the old packed-copy skip is gone.
  `test_notice_packaging_closure` requires `--expected-parent-sha256`.
- `bindings/react-native/tools/pack.py` and
  `bindings/react-native/android/.npmignore` (`.gradle/` and `build/`). The
  `package.json` `"files"` list is unchanged.

## AN01: source notice corpus

The corpus was derived by the trusted controller
[corpus/make-corpus.py.txt](corpus/make-corpus.py.txt) (exact bytes, SHA256
`0177bc48349d67afd900af73764f84841189b26b8c451f6f68e693fb915b9c25`). All 472
of its checks passed ([summary](corpus/make-corpus-summary.json)).

- **Graph:** `cargo tree --locked --offline` was run for x86_64-apple-darwin,
  aarch64-apple-ios, aarch64-apple-ios-sim and x86_64-apple-ios, with normal
  edges and with normal+build edges. All four targets give the same graph:
  - 7 linked crates: bitflags, fallible-iterator, fallible-streaming-iterator,
    libsqlite3-sys, rusqlite, smallvec, zeroize;
  - 6 build-only crates: sqlcipher-src (`build_native_c_source`), cc,
    find-msvc-tools, pkg-config, shlex, vcpkg.
  The other 22 lock packages (the wasm-only chain) are listed as excluded,
  with a reason.
- **Texts:**
  - Registry crate texts come from crate archives whose SHA256 equals the lock
    checksum.
  - rusqlite and libsqlite3-sys texts come from the locked git revision
    `661107f6…` via `git show`. `libsqlite3-sys/LICENSE` is a symlink to
    `../LICENSE`; it was resolved inside the tree and recorded as
    `symlink_target`.
  - Every text was reused only after it proved byte-equal to the Android copy
    of the same tuple.
  - SQLCipher's license is the verified `sqlcipher/LICENSE-sqlcipher` entry.
    The SQLite text is a byte prefix of the verified `sqlite3.h` entry.
  - `COPYRIGHT-library.html` (`68129500…`) comes from the pinned 1.98.1
    toolchain and is equal to the Android copy.
- **Crypto provider:** CommonCrypto and Security are recorded as
  `os_framework_not_redistributed`, with no texts. Their per-library
  imported-symbol evidence is bound to each library's SHA256. LibTomCrypt is
  `unused_source_not_linked`, with no text. Claiming it as linked rejects
  `wrong-platform`.
- **Toolchain:** the `rust-std` toolchain aggregate lists 19 linked sysroot
  crates. Its 76 verified rlib link inputs are hash-bound per Apple target,
  and the crates not named in the aggregate text are disclosed. The NDK
  aggregate and Android roles reject `wrong-platform`.
- **Archive members:** every member of every slice/architecture is
  classified. The categories are Rust std rlib objects, compiler_builtins C
  objects byte-equal to members of the target's rlib, `sqlcipher.o`,
  first-party objects and graph-crate objects. An unclassified member stops
  derivation.

**Correction (nm `--no-llvm-bc`).** An earlier lane statement listed 10
imported OS symbols. That list was incomplete. Apple's `nm` (LLVM 21) cannot
parse the LLVM 22 bitcode embedded by rustc 1.98.1 in 31 std members ("Unknown
attribute kind (105)"), so it skipped their symbol tables. With
`nm --no-llvm-bc`, every slice and architecture imports exactly 11 OS symbols:

- 9 CommonCrypto symbols, 8 of them from `sqlcipher.o`, plus
  `_CCRandomGenerateBytes` from Rust std;
- 2 Security symbols, `_SecRandomCopyBytes` and `_kSecRandomDefault`;
- 0 LibTomCrypt symbols.

The controller now fails on any nm diagnostic other than "no symbols". The b7
review's own pure-Python Mach-O parser independently confirms 11 imports and 0
LibTomCrypt symbols. The failed controller runs are preserved:

- attempt-1: symlink;
- attempt-2: nm exit status;
- attempt-3: bitcode.

## AN02/AN03: framework closure, orchestration and both interpreter modes

The finalized XCFramework has 40 files:

- the 10 native members, byte-identical to the parent record and to the
  preserved pre-notice copy;
- `native-build-parent.json`, a byte copy of the parent;
- `NOTICE.md` and `third-party/` (the index plus 26 texts) at the root,
  outside every slice's `Headers`;
- `packaging.json`, which lists every other member.

`validate-framework` treats the parent SHA256 as trusted external input:

- the parent copy inside the artifact must hash to it;
- native members are checked against that parent;
- notice members are checked against the source corpus bytes;
- `packaging.json` must equal the record recomputed from those trusted
  inputs.

A consistent rewrite of the library, parent copy and packaging record rejects
`parent-mismatch`. A consistent notice rewrite rejects `notice-mismatch`.

Orchestration tests run the actual `build-artifacts.py`, copied into a fresh
rebuilt-layout fixture with stub `cargo`/`xcrun`/`xcodebuild`. The build
finalizes the framework, and a broken corpus fails the build. The real
artifact was rejected `missing-packaging` before finalization
([real-artifact/red](real-artifact/red/)). It validated after finalization
through the same entry point ([real-artifact/green](real-artifact/green/)).
Both results hold in both interpreter modes.

## AN04: unchanged executable/API bytes and three-copy checks

- All 26 entries of the previous archive `6a2ca07b…` are byte-identical in the
  new archive. Against run-c `1ac8d113…`, only `README.md` differs, as already
  recorded. The 30 added entries are all notice/packaging members
  ([an04-entry-comparison.json](rn-pack/final-1/an04-entry-comparison.json)).
- Native libraries, headers, module maps and RN runtime sources are unchanged.
  The original Hermes 51/51 and Swift runtime captures remain historical, with
  their platform qualifiers. Nothing here is a new runtime claim.
- `pack.py` validated the source, package-copy and extracted copies (all
  packaging `47e27eb8…`). It then confirmed that the 16 non-framework entries
  are byte-identical to commit `ab68ce6` and that the framework member set is
  exact.
- `check_artifacts.py --expected-parent-sha256 c19233a6…` ran 5/5 OK on each
  of the Swift source, RN package-source and extracted copies, under `-I` and
  under `-I -O`. The checks include the two-copy Swift import typecheck
  ([rn-pack/final-1](rn-pack/final-1/)).

## AN05: archive source identity

The INVALID attempt-2 archive (`68608f9e…`) packed 8 untracked files from the
external `bindings/react-native/android/.gradle/` state. That state has
unknown ownership and was not deleted. The archive and its record are
preserved read-only.

The correction has three parts:

- **Exclusion:** `android/.npmignore` excludes `.gradle/` and `build/`.
- **Archive check:** before extraction, `pack.py` checks every archive member.
  The existing path, type and symlink checks still apply. The only generated
  exception is the exact validated XCFramework copy, with the same member set
  and bytes. Every other member must be byte-identical to its blob in one
  immutable commit: HEAD is resolved once at start and recorded as
  `head_commit`, and that object ID is used for every lookup. An untracked
  member rejects `unexpected-entry`. Different bytes reject `modified-entry`.
- **Explicit roots:** `--package-root` and `--repo-root` are authoritative.
  The Git work tree must equal `repo_root`, and inherited `GIT_*` variables are
  removed.

## TDD chronology

| Step | Kind | Result |
| --- | --- | --- |
| red-1 | harness error | fixture-name collision; only the test helper was fixed |
| red-2 | behavioral RED | stubs: every failure `not-implemented`, or build-artifacts left a notice-free framework |
| green-1 | GREEN | synthetic suite, both modes |
| red-3, red-3b | behavioral RED | check_artifacts lacked the flag and the closure test; one test was then made specific |
| green-2 | GREEN | both modes |
| red-4-relative-path | regression RED | a real pack failure: npm runs from the package directory |
| green-3 | GREEN 44/44 | frozen as [cut-44](cut-44/) and independently reviewed by b7 |
| an05-red-1 | setup failure, not behavioral | the shipped `.npmignore` did not exist yet |
| an05-red-2 | setup failure, not behavioral | the CLI root seam was absent |
| an05-red-3-seam-only | behavioral RED | untracked, modified and stray archives were packed and accepted |
| an05-green-1 | ordering failure | validator code loaded before the Git-root check |
| an05-green-2 | GREEN 50/50 | frozen as [cut-50](cut-50/) |
| an05-red-4-commit-pin | behavioral RED | the HEAD-move (B bytes) archive was accepted; original A bytes were falsely rejected; a foreign `GIT_DIR`/`GIT_WORK_TREE` redirected the root check (a control proves the redirection) |
| an05-green-3 | GREEN 53/53 | `-I` and `-I -O`; root re-ran them: 53.391 s and 52.944 s |

The step labels are in [tdd/AN05-LABELS.md](tdd/AN05-LABELS.md). Each step
directory holds its `sources.sha256`.

## Reviews and root verification

- **b7 AN01–AN04 static review** ([review/FINDINGS.md](review/FINDINGS.md),
  with its parser scripts and symbol output): no blockers within its stated
  limits. Its residuals R1–R7 are carried below.
- **AN05 focused reviews:** the initial prep gaps were closed, the commit-pin
  defect was corrected, and the correction review passed.
- **Root verification** ([root-verification/](root-verification/)):
  - 53 tests in both modes;
  - all 34 committed blobs equal the reviewed bytes;
  - the Swift artifact check;
  - 1084 staged whitespace findings, all classified as exact upstream corpus
    captures and none in first-party source
    ([wrapped](root-verification/staged-whitespace.txt.json)).

## Before the next native rebuild

The CommonCrypto/Security evidence in the index is bound to each library's
SHA256. A real native rebuild therefore produces libraries that `finalize`,
and so `build-artifacts.py`, reject with `stale-provider-evidence` (fail
closed) until the corpus is regenerated.

Reproduction procedure:

1. Rebuild with `build-artifacts.py`. It will stop at notice finalization and
   leave the new `native-build.json` and a notice-free framework.
2. Move the old corpus aside. Run
   `python3 -I target/phase-seven-rn/def-0030/make-corpus.py NEW_CORPUS SUMMARY.json`.
   The exact reviewed bytes are in [corpus/make-corpus.py.txt](corpus/make-corpus.py.txt).
3. Review the summary and the index diff. Install the new corpus, then run
   `apple_notices.py finalize` on the new framework with the new parent.
4. Repeat the validations, check_artifacts and the RN pack. Commit the new
   corpus.

Required local inputs:

- `bindings/native/Cargo.lock`;
- the Cargo registry cache at
  `~/.cargo/registry/cache/index.crates.io-1949cf8c6b5b557f/<crate>-<version>.crate`,
  for the 11 registry crates, checksum-verified;
- the rusqlite git database `~/.cargo/git/db/rusqlite-8e5e8204e489f2fd` at
  revision `661107f6eb5d7477c61be1221744de3ad96c53e7`;
- the Rust 1.98.1 toolchain at `~/.rustup/toolchains/1.98.1-x86_64-apple-darwin`:
  the Apple-target rlibs and `share/doc/rust/COPYRIGHT-library.html`;
- `bindings/android/third-party/` as the byte-agreement reference;
- Xcode `nm` (with `--no-llvm-bc`), `lipo` and `ar`.

Limitations:

- The controller is untracked under `target/` and uses absolute local
  cargo/rustup paths.
- It is not a permanent tool. Promoting it is a separate future proposal.

## Honest residuals

- `finalize` is not atomic. It writes the notice members before its final
  validation, so a post-write failure would leave a partly finalized tree.
  Pre-write checks make this unlikely.
- A malformed index can raise a non-`NoticeError` traceback (exit 1). It still
  rejects.
- The validator trusts the separately reviewed index derivation: graph,
  provider evidence and toolchain link inputs. The evidence `architectures`
  field is not independently validated by that pure validator. The LibTomCrypt
  wrong-platform check is a name match plus a symbol count of 0.
- `check_artifacts.py` accepts unittest name filters. RN pack calls the same
  `validate_framework` function directly.

## Evidence layout and presentation

Contents:

- [tdd/](tdd/): the TDD steps;
- [cut-44/](cut-44/) and [cut-50/](cut-50/): the frozen reviewed cuts;
- [corpus/](corpus/): the controller copy, runs and summary;
- [real-artifact/](real-artifact/);
- [rn-pack/](rn-pack/): attempt-1 relative path, INVALID attempt-2 and final-1
  records, without the archives;
- [review/](review/);
- [root-verification/](root-verification/);
- [assemble-packet.py.txt](assemble-packet.py.txt);
- [files-sha256.json](files-sha256.json).

Every file is a byte copy, and raw paths are kept.
[presentation.json](presentation.json) lists each source path and hash, and
names the 8 files stored as lossless JSON wrappers
(`{format, original_path, original_bytes, original_sha256, utf8}`) because
their original bytes fail whitespace checks.
