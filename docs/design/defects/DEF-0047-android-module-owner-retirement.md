---
id: DEF-0047
title: Android module can publish an owner after invalidation
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-30
updated: 2026-10-31
parents: [SPEC-0046]
affects: [API-0011]
regression_for: [TC-0050]
tags: [review, regression]
---

# DEF-0047: Android module can publish an owner after invalidation

## Symptom

The 2026-10-30 review recorded SDK-4: The module checks live separately from adding an owner; invalidation between those steps closes no owner, and the late callback retains and resolves it.

## Acceptance

Serialize owner publication and module retirement, reject retired opens and
close each acquired owner exactly once across those publication/retirement
interleavings, with deterministic host tests. A separate explicit close racing
retirement may issue a harmless duplicate close against a never-reused handle.

## Root cause and fix

The open reply checked the atomic live flag separately from registering its
owner. Invalidation could clear the owner set between those operations. One
module lifecycle lock now orders registration and promise settlement against
retirement; retirement drains the owner set under that lock and closes its
snapshot outside the lock. A reply arriving after retirement closes its acquired
owner and rejects `Closed`.

`packages/react-native/tools/android/test_module_lifecycle.py` compiles the
actual module with controlled host dependencies. Before the fix, its deterministic
mid-publication case failed with `publication race leaked or double-closed its
owner`; the other three cases passed. After the fix, all four passed under normal
and optimized Python execution: already-retired admission, pending native reply,
mid-publication invalidation, and post-publication/idempotent invalidation.
The ten existing React Native JavaScript host tests also passed.

This is Java lifecycle evidence with host stand-ins, not Android-device, JNI,
Codegen or ordinary React Native consumer certification. Status remains draft.

