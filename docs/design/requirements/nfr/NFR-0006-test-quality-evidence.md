---
id: NFR-0006
title: Coverage and mutation evidence expose and preserve verified test quality
type: nfr
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-29
updated: 2026-10-29
parents: [OBJ-0002]
depends_on: [ADR-0036]
tags: [maintainability, testability, coverage, mutation]
---

# NFR-0006: Measured test quality

Origin: the 2026-10-29 operator request for a documentation/API/test audit,
coverage improvements, mutation testing and a Claude Opus 5.5 challenge.
This applies ADR-0036 and TEST-9 without changing either policy.

## Scale and meter

Measure covered/total executable lines and branches independently for each
component/language. Report instrumentation limits, source identity and missing
targets. Inline unit-test source and generated glue must be distinguished when
interpreting an aggregate. Execute tests and preserve raw coverage evidence.

For a declared critical mutation selection, measure caught / (caught + missed).
Publish selected and completed counts, unviable results and timeouts separately.
Do not count a timeout or build failure as caught. An empty or incomplete run
does not establish a score. Execute the unmodified baseline first.

## Past, must and target

- **Past:** no measured repository baseline recorded by ADR-0036 at adoption;
  this change measures an initial scope, with all other scopes unknown.
- **Must:** evidence must reject missing/stale source, impossible counts,
  incomplete mutation selections and failed baselines. A mutation selection
  cannot pass its gate with survivors or timeouts. Preserve established scoped
  coverage floors under TEST-9 when those floors have been measured and recorded;
  do not silently lower or redefine them.
- **Target:** ADR-0036's close-to-100% line and branch coverage with only explicitly
  justified, reviewed gaps; all viable critical selected mutations caught by
  assertions. A selected score is never a whole-project assurance claim.

No fixed numerical lower coverage threshold was supplied by the operator.
This draft does not invent one, accept unmeasured platform gaps, or certify a
release. The implementation report must name remaining measurement/floor work.

## Verification

SPEC-0044 and TC-0048 define report validation, baseline execution, mutation
experiments and public-contract checks. Nonmatching public method names require
compatibility review under NFR-0005, not silent renaming to satisfy a checker.
