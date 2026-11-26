---
id: DEF-0027
title: Git normalizes certified Android notice and raw evidence bytes
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-25
updated: 2026-10-26
parents: [SPEC-0033]
affects: [SPEC-0033, API-0011]
regression_for: [TC-0036]
tags: [phase-seven, android, packaging, provenance]
---

# DEF-0027: Git normalizes certified Android notice and raw evidence bytes

## Observed behavior and cause

Before the Android SDK milestone commit, comparing all225 staged files with
their working bytes found three mismatches. With the existing user Git setting
core.autocrlf=input, Git converted CRLF to LF in the vcpkg0.2.15 LICENSE-APACHE
and LICENSE-MIT under bindings/android/third-party/rust/, and in
docs/implementation/phase-seven-kotlin-final/package/cleanup.txt. Their original
working SHA256 values are respectively 60c93a31…, 016d20f3… and b6d35087…;
staged values are 3708458d…, ff7d9943… and db8564d0…. No incorrect SDK commit
was made. The worktree and certified archives remain intact.

P04's exact upstream notice comparison and the recorded raw evidence hashes
would fail after checking out these normalized Git blobs. No existing test
exercised Git's clean/checkout conversion for these certified inputs.

Independent prep review also found the same two vcpkg notices under
licenses/javascript/ already normalized in HEAD. Their working
bytes retain the upstream CRLF hashes above, matching the committed portable
declaration evidence's mirror_sources and packed license entries. HEAD contains
the LF hashes above instead. Git status reports these paths clean because its
clean filter hides the byte difference; raw blob comparison is required.

## Intended correction and scope

Preserve all bindings/*/third-party/** notice corpora and this exact raw log
through scoped Git text-conversion attributes. Explicitly restore the two JS
vcpkg HEAD blobs from the upstream working bytes already bound by the verified
packages and evidence; do not substitute LF hashes or regenerate those packages.
Keep source files and other documentation under
their established formatting conventions. Do not change Git user/repository
configuration, whitespace checks, package notices, certificate hashes, runtime
code or the certified AAR. Upstream license whitespace stays verbatim.

## Named regression before implementation

G28 (extends TC-0036 packaging provenance): Given every file enumerated from
bindings/*/third-party/** (including JS and Android), the exact cleanup log and
an isolated temporary Git index based on HEAD, when
the files pass through Git's clean and checkout filters with core.autocrlf set
to input, true or false, then each staged and checked-out byte sequence equals
the original input. The test must first fail on all five conversion-sensitive
paths with input. Separately pin the JS upstream working bytes to the committed
portable-declarations record's mirror_sources hashes before restoring its blobs.
It must not change the real index, working files or Git configuration.
Also prove an ordinary LF source remains subject to text conversion under
core.autocrlf=true, so the fix cannot silently disable conversion globally.

The permanent regression belongs outside the certified Android tool closure,
under tests/tooling/, and retains no temporary index/output on success or
failure. Its results describe Git storage only, not a new mobile runtime run.
Rollback reverts the attributes and regression in unpublished history; preserve
the upstream bytes and provenance rather than recertifying altered text.

## Correction and observed results

After corrected independent prep approval, tests/tooling/git_provenance.py
first failed121 real filter comparisons:10 with input (the five paths, both
clean and checkout) and111 with true, including LF notices changed on checkout.
With scoped -text attributes, the complete input/true/false matrix passes in
ordinary Python and under -O; the normal-source conversion control also passes.
The two JS vcpkg upstream blobs are restored explicitly. No source/runtime or
certified package bytes change. The [evidence](../../implementation/phase-seven-git-preservation/README.md)
records commands, test/log hashes, RED cases and unchanged Android artifact
hashes. The later iOS audit found that the separately reported sidecar
differences were already explicitly mapped as presentation normalization in the
approved dependency snapshot. That snapshot remains unchanged; the
[clarification](../../implementation/phase-seven-ios-notice-clarification.json)
distinguishes its raw and presentation identities from this Git-filter defect.

## Primary-source license evidence extension

When staging the new security triage, the same clean filter changed
docs/implementation/phase-seven-security-remediation/sources/licenses/streamex-0.7.2-LICENSE.
Its captured upstream CRLF bytes match the frozen triage index, but Git's staged
blob did not. The27-file triage staging was removed without changing its working
bytes or index. No normalized evidence was committed.

Extend G28 and the -text preservation class to raw captured license texts under
docs/implementation/**/sources/licenses/**, in addition to the original binding
corpora and exact cleanup log. These are primary source copies with recorded
byte hashes; this excludes deliberately derived iOS presentation sidecars.
Before the attribute extension, enumerate these directories in the existing
permanent regression and observe the real StreamEx clean/checkout failure.
Then add only that scoped attribute and rerun the complete three-mode matrix,
including the ordinary-source conversion control. Preserve the earlier test
source hash and results as the earlier cut; record this extension separately.

## Captured license directories outside `sources/`

Staging follow-up 3 reproduced the same failure for
`docs/implementation/phase-seven-security-remediation-followup-3/licenses/jdom-2.0.6.1-LICENSE.txt`.
Its raw CRLF bytes still match the frozen packet index; the staged blob did not.
Only the newly staged follow-up-3 paths were removed from the real index, and
no normalized copy was committed. This historical license capture is not
evidence that the compiler's IntelliJ JDOM fork is jdom2.

Extend the same G28 dynamic enumeration and `-text` scope from
`docs/implementation/**/sources/licenses/**` to
`docs/implementation/**/licenses/**`. Require the real captured JDOM bytes to
fail the clean/checkout comparisons before the attribute change, then pass all
three conversion modes normally and under optimized Python. Retain the
ordinary-source conversion control. Directories named `component-licenses`
with deliberate presentation mappings remain outside this exact directory-name
class. No package, source runtime, frozen evidence index, user Git setting or
whitespace-check suppression changes.
