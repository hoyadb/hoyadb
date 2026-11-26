---
id: DEF-0037
title: React Native backup flags conflict across stores and platform reopen paths
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-26
parents: [SPEC-0033]
affects: [REQ-0045, REQ-0046, API-0010, API-0011]
regression_for: [TC-0035, TC-0036]
tags: [purpose-audit, remediation]
---

# DEF-0037: React Native backup flags conflict across stores and platform reopen paths

## Symptom and evidence

The iOS HoyaHostOpen path sets NSURLIsExcludedFromBackupKey on the shared Application
Support/hoyadb directory for each open, so a later opener rewrites that shared policy.
Android selects filesDir or noBackupFilesDir based on the flag; reopening one name with
a changed flag selects a different store path. These are static findings in the
production host code, not fresh mobile-runtime observations.

## Confirmed cause and contract boundary

The policy is per open in the API but per directory in each OS integration, with
different directory identity rules. No cross-store collision or changed-flag reopen
policy is enforced or tested.

## Required correction

Define and test one explicit store identity and backup-policy rule. Mixed named stores
must not silently change one another's policy, and reopening with a changed flag must
not appear to lose existing data. Preserve path canonicalization, key preflight,
fail-closed symlink checks and OS verification. Any migration or deletion of existing
user stores remains separately unauthorized; compatibility must reject or safely retain
them.

## Regression gap and prep cases

BK01 two named stores with opposite policies in either open order; BK02 reopen one name
with changed flag; BK03 existing legacy directory and policy failure; BK04 canonical
paths, symlinks, invalid inputs and key cleanup; BK05 OS-level behavior on actually
executed platforms with held targets honestly unverified. Current acceptance covers a
single policy selection, not collision/reopen semantics.

The concrete correction spec and declarative cases must be independently reviewed before
permanent regression tests or implementation. This intake is not a fix or a passing
result. Each implementation starts with a source-bound behavioral RED, then the smallest
justified GREEN and existing surrounding controls.

## Compatibility and rollback

No existing user store is migrated, reset, deleted or opened during remediation.
Preserve byte budgets, redaction, owned lifecycle and restricted-build controls. Revert
task-owned source and documentation changes together if the correction fails review,
preserving all diagnostic and RED/GREEN evidence. Any on-disk format or public semantic
change needs its explicit reviewed compatibility policy.

## Completion

Open. The operator requested fixing the audit findings and repeating Claude Opus 5.5
correctness and purpose reviews until three consecutive issue-free rounds. The
clean-review count starts at zero and resets after a new finding. This does not waive
the independent platform SDK security, advisory or execution gates.
