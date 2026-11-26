---
id: SPEC-0033
title: platform SDK general platform SDKs testing support and measured baseline
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-11-07
parents: [ARC-0001, ARC-0002]
satisfies: [REQ-0046, REQ-0034, NFR-0001, NFR-0002, NFR-0004, NFR-0005]
depends_on: [RFD-0006, API-0011, API-0010, SPEC-0015, TC-0016, TC-0017]
tags: [sdk, bindings, packaging, performance]
---

# SPEC-0033: platform SDK general platform SDKs testing support and measured baseline

Complete ROADMAP platform SDK / IMPLEMENTATION_PLAN Stage9 under reviewed RFD-0006,
after security closes and exact new graph approval. API-0011 fixes the consumer
boundary before implementation; TC-0036/37 define behavior and measurement.

1. One general Rust facade forwards existing engine traits and owners, with
   explicit memory/protected engine selection. General caller schema/migrations,
   value/query/operation codecs are validated in core. First-poll readiness is
   proved for synchronous bridge operations; OPFS initialization remains async.
2. Browser JS/TS/worker, Swift macOS+iOS, Kotlin/JNI Android and official RN iOS+
   Android expose the same concepts and owned events/async scoped handlers.
   Demonstrate two unrelated schemas, not a renamed hardcoded fixture.
3. Separate supplied-key binary open path, generation-safe pending-open/close,
   canonical owned paths, truthful backup controls, redacted diagnostics/errors
   and actual reviewed Android provider preserve security's floor.
4. Optional proposed insertion IDs use shared core validation/allocation without
   foreign callbacks under borrow or replacing generator custody. Existing Rust
   generator APIs and strict entity allocator remain. TC-0016 prefix clarification
   is pending operator choice; no silent relaxation or acceptance rewrite.
5. Passive owned core custom-merge sessions preserve sequential pull semantics,
   the existing custom validator and complete stale-state/atomic checkpoint
   guards. No host LWW, metadata clock editing or lock/permit across callbacks.
6. QueueEntry attempts/next_due_ms and host scheduling expose durable retry state;
   bounded injected jitter/clock drive tests, production entropy drives wrappers.
   API-0010 count-only diagnostics and all recovery/cancel/retention rules remain.
7. Hermetic testing helper uses real core memory and the same fake/real transport
   seam, attaching exact correlations. Map every TC-0016 and TC-0017 scenario, original
   applicable conformance and security cases, to actual supported runtimes.
8. Local npm/SwiftPM/XCFramework/AAR/RN packages build in independent consumers;
   preserve old fixtures and disk formats, establish0.1.0/ABI1 snapshots, no
   implicit upgrade reset. Actual support claims distinguish runtime/build-only
   targets and known multi-instance limitations. No external publication implied.
9. TC-0037 baseline protocol produces optimized protected engine/API, memory,
   disk/package and boundary measurements, raw samples/source/runtime manifest;
   only then adopt concrete OBJ-0004/NFR-0001 budgets and a failing regression check.
   Document known configurable platform/wire/page/queue limits and actual
   footprint without inventing a mobile database size ceiling; NFR-0004's numeric
   capacity remains uncommitted until measured and reviewed.
10. All relevant checks, fresh container/mobile/browser consumers, licenses,
    source/artifact closure, documentation and bounded issue-focused Claude review
    precede complete-phase claim. Any unexecuted required target remains open.

Every new behavior uses compiling meaningful RED→minimal GREEN; build/tool setup
errors are not behavioral RED. No runtime installation/activation occurs solely
because these preparation artifacts exist.

## Additive Android ABI packaging for existing consumers

Monstera declares `armeabi-v7a`, `x86`, `arm64-v8a` and `x86_64`. Local Hoya
integration must preserve that selection. Extend the existing approved native
provider/JNI recipes with the two 32-bit targets; keep the current two-ABI AAR
default and add explicit ABI selection for a four-ABI package. Unknown,
duplicate or missing selected slices reject. No runtime dependency, wire
operation, storage format or protection guarantee changes.

Each selected slice retains the exact export, allowed NEEDED, API24 provenance
and minimum 16KiB LOAD alignment gates. Independent ELF inspection requires
ELF32/ARM for `armeabi-v7a`, ELF32/Intel 80386 for `x86`, and the existing ELF64
machine checks for the two 64-bit ABIs. Hash-bind the additional compiler/sysroot
inputs and retain source/artifact evidence. Existing accepted artifacts remain
historical; build a separate fresh package after tooling changes.

TC-0036 G23 requires actual protected persistence, pending-write recovery and
durable entity-alias reconciliation/restart in a 32-bit process before claiming
32-bit runtime support. Build-only certificates do not imply these results.
Reverting the additive recipe leaves the earlier two-ABI build path available;
it cannot justify dropping an existing application's declared ABI support.
