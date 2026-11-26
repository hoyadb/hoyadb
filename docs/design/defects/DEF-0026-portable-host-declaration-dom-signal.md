---
id: DEF-0026
title: Portable JavaScript host declarations require an undeclared DOM AbortSignal
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-25
updated: 2026-10-25
parents: [REQ-0046, SPEC-0033]
affects: [REQ-0046, SPEC-0033, API-0011]
regression_for: [TC-0036]
tags: [phase-seven, typescript, declarations, react-native]
---

# DEF-0026: Portable JavaScript host declarations require an undeclared DOM AbortSignal

## Observed behavior

The packed RN/client/testing declaration probe passes with ES2022+DOM but fails
with ES2022 alone: the shared client execute declaration references the global
AbortSignal, which is absent. The recorded error is TS2304 at index.d.ts:101.
The reviewed RN runtime types do not provide it. The preserved
[declaration evidence](../../implementation/phase-seven-rn-declarations/README.md)
uses the established TypeScript7.0.2 verifier, strict/noEmit, types:[], no plugins
or inherited configuration, and no skipped library checks.

## Cause and intended correction

The portable host declaration in packages/client/index.d.ts couples
its cancellation option to a browser-only type name. Runtime client.js consumes
only the cancellation capability's aborted flag and abort listener registration
and removal; establish its exact usage before defining a structural type.
Express that existing capability without requiring DOM ambient declarations or
new dependencies. Browser AbortSignal values must remain assignable; preserve
current runtime cancellation and handler result behavior. No event loop, retry,
transport or AbortController implementation belongs in this correction.

## Named regressions before implementation

1. A strict packed portable-host/testing/RN consumer with ES2022 only and no
   ambient DOM/node types compiles using a capability matching actual runtime
   usage. Observe the current TS2304 as behavioral RED before the correction.
2. A strict browser consumer with ES2022+DOM still accepts an actual
   AbortController.signal and existing public options; invalid capability
   shapes reject through genuine negative type assertions.
3. Existing runtime cancellation tests remain unchanged and pass. The
   declaration correction does not change runtime JS, native code or WASM.
4. Repacked consumers resolve only the local packages, typecheck with noEmit and
   no skipped library checks, and retain source/artifact manifests. Prove every
   non-declaration runtime byte unchanged rather than infer new browser runtime
   results from a changed package hash. Preserve old packets and footprints as
   historical cuts; record new artifact identities separately.

The new regression extends TC-0036's portable public-package compatibility
coverage; the existing DOM-enabled probe did not cover this environment.
Rollback reverts unpublished declarations/package outputs. No runtime dependency,
product cancellation semantics, customer store or persistent format changes.

## Correction and evidence

The declaration now uses an inline structural cancellation capability matching
the runtime's exact listener/aborted accesses, with no new global/export or
dependency. Packed ES2022 host/testing and RN probes first fail with TS2304;
after the correction they and the DOM control pass, including negative shapes.
The unchanged runtime suite passes84 tests and the packed real WASM memory
consumer passes. [Evidence](../../implementation/phase-seven-portable-declarations/README.md)
preserves the exact before/after commands, type diagnostics and tool hashes.

Root independently compares all105 packed files: only the shared client
index.d.ts changes. All100 non-declaration files, including JS, WASM and source
manifests, remain byte-identical. The new client tarball is `fb710c70…`;
earlier browser packets and footprint numbers keep their original identities.
No new browser/mobile runtime execution is inferred from this declaration fix.
