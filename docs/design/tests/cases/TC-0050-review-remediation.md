---
id: TC-0050
title: Review fixes preserve lifecycle and validate public inputs
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-30
updated: 2026-10-31
parents: [TP-0001]
verifies: [SPEC-0046]
tags: [review, regression]
---

# TC-0050: Review remediation

## FIX01: Explicit application sync survives lifecycle cancellation

DEF-0044 (SDK-1): Cancel the SDK-owned wait on stop/reset/close, await owned work where required, suppress late results and let a successful reset synchronize again.

## FIX02: Application sync repeats mapping after admission contention

DEF-0045 (SDK-2): Retain the exact response and mapped proposal across Busy, use release-aware retries and cancel retained work on lifecycle invalidation.

## FIX03: Failed database open retains an inaccessible owner

DEF-0046 (SDK-3): Validate static sync options before owner acquisition and dispose any owner acquired by a later failed open without losing retryable cleanup state.

## FIX04: Android module can publish an owner after invalidation

DEF-0047 (SDK-4): Serialize owner publication and module retirement, reject
retired opens and close each acquired owner exactly once across those
publication/retirement interleavings, with deterministic host tests. This does
not add deduplication of a separate explicit close racing retirement.

## FIX05: Swift artifact instructions omit the required trust anchor

DEF-0048 (SDK-5): Document a complete workflow that supplies an independently trusted compilation-parent hash, preserving the trust gate and honest artifact freshness limits.

## FIX06: Browser guide contradicts fresh-worker recovery

DEF-0049 (SDK-6): Align active guidance with fresh-worker recovery, shared concurrent attempts and permanently closed old handles; preserve historical evidence.

## FIX07: Clean JavaScript CI lacks required WASM assets

DEF-0050 (REL-01): Build ordinary WASM with pinned tooling and supply it to the full JavaScript suite; document the same prerequisites for contributors and verify clean-source execution.

## FIX08: Quality evidence includes generated and excluded crate sources

DEF-0051 (REL-02): Scope snapshots and coverage inventories to actual root-workspace members and their first-party inputs, excluding generated outputs while detecting real source additions and drift.

## FIX09: Traceability validation silently accepts invalid scan inputs

DEF-0052 (REL-03): Reject missing/non-directory roots, unreadable or undecodable candidates and unterminated frontmatter; continue allowing ordinary Markdown without artifact frontmatter and report errors coherently in the RTM consumer.

## FIX10: Active roadmap instructions require a private process guide

DEF-0053 (REL-04): Point active workflow instructions to CONTRIBUTING.md and preserve historical references and accepted ADR bodies.

## Verification record

DEF-0054 extends application concurrency coverage: a pending key settled between
list and advisory context reads does not fail the listing or background pass.
Other pending entries and genuine context/lifecycle errors remain visible.

DEF-0055 restores meaningful native contention coverage: a same-database handler
must retain the response through Busy and invoke transport/mapper only once;
an independent-database twin must complete before the other handler is released,
as required by ADR-0037. Existing assertion timeouts must not be weakened.

Existing DEF-0040 additionally uses TC-0040 OD04/OD07 and TC-0044 CP13 for the
Kotlin per-database background sink. Cover observation, callback, sync, merge and
timer sources, including failed unsubscribe and managed drain; redact fields,
contain sink failures, suppress stale epochs and preserve committed outcomes.

Intake precedes regression tests and fixes. Record actual RED/GREEN outcomes,
commands, cross-review results and unverified scope in the implementation report.
Documentation corrections use nearby behavioral tests or executable instructions;
they do not require tests that merely duplicate prose.

