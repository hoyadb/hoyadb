---
id: SPEC-0035
title: Recover queued retries after a corrected forward wall-clock jump
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-26
parents: [SPEC-0029]
satisfies: [REQ-0018, REQ-0043]
depends_on: [API-0006, API-0007, DEF-0036]
tags: [purpose-audit, retry, clock, recovery]
---

# SPEC-0035: Bounded retry clock recovery

## Need and verified baseline

DEF-0036 records a public-API counterexample: a forward sample followed by
corrected samples leaves a Sent record waiting for the erroneous future time.
The retry planner currently uses max(sample, header.last_ms) for both identity
ordering and scheduling. Keep identity monotonic; stop using that floor as the
scheduler clock. This is a functional correction, not a new latency target.

## Scope and behavior

Keep the existing prepare_push(now_ms, jitter_basis_points) signature, supported
UTC year range, configured retry policy, states, frozen envelopes and keys.
No new persisted field, transport operation, dependency or implicit reset.
Acknowledged records remain excluded from automatic resend; explicit final-outcome
recovery is a separate part of DEF-0036 and is not closed by this slice.

For a valid raw sample r:

1. Validate the existing timestamp and jitter ranges before any mutation.
2. Retain header.last_ms = max(previous header.last_ms, r) for identity ordering.
3. Compute C = max_delay_ms + floor(max_delay_ms * 2500 / 10000) in i128.
   This is the maximum delay the existing planner can produce, including jitter.
   The recovery ceiling is min(i64::MAX, r + C), computed in i128.
4. For every Sent record, replace due_ms with min(due_ms, recovery ceiling).
   Do not change attempts, frozen bytes, key, outcome or other record state just
   because a deadline was capped. Queued, Acknowledged and terminal records are
   not capped as if they were pending retries.
5. Decide due eligibility against r, not header.last_ms. A due or Queued record
   uses the existing backoff and supplied jitter; its next due is r + delay with
   the existing checked overflow error. Failed planning persists nothing.
6. Persist a cap even when this call sends no envelope, through the existing
   atomic owner plan. Return the capped earliest Sent deadline in next_due_ms.

A deadline produced at sample r0 with valid delay d <= C is never shortened by
this correction for later samples r >= r0: r + C >= r0 + d. This proof relies
on retry options remaining equal to the persisted options; owner reopen rejects
conflicting options. A backward sample whose existing deadline is within r + C
retains that deadline. Its eligibility can nevertheless differ from the old
floor-based planner: when r < due_ms <= last_ms, the old planner sent immediately,
while the corrected planner waits for raw samples to reach due_ms (at most C away).
This is an intentional scheduling behavior change, covered by CS09. A cap after
a larger rollback never moves a deadline later; increasing corrected samples
therefore do not keep extending the wait. This does not invent a trustworthy
wall clock: a clock that never advances cannot establish elapsed time. Existing
extreme policy/checked-arithmetic limits remain explicit errors rather than a
wrapped or fabricated timestamp.

## Integrity and compatibility

The command identity/key floor, exact frozen envelope and idempotency key remain
unchanged. Restore and validate existing format-1/2 records with the existing
strict decoder. No relaxed validation, packet conversion or user-store touch is
part of this change. Older binaries can read the unchanged format, but retain
the older scheduling bug. The deadline adjustment can produce an earlier retry
after clock correction, so the already-required backend key deduplication remains
essential. No network request is implemented in the core.

API-0006 dispatch timing must describe the raw scheduling sample and bounded
recovery ceiling; API-0007 last_ms remains a monotonic identity floor rather than
an effective current time. Existing exact deadline/jitter controls stay green.

## Acceptance and TDD

TC-0039 supplies named state-transition, boundary, restart and atomicity cases.
The first regression must fail on the unchanged implementation because the
corrected retry deadline remains beyond the legal recovery ceiling, not because
of a missing symbol, fixture or import. Capture implementation/test/interpreter
or toolchain hashes before and after RED and GREEN. Execute shared public-owner
cases on memory and SQLite; unit boundary cases exercise the pure queue planner.
Keep old c16 deadline, jitter and frozen-byte assertions unchanged. Run focused
surrounding suites after the change, then the full workspace at integration.

## Quality attributes and rollback

- Functional Suitability: due work resumes under corrected progressing samples.
- Performance Efficiency: one existing bounded-record traversal; no extra store
  scan or wake loop and no numeric latency promise.
- Compatibility: unchanged API/record shapes and ordinary-clock deadlines.
- Interaction Capability: next_due_ms exposes the actual retained deadline;
  no hidden immediate retry or false success.
- Reliability: cap and retry state are atomic and restart-stable.
- Security: bounds, redaction and stable deduplication identity remain; no new
  trust boundary, transport, secret or input source is introduced.
- Maintainability: the cap formula and ordinary-clock proof have direct tests.
- Flexibility: existing injected clock and retry options remain in use.
- Safety: no physical-control behavior exists; preserve all queued data on error.

Rollback reverts task-owned source and contract text together. No format rollback
or migration is needed. Keep all diagnostic and RED/GREEN evidence. This spec does
not authorize Android/JVM/Gradle execution or alter any security approval/hold.
