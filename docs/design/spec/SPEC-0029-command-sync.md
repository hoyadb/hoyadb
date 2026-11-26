---
id: SPEC-0029
title: command sync command execution and durable sync delivery
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-23
updated: 2026-10-23
parents: [ARC-0001, ARC-0002]
satisfies: [REQ-0010, REQ-0011, REQ-0012, REQ-0013, REQ-0014, REQ-0015, REQ-0016, REQ-0017, REQ-0018, REQ-0019, REQ-0020, REQ-0021, REQ-0022, REQ-0028, REQ-0029, REQ-0033, REQ-0034, REQ-0043]
depends_on: [RFD-0003, SPEC-0008, SPEC-0009, SPEC-0013, SPEC-0015, API-0004, API-0005, DM-0002, DM-0004]
tags: [command-sync, commands, sync]
---

# SPEC-0029: command sync delivery

Origin: operator request to resume command sync, following reactivity closure at 682ca06.
RFD-0003's policies are now operator-approved and adopted by ADR-0032. Each
implementation slice still requires its exact reviewed API contract before test
code or runtime edits; completed slices and actual evidence are recorded in the
command sync implementation report.
The public API remains unpublished Rust with experimental Swift/browser bridges.

## Scope and stop condition

Deliver IMPLEMENTATION_PLAN Stage 5 local command isolation and Stage 6 durable
command queue/reconciliation end to end on memory, native SQLite and browser
OPFS. Exercise actual async Swift/browser handlers, durable independent-process/
page restart, deterministic fake transport and host HTTP reference. Preserve
existing schema/query/reactive semantics, private SQLite DDL and runtime graph.

entity sync entity diffs/LWW/pull/bootstrap/membership reset, release encryption,
additional platforms, packaged SDKs, independent multi-tab/store writers and
production publication remain outside this phase. Existing feature scenarios
that explicitly require entity diffs are mapped as entity sync, not counted green.
Do not implement a server business-handler substitute or library networking.

## Ordered slices and acceptance

1. **Local owner/scopes:** application-wide FIFO coordinator; handler-owned
   staged view supports the existing public query/relation semantics and ordered
   writes. Committed readers/drains see no partial state. Abort/failure/close,
   explicit cancellation, re-entry and stale ticket behavior follow P1. Real
   Swift and browser hosts prove async suspension and no serialization bypass.
2. **Envelope/atomic enqueue:** bounded JSON/context/key behavior follows P2;
   commit local changes, bookkeeping/identity/counters and envelope together.
   Exact new metadata validation is additive; no raw storage escape. Inject
   ordinary storage failures and kill independent clients immediately before/
   after commit. No success result is returned for failed durability.
3. **Durable transitions/dependencies:** persist P3/P4 graph and state, recover
   sent and acknowledged-awaiting-outcome separately, freeze actual outgoing
   bytes, reject malformed/future metadata unchanged. Verify multi-parent and
   transitive chains, interrupted mapping transactions and ID collisions.
4. **Transport/reconciliation:** a developer-supplied transport receives arrays
   of eligible frozen envelopes in execution order. Fake and reference HTTP use
   key correlation, independent outcomes and missing/duplicate/reordered replies.
   Implement P5 validated mapping/correction/explicit guarded rollback and the
   ordinary reactive notification path. Do not resend after durable receipt.
5. **Status/cancel/scheduling:** one status projection includes pending/sending,
   blocked local IDs, awaiting outcomes, resolution attention and bounded recent
   results without leaking payloads by default. Status/outcome observation uses
   owned drains and lifecycle guards. P6/P7 lifecycle, capacity, tombstones,
   cancellation cascades and deterministic backoff hold across restart.

For each slice: commit reviewed exact contract/cases, write a meaningful
behavioral RED, implement minimally, observe GREEN and preserve commands/output.
Missing imports, compilation errors and unavailable tools do not constitute RED.
Do not label a local-only scope implementation full command sync.

## Evidence and quality

TC-0032 maps original acceptance to new explicit boundaries. Planned test homes:
shared conformance under tests/conformance, adapter integration under the existing
crates, and actual host fixtures under spikes/cross-platform/native/tests and
spikes/storage-browser. Final exact file names/commands are established in the
reviewed API/test slice before adding them. No unexecuted command is evidence.

Run the existing workspace tests, fmt, Clippy, core/memory/browser WASM builds,
native Swift and browser OPFS harnesses, fresh pinned-toolchain container checks,
trace checker and generated RTM. Retain all platform validation–3 regressions. Review final
diff and independently review real issues without requiring findings. Record
actual counts/runtimes/versions and any unverified platform or crash boundary.

Functional Suitability: TC-0032 and original command cases. Performance Efficiency:
measure staged view/storage/boundary costs, no invented SLO; global serialization
and snapshot costs disclosed. Compatibility: additive opt-in metadata, old-reader
rejection and no changed private DDL. Interaction Capability: typed errors,
explicit progress/cancel/conflict flow in the upcoming API contract, no visual UI.
Reliability: atomic queue/data/state, restart, exact retry and guarded rollback.
Security: REQ-0043/THR-0017, immutable context, reserved barriers, bounded decode,
owned messages and no logging/telemetry. Maintainability: one core state machine,
shared assertions and traceability. Flexibility: supplied host transport/runtime,
dependency-free core. Safety: no physical control; partial state, replay and data
loss hazards are tested explicitly, without claiming backend deduplication.

Rollback: source revert without deleting stores; new metadata requires compatible
readers. Do not reset live recovery work. Only synthetic task-owned files and OPFS
namespaces may be created/removed by verification. Existing CI edits are excluded.
