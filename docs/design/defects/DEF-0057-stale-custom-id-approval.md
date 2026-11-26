---
id: DEF-0057
title: Active SDK guidance still marks the approved custom-ID policy pending
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-31
updated: 2026-10-31
parents: [SPEC-0047]
affects: [API-0011, REQ-0046]
regression_for: [TC-0051]
tags: [documentation, compatibility]
---

# DEF-0057: Approved custom-ID policy is still marked pending

API-0011's proposed-ID section and RFD-0006 P4 still say the unprefixed command
example needs operator clarification. Accepted ADR-0035 records the approval
and explicitly requires exact `local:test-fixed-id-001` for command-created IDs
and exact `test-fixed-id-001` for ordinary unconfigured CRUD. Its ID-policy gate
is cleared. Treating that decision as unanswered needlessly blocks acceptance.

Correct active guidance with an explicit supersession link. Preserve accepted
ADR bytes, the historical scenario and historical pre-approval question; never
relax the core's surviving-command prefix guard or weaken exact-ID assertions.
Verify the nearby actual tests and record any remaining execution gap separately.

## Correction and evidence (2026-10-31)

API-0011 now states the approved exact command and ordinary CRUD values, and
RFD-0006 P4 labels its original question historical with the ADR-0035 resolution.
ADR-0035 and TC-0016's historical scenario remain unchanged. Source inspection
confirmed exact assertions in `crates/hoyadb-core/src/command/owner.rs` for both
ordinary and command insertion, `crates/hoyadb-bindings/tests/general.rs`, and
the Swift and Kotlin testing-helper cases. This documentation correction does
not require or authorize weakening those assertions. Fresh execution is recorded
in the follow-up implementation evidence, not inferred from this source check.
