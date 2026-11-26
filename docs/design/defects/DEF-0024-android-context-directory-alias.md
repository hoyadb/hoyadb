---
id: DEF-0024
title: Android protected opens reject an aliased operating-system data directory
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-25
updated: 2026-10-25
parents: [REQ-0046, SPEC-0033]
affects: [REQ-0046, SPEC-0033, API-0011]
regression_for: [TC-0036]
tags: [phase-seven, android, protected-open]
---

# DEF-0024: Android protected opens reject an aliased operating-system data directory

## Observed behavior and cause

The outside-checkout Android APK rejected protected opens with InvalidRequest.
Diagnostics located `Database.openProtected`'s requirement that the OS-supplied
Context base directory already equal its canonical path. That is false for
the installed app's aliased base. A plain-directory fixture did not reproduce
the failure; a fixture with an aliased ancestor did.

## Correction and existing regression

The Kotlin implementation now resolves the OS base once, then still requires
its owned hoyadb child to be canonical, within that base and not a symlink.
The aliased-ancestor regression changed RED to GREEN; the symlinked-child
rejection remains passing before and after the fix. No native/JNI/provider
change or user-store conversion was made.

The source, original APK failure, non-reproducing fixture, behavioral RED/GREEN,
final50-test suite and23-group APK pass are bound in
[the Android evidence](../../implementation/phase-seven-kotlin-final.md).
TC-0036 already requires real protected consumer opens and path-policy checks;
the installed-app ancestor shape was missing from the earlier test coverage.

This DEF is recorded after the lane's implementation. It does not claim that
this record preceded the fix or replace the preserved test chronology. Final
independent matching-artifact review remains outstanding. Rollback reverts the
unpublished Kotlin change; existing files and encryption keys are preserved.
