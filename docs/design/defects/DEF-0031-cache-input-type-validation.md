---
id: DEF-0031
title: Cache record and capture field types escape structured rejection
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-26
parents: [SPEC-0034]
affects: [SPEC-0034, TC-0038]
regression_for: [TC-0038]
tags: [sdk, security, build-tooling, input-validation]
---

# DEF-0031: Validate cache record and capture types before using them

## Symptom and provenance

Given an otherwise valid Version1 cache record, replacing
`images[0].install_path` with an empty list makes
`native_cache.check_host(SystemCacheEvidence(record, {}, {}))` raise
`TypeError` rather than return a structured record rejection. Root reproduced
this with pure Python against both the current `cf2fa3b` implementation and
the pre-SR-22d source at `1a7d2c5`. The local diagnostic is
`target/phase-seven-security-remediation/unhashable-path-repro.json`.
No JVM, host reader, actual baseline or native payload was involved.

Root also reproduced two siblings at `cf2fa3b`: a null record `arch` raises
while concatenating the cache filename prefix, and a list/dictionary observed
library `name` raises during cache lookup or inventory lookup. The latter
also occurs without cache evidence when the entry carries a digest. The
local diagnostic is
`target/phase-seven-security-remediation/input-type-sibling-repro.json`.

The image-path bug is inherited from Version1, not introduced by the
dual-resident schema or its corrected missing-key handling. These exceptions
abort normal rejection
reporting; it does not produce a clean result or approve the malformed record.

## Confirmed root cause

In `packages/react-native/tools/security/native_cache.py`, `_image_errors`
detects an invalid path but continues to construct `set(paths)` for duplicate
detection. A list or dictionary path is unhashable, so set construction raises
before `check_host` can return the accumulated image-validation errors.

The same validation-order pattern occurs for `arch`: `_record_errors` records
its invalid type and then `_file_errors` concatenates it into a filename.
For observed library names, `kotlin_guard.check_observed_jvms` uses an
unvalidated name in `native_cache.lookup`'s set membership or in
`inventory.get`/`baseline.get`. Unhashable JSON values raise there.
The same entry boundary calls `lib.get` before verifying that the library
entry is an object; non-object entries belong to this narrow correction too.

## Required correction and scope

Validate image path and observed library-name types before hashing or lookup,
and validate `arch` before filename construction. Return an image-record or
architecture rejection, or an incomplete-capture library-name violation,
instead of raising. A rejected malformed name must be reported by field and
role without echoing its value in any `Result` field. Invalid entries and
names must enter no load bin or `discovered_images` list. Reject a non-object
library entry before reading its fields. Preserve valid Version1 and Version2 behavior,
exact identity/key checks,
duplicate-path detection, aliases, mandatory images and capture classification.
Do not catch arbitrary exceptions to make a record appear valid or replace
field validation with a generic success/fallback path.

Scope is these three field checks and the library-entry object check at the
same boundary in `native_cache` and `kotlin_guard`, plus their regression tests.
It is not a general observation-shape validation refactor. No host-reader or
collector work, image enrollment, runtime observation, dependency activation,
compiler execution or execution-hold change is part of this correction.
Rollback reverts unpublished validator/test changes while preserving diagnostic
and RED/GREEN evidence.

## Regression gap and named cases before implementation

TC-0038 SR-22b already requires malformed or missing image identities to reject.
Its tests at intake covered missing fields and several path constraints; a
regression for these three field-type failures was not found. No fix or new
automated regression had been written when this case list was committed.

```gherkin
Feature: Malformed cache image paths return record rejections
  Scenario: UP01 an unhashable image path does not escape validation
    Given an otherwise valid Version1 or Version2 cache record
    And an image install_path replaced by a JSON list or dictionary
    When check_host and the full observed-JVM guard validate the record
    Then they return a system cache record images rejection instead of raising
    And the full guard result is not clean
    And the unchanged implementation first fails this result assertion

  Scenario: UP02 all non-string JSON path types remain invalid
    Given an otherwise valid cache record for each supported schema
    When an image install_path is null Boolean numeric a list or a dictionary
    Then the record rejects with its image-validation identity
    And no value is coerced into a valid path

  Scenario: UP03 valid and duplicate path controls retain their meaning
    Given valid Version1 and Version2 fixtures and their existing capture controls
    When the corrected image validator runs
    Then their existing outcomes remain unchanged
    And duplicate valid string paths still reject as duplicate images
    And empty relative wildcard or traversal paths remain rejected

  Scenario: UP04 an invalid architecture never reaches filename construction
    Given an otherwise valid record for each supported schema
    When arch is empty null Boolean numeric a list or a dictionary
    Then check_host and the full guard return an architecture record rejection
    And neither raises or coerces the value into a cache filename

  Scenario: UP05 invalid observed names return incomplete-capture violations
    Given otherwise valid JVM observations with and without reviewed cache evidence
    When a library entry is not an object
    Or its name is missing empty or a non-string JSON value
    And the library entry is tested both with and without a digest
    Then the full guard rejects it with a role-specific library-name violation
    And no lookup or set operation raises
    And its invalid value is not echoed in any Result field
    And it enters neither discovered_images nor any load bin
    And valid payload baseline cache and dual-resident classifications stay unchanged
```

Write the regression first and observe a failure caused by the escaping
exception, converting that exception to an assertion failure in the test.
An import, fixture or unsupported-schema failure is not that RED. Run the
regressions and existing guard suites normally and under optimized Python,
with source identities recorded for both RED and GREEN before acceptance.

## Implementation verification

Validate every field before filename construction, set construction or native
library lookup. Preserve valid duplicate-path and payload/baseline/cache
classifications. Malformed entries must produce fixed role-specific violations
and enter no load or discovery bin. Test ordinary and optimized Python modes.

This closes UP01–UP05 only. General observation-container validation, such as
a non-iterable `native_libraries_loaded` container or malformed class entries,
remains outside this correction; the capture API is not claimed to be
universally raise-free. No image or baseline is enrolled, no collector or
host reader is implemented, and all execution holds remain. This is not a
final platform SDK review round.
