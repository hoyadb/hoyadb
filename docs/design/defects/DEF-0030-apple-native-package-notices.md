---
id: DEF-0030
title: Swift and React Native Apple artifacts omit native third-party notices
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-26
parents: [SPEC-0033]
affects: [SPEC-0033]
regression_for: [TC-0036]
tags: [phase-seven, packaging, provenance, apple, react-native]
---

# DEF-0030: Apple native package notice closure

## Observation and scope

The focused static integration review found no third-party notice corpus in
the Swift XCFramework or the RN package that contains it. Root independently
listed the current framework: its ten files are the three static libraries,
six headers/modulemaps and Info.plist. The recorded run-c RN archive has26
entries and likewise no native notice corpus. The current RN archive differs
only in README. The static libraries contain third-party native code, while
TC-0036 G23 requires matching dependency and license evidence for the actual
packaged target. Android already has an explicit P04 notice closure; that
does not place notices in the separately distributed Apple artifacts.

This is a packaging completeness gap, not a legal conclusion or a runtime
failure. Do not invent a first-party project license or claim an SBOM/legal
certification. The source and binary identity records remain historical.

## Required correction

Keep a version/source/hash-bound native notice index and exact upstream texts
under crates/hoyadb-native/third-party, with a NOTICE explaining roles and limits.
Derive roles from the actual Apple target graph and existing archive/build
provenance. Reuse Android/JS texts only after their source tuples and original
bytes agree. Distinguish linked Rust/native providers, build-only inputs and
toolchain aggregate attribution. Do not attribute Android-only runtime/toolchain
inputs to Apple. Disclose aggregate scope and any attribution gaps accurately.
Bind the crypto-provider role to actual symbols: these Apple libraries import
CommonCrypto and Security from the OS, not redistributed LibTomCrypt code.
LibTomCrypt may be recorded as unused source within sqlcipher-src, never as
linked Apple code. The Rust std aggregate applies; the NDK aggregate does not.

Place NOTICE and the indexed corpus inside the XCFramework's root, outside
slice Headers, so the Swift artifact and RN package's existing Artifacts tree
carry them with the binary. Validate the exact expected set and bytes before
packaging and after extraction. Header/modulemap layout and native library
bytes must remain unchanged. Update packaging provenance with a clear parent
artifact identity and new notice/package identities, not a claim of fresh
native compilation or a rewritten historical runtime result.

Keep the current native-build.json byte-exact as the compilation parent. A
separate packaging.json at the XCFramework root binds its SHA256 and a
byte-identical native-build-parent.json copy shipped inside the artifact. The
packaging record lists every member except itself: the original ten native
members, parent record, NOTICE and full corpus. check_artifacts.py verifies
native members against that parent and the complete additional set against
the packaging record and notice index. No broad root-member exclusion or
packed-copy skip substitutes for either check.

build-artifacts.py must always invoke the shared notice-packaging entry point
after xcodebuild creates the native framework and records its native members.
The same entry point may repackage today's validated artifact without native
recompilation, but preserves its original compilation parent. RN packing must
verify the result before making the tarball. A future real native rebuild
produces its own truthful compilation parent and necessarily includes the
validated notices; a one-off copy into today's framework is insufficient.

## Named cases before implementation

- AN01: Given the exact native locks, Apple target dependency roles and provider
  archive provenance, when the source notice corpus is validated, then every
  declared component has classified provenance and exact license bytes.
  Missing, changed, unclassified or wrong-platform attribution rejects.
  In particular, claiming linked LibTomCrypt on Apple rejects; record OS
  CommonCrypto/Security separately from redistributed providers.
- AN02: Given the current notice-free artifact, when the actual packaging
  validator checks it, then the missing corpus is a behavioral RED. After the
  correction, validate the full expected notice set and hashes in the source
  XCFramework, RN package-source copy and extracted npm archive. A removed,
  changed or unexpected notice fails; directory existence alone cannot pass.
  Exercise the shared packaging function called by build-artifacts.py on a
  fresh rebuilt-layout fixture, and verify RN uses the same final validator,
  so the next build cannot silently revert to a notice-free artifact.
- AN03: Given a copied artifact and notice source, when the actual validator
  runs normally and under optimized Python, then it enforces the same exact
  closure. Use synthetic owned fixtures for rejection tests, without fetching
  dependencies or compiling native code.
- AN04: Given the prior sealed library/header/modulemap and RN runtime-source
  hashes, when the notice-bearing artifact and RN tarball are produced, then
  these executable/API bytes remain identical. Record the changed package
  identities and added notice members. Preserve the original runtime captures
  and unsupported-platform qualifiers; do not relabel them as new execution.
  Rerun check_artifacts.py, including its two-copy Swift import typecheck, on
  both the notice-bearing source artifact and the extracted RN copy.
- AN05: Given the RN package's existing `android` files entry and an owned
  fixture containing untracked `.gradle/` and `build/` state, when the actual
  npm packaging entry point runs, then neither state directory enters the
  tarball. Add a scoped `android/.npmignore` for those directories, without
  deleting or changing external state or the package.json files list. Test
  with the actual offline npm command, not only a mocked archive list.
  Before extraction or acceptance, every archive file outside the independently
  validated `Artifacts/HoyaNative.xcframework/` tree must name a Git-tracked
  file in this package and be byte-identical to its committed `HEAD` blob.
  Resolve that commit once before packaging, record its immutable object ID,
  and use that same ID for every member lookup even if HEAD later moves.
  A regression must move HEAD between root resolution and member checking:
  newer bytes absent from the recorded commit reject, while its original
  bytes remain attributable to that recorded commit. Git queries ignore
  inherited `GIT_*` overrides so the supplied repository root is authoritative;
  cover a foreign Git directory/worktree environment in an isolated fixture.
  A tracked but locally modified source must reject; tracked path membership
  alone is not provenance. Name and test any actual npm content transformation
  explicitly before accepting it; no generic package.json exception is implied.
  An untracked file elsewhere rejects `unexpected-entry`
  and preserves the rejected archive and failure record. Verify tracking in
  isolated synthetic Git fixtures. Give the real pack function explicit
  package-root and repository-root inputs (with existing production defaults),
  and query Git in that supplied repository, not the tool's location. Fixtures
  must not rely on a copied tool residing at one particular directory depth.
  Retain existing archive path/type checks
  and complete framework validation. A tracked source file still packages,
  and changed generated framework members cannot use this exception to pass.

The first notice-bearing RN packing attempt revealed eight externally created
untracked Gradle-state files in its tarball. That attempt is invalid evidence,
not a deliverable. Keep its archive and earlier captures unchanged, then
produce a fresh tarball after AN05's genuine RED/GREEN. Package-input exclusion
is the correction; removing files of unknown ownership is not authorized.

Observe meaningful RED before implementation, then GREEN through the actual
packaging entry points. The root corpus is covered by the existing third-party
-text Git attribute; verify staged blobs preserve original bytes. This adds
no dependency, runtime operation, native rebuild, publication or user-store
change. Rollback restores unpublished packaging sources/artifacts while
retaining prior archives, manifests and evidence cuts.
