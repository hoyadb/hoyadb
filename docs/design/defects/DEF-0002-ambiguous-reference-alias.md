---
id: DEF-0002
title: To-one aliases sharing a key can disagree on their target
type: defect
status: active
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [REQ-0002, SPEC-0026]
depends_on: [API-0004, THR-0015]
tags: [schema, relations, regression]
---

# DEF-0002: Ambiguous to-one aliases

Review of the uncommitted Phase 2 schema normalizer found that two ToOne
declarations sharing a foreign-key column check only the Reference kind and
optional flag, allowing different target tables. API-0004 requires shared-key
declarations to agree; this can make one stored reference resolve to different
entity kinds depending on its alias.

Expected: aliases with the same key must have identical target and optionality.
Matching aliases remain valid; differing targets or optionality reject before
any metadata/table mutation. The fix records the complete shared-key mapping
during normalization. TC-0027 supplies a permanent expected-RED regression,
also exercised through the shared public schema boundary in TC-0026 D14.

No shipped data or user store is affected by this review finding. Rollback is
a code revert preserving data; no deletion/migration is required. Existing
ambiguous schemas are rejected rather than silently reinterpreted.

## Correction evidence — 2026-10-22

The permanent regression was observed failing against the original code, then
passing after the correction. It remains in the shared memory/native/browser
conformance suite (TC-0027 through D14; TC-0028 as its own case). See
[execution evidence](../../implementation/phase-two-data-model.md).
