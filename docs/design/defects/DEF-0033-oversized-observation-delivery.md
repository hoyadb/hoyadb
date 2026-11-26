---
id: DEF-0033
title: One oversized observation blocks other subscriptions and hides managed failures
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-26
parents: [SPEC-0033]
affects: [REQ-0007, REQ-0046, API-0011]
regression_for: [TC-0036]
tags: [purpose-audit, remediation]
---

# DEF-0033: One oversized observation blocks other subscriptions and hides managed failures

## Symptom and evidence

Two individually valid writes can make an observed snapshot exceed the 1 MiB frame
limit. The shared event prefix then rejects the first undeliverable event without
removing or transforming it. Later drains repeat LimitExceeded and all later events for
that owner remain blocked. The existing
g11_undeliverable_single_snapshot_remains_until_cancellation test demonstrates the
retained head. Automatic JavaScript delivery appends the error to an undeclared array,
Kotlin suppresses it, and Swift reports it to every listener.

## Confirmed cause and contract boundary

The prefix helper has no per-subscription oversized-event representation or failure
isolation. Managed drain error channels differ between SDKs. A bounded wire protocol is
intentional; silent owner-wide starvation is the defect.

## Required correction

Retain byte/depth/node limits and lossless successful data. Isolate an undeliverable
subscription, deliver a bounded typed error naming the owned subscription, and continue
healthy data/status/outcome delivery. Define explicit refresh/resubscription or recovery
behavior. Expose typed background sync/callback failure reporting consistently, without
masking committed mutation results. Query pagination or bounded windows must be explicit
and must not silently truncate rows.

## Regression gap and prep cases

OD01 oversized membership snapshot plus healthy data/status subscriptions; OD02 one
error and deterministic recovery without duplicate delivery; OD03 committed mutation
remains successful; OD04 JS/Swift/Kotlin managed notification and background error
parity; OD05 frame-exact terminal prefix and cancellation/replacement controls; OD06
bounded query-window guidance and tests. Current facade coverage intentionally retains
the blocked snapshot, and no reviewed SDK regression covers the integrated failure.

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
the independent Phase 7 security, advisory or execution gates.
