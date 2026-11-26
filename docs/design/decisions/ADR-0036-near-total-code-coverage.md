---
id: ADR-0036
title: Require code coverage close to 100 percent with justified gaps
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-10-25
updated: 2026-10-25
parents: [ARC-0001, ARC-0002]
depends_on: [SPEC-0033, TP-0001]
tags: [quality, testing, coverage]
---

# ADR-0036: Require code coverage close to 100 percent with justified gaps

## Context and source

After learning that no measured line or branch coverage percentage was
available, the operator instructed: "Add a decisions that coverage must but
close to 100%. No implementation yet."
Passing test counts establish neither a coverage percentage nor complete
behavioral verification. This decision records the quality requirement now;
coverage instrumentation, new tests and CI enforcement are subsequent work.

## Decision

In the context of verifying hoyadb across its core and platform bindings,
facing an unmeasured coverage baseline, we decided to require measured code
coverage close to 100%, targeting 100% of project-owned executable code, accepting
only narrowly scoped, explicitly justified gaps reviewed with their risks.

- Measure line and branch coverage separately. Report results by component and
  language for the Rust core, storage adapters, facade, platform wrappers and
  first-party executable tooling. A high aggregate cannot hide an unmeasured
  or poorly covered component.
- Identify the source revision, tested targets, instrumentation scope, covered
  and total counts, excluded paths and uncovered regions. An unsupported metric
  or unexecuted target remains unknown; it does not count as covered.
- Third-party vendored and mechanically generated code may be excluded with a
  recorded inventory and rationale. First-party logic must not be relabelled,
  removed from the denominator or suppressed merely to raise the percentage.
- Every remaining first-party gap needs a concrete reason, affected behavior,
  risk, alternative verification where available, and reviewer acceptance.
  Security, atomicity, recovery and concurrency paths require particular
  scrutiny; ordinary happy-path tests cannot justify leaving their failure
  paths unexplained.
- Coverage complements meaningful assertions, regression tests and actual
  platform execution. Executing a line is not proof of correctness. Preserve
  the existing TDD and no-weakened-tests requirements.
- Once measured, preserve coverage floors and review regressions under
  [TEST-9](../../../ai-governance/rules/09-testing.md#test-9--coverage-is-a-floor-not-a-target).
  The 100% target does not require pointless tests or override that rule's
  allowance for a justified, reviewed gap. The operator supplied no separate
  numeric lower floor; this decision does not invent one.

## Consequences and later confirmation

Future implementation must prepare its measurement scope and acceptance cases
before choosing or adding coverage tools. Evidence must expose remaining gaps
against the 100% target and demonstrate that reports and any enforcement reject
missing, stale or misleading results. These checks have not been implemented
or executed, and current coverage remains unknown.

This documentation-only decision authorizes no dependency activation, test
implementation, CI change, platform build or new benchmark run. Existing exact
compiler-bundle approval and Android runtime gates remain separate. A future
change to this policy is recorded through a new decision rather than rewriting
this accepted decision silently.
