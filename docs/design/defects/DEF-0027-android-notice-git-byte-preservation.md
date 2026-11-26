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
tags: [sdk, android, packaging, provenance]
---

# DEF-0027: Git normalizes certified Android notice and raw evidence bytes

## Observed behavior and cause

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

## Captured license directories outside `sources/`
