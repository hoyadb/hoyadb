---
id: SPEC-0032
title: Phase 6 engine encryption backup capabilities and redacted boundaries
type: spec
status: accepted
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [ARC-0001, ARC-0002]
satisfies: [REQ-0015, REQ-0028, REQ-0030, REQ-0031, REQ-0032, REQ-0045]
depends_on: [RFD-0005, SPEC-0014, API-0006, API-0009, API-0010, TC-0012, TC-0015]
tags: [phase-six, security, encryption]
---

# SPEC-0032: Phase 6 engine encryption backup capabilities and redacted boundaries

## Scope and acceptance

Implement IMPLEMENTATION_PLAN Stage 8 / roadmap Phase 6 for the existing Rust,
Swift desktop and dedicated-worker OPFS targets under RFD-0005 recommended
policies. User authorizes those recommended choices; exact new dependency graph
approval, security review and committed contract/cases precede activation.
Keep core/memory dependency-free and all previous user/formatting work intact.

1. Review exact cipher source/entropy/provider/build/dependency/license changes;
   pin maintained authenticated SQLCipher on both engines with no private cipher.
2. Add supplied-key protected open below storage traits. Missing/invalid/wrong
   keys and foreign/plaintext formats fail before initialization/mutation. Verify
   profile, no plaintext fallback, no automatic migration/rekey/key management.
   Foreign pending opens reserve their client and dispose late acquired handles
   before close completes, without stale owner publication (API-0010 S10).
3. Protect main data and durable queue/checkpoint/comparison metadata and sidecars;
   keep temp storage memory-only. Test canaries, page tampering and unkeyed reads
   as well as functional behavior, transactions and process/reload recovery.
4. Implement actual Swift directory backup hint default/opt-in/readback/failure
   and replacement policy, and truthful browser unsupported capability.
5. Add allowlisted bounded diagnostics, redacted owned Debug/error boundaries,
   immutable foreign context snapshots, no telemetry/network/automatic export.
6. Run TC-0035 mapping original TC-0012/TC-0015, actual native/browser hosts,
   prior regressions, all formatting/Clippy/WASM/examples gates, fresh pinned
   container, trace/RTM and final bounded Claude Opus issue-focused review.

Each behavior change uses compiling meaningful RED then minimal GREEN. Never
weaken existing assertions, publish a plaintext build as protected, silently
accept unsupported mechanisms or describe an unexecuted host/cipher gate as done.
Report source/artifact hashes, exact counts and real limitations. Scope stops
before Phase 7; no deployment, push, merge or publication.

## Quality and rollback

Functional/reliability: same API/storage semantics under encryption and atomic
failure/recovery. Compatibility: no implicit plaintext conversion, explicit
legacy unprotected fixtures, fixed keys/format and checked upgrade. Security/
privacy: authenticated engine, scoped threat review, no key/output leaks, honest
backup limits. Performance: measure protected open/boundary costs without new
SLOs or a Phase 7 benchmark project. Maintainability/flexibility: passive core,
existing host seams, pinned upstream integrations. Safety: no physical actuation
claim, confidentiality/data-loss risks treated explicitly.

A source revert cannot read new cipher files with an older/plain SQLite reader.
Keep files and the application's original key; use a compatible reader. Do not
rotate, decrypt, erase, reset or migrate user stores. Owned synthetic fixtures
are the only data used for destructive corruption/restart procedures.

## Verification status — 2026-10-24

Operator-approved ADR-0034 policies and exact graph are implemented and reviewed.
[Phase 6 execution evidence](../../implementation/phase-six-security.md) records
protected native/Swift/OPFS, fault/restart, privacy and pinned-container checks,
and the final crypto plus lifecycle Claude reviews. Scope and residual limits
remain explicit; this status does not certify new mobile targets or publication.
