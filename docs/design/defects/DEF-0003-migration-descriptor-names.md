---
id: DEF-0003
title: Unselected migration descriptors omit nested name validation
type: defect
status: active
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [REQ-0041, SPEC-0026]
depends_on: [API-0004, THR-0015]
tags: [migrations, validation, regression]
---

# DEF-0003: Nested descriptor names

Review of the uncommitted migration planner found that descriptor checks cover
step table/column operands but omit AddTable's nested declarations and
SetRelations' relation names/operands. An unselected descriptor can therefore
contain a reserved or invalid name even though API-0004 requires validation of
all authored descriptors before a same-version no-op or initialization.

Expected: context-independent names and structural constraints reject as
MigrationFailure before any store operation, even when the edge is not selected.
Context-dependent existence/type checks still run against the selected source
schema. TC-0028 supplies the permanent expected-RED regression. The fix validates
nested declaration names without inferring migration steps or running an
unselected edge. No existing store or shipped data is modified by this finding.

## Correction evidence

TC-0028 permanently checks migration descriptor names in the shared conformance
suite; TC-0027 guards ambiguous relation aliases.
