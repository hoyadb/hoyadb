---
id: DEF-0025
title: React Native factories ignore retained stores and configured queue options
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-25
updated: 2026-10-25
parents: [REQ-0046, SPEC-0033]
affects: [REQ-0046, SPEC-0033, API-0011]
regression_for: [TC-0036]
tags: [phase-seven, react-native, factory, memory, sync]
---

# DEF-0025: React Native factories ignore retained stores and configured queue options

## Observed behavior and cause

Actual Hermes consumer runs failed the retained-memory factory case: duplicate
open of the selected store did not return Busy. The factory hard-coded
store:null instead of forwarding the caller's handle. Both memory/protected
receipt-wake cases also failed on their future retry deadline: the factories
read storeOptions while the shared declaration and callers provide options,
so configured retry delays were silently dropped.

## Correction and existing regression

The memory factory forwards the exact store handle, with null remaining the
fresh ephemeral default. Both factories forward options under the existing
shared OpenOptions contract; public RN declarations match it. No second
database, recovery policy, clock or retry algorithm was introduced.

The retained regression includes duplicate open/drop Busy, failed migration
preserving the original store, successful authored migration, ephemeral default
and explicit drop. The genuine Hermes RED preceded the fix; the final run
passes51 cases plus independent protected writer/reader processes. Details,
including consumer-only fixture corrections, are retained in
[the RN iOS evidence](../../implementation/phase-seven-rn-ios/README.md).
TC-0036's retained-memory and configured sync behavior was already specified;
the public RN factory forwarding path was not covered by the Node channel tests.

This DEF is recorded after the lane's implementation and preserves that
chronology. Declaration typechecking and final independent matching-artifact
review remain separate gates. Rollback reverts unpublished RN source only;
no persistent format, new dependency or customer-store action is involved.
