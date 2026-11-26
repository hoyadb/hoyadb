---
id: SPEC-0031
title: entity sync entity diffs conflict resolution resumable pull and explicit reset
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [ARC-0001, ARC-0002]
satisfies: [REQ-0023, REQ-0024, REQ-0025, REQ-0026, REQ-0027, REQ-0028, REQ-0029, REQ-0044]
depends_on: [RFD-0004, SPEC-0010, SPEC-0011, SPEC-0012, SPEC-0013, API-0006, API-0007, API-0008, DM-0003, RISK-0005]
tags: [entity-sync, entity-diff, conflict, bootstrap, reset]
---

# SPEC-0031: Full roadmap entity sync

Origin: operator requested full entity sync after the formatting maintenance handoff.
RFD-0004 proposes choices for accepted artifacts' explicit open questions.
The operator approved those choices; ADR-0033 adopts them. The exact additive API/storage/host contract
must be reviewed and committed before its tests and implementation. This file
sets the delivery scope and acceptance; it does not claim any implementation.

## Scope and stop condition

Complete IMPLEMENTATION_PLAN Stage 7: entity diffs through the existing durable
queue; per-field timestamp/default deletion resolution and custom table hooks;
server authority on equal timestamps; membership-scoped pull; specified paged
bootstrap, atomic checkpoint/data/progress recovery, and total explicit reset.
Support the current unpublished Rust API and experimental actual Swift and
browser worker hosts, using the same Rust production owner and SQLite adapter.
Preserve all platform validation–4 behavior outside explicitly adopted contract extensions.

Core remains dependency-free and passive. Backend membership/authentication,
authoritative business handlers and server arrival ordering remain integration
responsibilities. No production SDK, new binding target, independent concurrent
writers, encryption, remote release or roadmap security/7 work. No task operation
may reset/migrate existing user data; verification uses owned synthetic stores.

## Ordered delivery and acceptance

1. **Reviewed policies and contracts.** Resolve RFD-0004 with operator approval
   and adopting ADR. Specify exact discriminated ledger/codec compatibility,
   ordinary-write table opt-in/context, patches, wire correlation, resolver
   inputs/outputs, pull tickets/page/progress and reset/host generation APIs.
   Add REQ-0044/THR-0018 controls and TC-0034 before runtime code. Preserve the
   unrelated formatting pass and pre-existing user CI/skills changes.
2. **Automatic durable entity path.** Configured tables generate exact partial
   patches without app-built envelopes. Same ordered queue, capacity/backoff,
   status and outcome/inverse semantics as commands; no idempotencyKey. Handle
   repeated record IDs and different tables without guessing response order.
   Creation/mapping, Reference dependencies and frozen retries survive restart.
   Failures atomically preserve rows, counters and recovery state.
3. **Resolution and authoritative input.** Persist field clocks/tombstones,
   merge distinct fields, prevent default resurrection, expose per-table hooks
   with bounded validated output and no unresolved client tie-break. Test real
   independent client sequences through a deterministic backend fixture and
   actual production local apply; no claim of library-owned backend execution.
4. **Pull and bootstrap.** The host transports a null initial checkpoint and
   exact subsequent opaque tokens. Serialize page commit with local writers,
   use latest local state and preserve a conflicting whole page unchanged.
   Commit page data/checkpoint/progress together; malformed/stale/oversized/
   interrupted pages cannot skip data. Real native restart and browser reload
   resume partial bootstrap; subscriptions expose committed partial data and
   explicit readiness. UI queries never change transport membership requests.
5. **Reset and composition.** PreserveRecovery rejects live recovery. Explicit
   DiscardRecovery clears all local tables and recovery/checkpoint/comparison
   state atomically, retains schema/configuration and never reuses identity.
   Old subscriptions, pull tickets and cached host replies become inert only
   after successful commit. Busy/failed resets preserve everything. Verify
   awaited handlers, sends, pull responses and callbacks around the boundary.
   Reopening a different store leaves the original untouched.

Each slice requires a compiling meaningful behavioral RED, minimal GREEN,
review and recorded output. Existing test failures, compiler errors or harness
timeouts are not acceptance evidence. Commit reviewed spec/cases first. Keep
evidence tied to exact sources/artifacts, runtime profile and observed versions.

## Verification and quality

TC-0034 maps every original TC-0011/0013 scenario and mixed TC-0014 cases, plus
the newly chosen behavior. Execute shared conformance on memory/native SQLite/
browser OPFS, physical transaction faults and independent native processes,
real suspended Swift/browser host integrations, actual HTTP pull/push and page
reloads. Run the existing workspace and isolated spike regressions, examples,
all five formatting/Clippy gates, WASM builds, fresh pinned-container validation,
trace checker and RTM. Final diff and independent issue-focused review required;
one bounded Claude checkpoint is useful if available, not a findings quota.

| ISO/IEC 25010 characteristic | entity sync treatment |
| --- | --- |
| Functional Suitability | TC-0034 and original entity/scoping/mixed-array cases |
| Performance Efficiency | Measure actual page/queue/bridge costs and metadata growth; enforce bounded messages, no invented latency SLO |
| Compatibility | Old queue-free/command sync readers supported by new code; explicit opt-in format extension; old code rejects new metadata; unchanged private physical DDL |
| Interaction Capability | Typed conflicts, explicit partial/ready progress and destructive-reset choice; no new visual product UI |
| Reliability | One transaction for data/checkpoint/ledger, exact retries, restart, stale-response and inverse guards |
| Security | REQ-0044/THR-0018, scope binding, trusted context, bounded decode, no client authorization or implicit purge |
| Maintainability | One queue/owner/adapter, shared conformance, reviewed contracts and source-linked evidence |
| Flexibility | Integrator-owned transport and table merge hooks; existing supported hosts; no new runtime dependency |
| Safety | No physical control; stale data/replay/data-loss hazards tested without claiming backend or power-loss guarantees |

## Rollback and limitations

Revert only entity sync source changes. Existing files remain; use a compatible
reader for entity sync metadata, never strip unknown recovery to appease old code.
Default reset preserves pending recovery; explicit discard is an application
policy and cannot undo remote effects. Tombstones are retained until reset;
measure/disclose that footprint instead of inventing safe compaction. Plaintext
at-rest and backup hardening remain security. Remote CI is reported only if seen.

## Execution evidence
