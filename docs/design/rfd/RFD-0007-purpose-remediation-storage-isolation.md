---
id: RFD-0007
title: Purpose remediation requires explicit store isolation and recovery scaling contracts
type: rfd
status: committed
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-26
parents: [OBJ-0001, OBJ-0002, OBJ-0003, OBJ-0004, REQ-0011]
depends_on: [ADR-0025, API-0006, API-0007, API-0009, DEF-0034, DEF-0035]
tags: [purpose-audit, storage, concurrency, compatibility]
---

# RFD-0007: Storage isolation and recovery scaling

## Context and authorization

The operator requested fixing all six purpose-audit findings and repeating
Claude Opus 5.5 correctness, fix and pertinence reviews until three consecutive
issue-free rounds. That authorizes remediation, tests and review. It does not
silently settle the accepted app-wide serialization decision or authorize
conversion/reset of existing user stores. CHG-4 requires the concrete
architecture/contract choice below; no accepted ADR body is rewritten here.

The source-bound public counterexample at 7058b29 shows a queued or active
request on owner A makes independent owner B's local insert Busy. The existing
ADR-0025 explicitly accepts one handler across the whole app. API-0006 additionally
makes all mutation permits global. Separately, command admission copies all
application rows, and complete recovery-row replacement plus sorted contiguous
metadata ordinals makes repeated fixed-page bootstrap work grow quadratically.

The preparatory Opus storage review rejected implicit format conversion,
misleading trait-boundary cost counters, a new SHA dependency, and validation
removal disguised as optimization. Those are not accepted shortcuts.

## Choice I: serialization boundary

### I-A — store-scoped handler FIFO (recommended for complete isolation)

Serialize handlers and direct/entity/pull/reconciliation mutations within one
logical store, while independent stores remain independently writable and may
run independent handlers concurrently. Preserve atomic commit, read-your-writes,
stale-scope rejection, same-store FIFO, cancellation and close/drop cleanup.
Every owner/request identity remains globally unambiguous across coordinators.

This changes ADR-0025's cross-app guarantee. A successor ADR must explicitly
record it, REQ-0011 and affected draft contracts/cases must name the store scope,
and SDK module-level queues must become owner-scoped. Handlers on different
stores may now interleave in application code; applications relying on app-wide
serialization for cross-store invariants lose that guarantee and must coordinate
those invariants themselves. Native exclusive ownership makes one owner correspond
to one persistent store. In the browser, enable store-scoped admission only after
an actual supported-facade duplicate-owner rejection test; otherwise retain the
conservative global browser coordinator. Do not infer cross-tab synchronization
or multi-writer support from this choice.

A host can still stall its own store with an unbounded handler; explicit
cancellation/abort and optional bounded host deadlines are the liveness tools.
No library can preempt arbitrary non-yielding application code. This boundary
prevents that handler from making unrelated stores Busy; it is not a promise
that the handler itself terminates.

### I-B — retain app-wide handler FIFO, scope only other mutation permits

Keep the accepted global handler slot and FIFO. Make plain/entity/pull/open
permits owner-scoped, fixing the demonstrated independent plain-write failure.
Other stores' commands remain blocked by a hung or abandoned request. This is a
compatible partial correction, not closure of the complete cross-store
availability finding. JavaScript queues must still be owner-scoped for independent
plain writes; core request admission retains global handler FIFO under this
option. No invented default timeout hides the remaining command blockage.

## Choice F: comparison metadata format

### F-A — stable format for new ledgers, explicit conversion only (recommended)

Introduce one unambiguous new comparison-metadata format for newly created entity
ledgers. Packet identity is an injective bounded encoding of table and record id,
not an ordinal or an unapproved hash dependency. No SHA implementation/dependency
is added to the dependency-free core. A single format marker and exact key/data
checks reject mixed, malformed and unknown formats; ordinary command-only
format-1 behavior remains unchanged. "New entity ledger" means first entity
configuration where no entity-format state previously existed. Reset and rebind
of a format-2 entity store remain format 2. A pre-existing command-only store
adopting entity sync for the first time receives the new format and therefore
requires reverse conversion before an older binary can read that entity state.

Existing format-2 ledgers are read and written in their existing format on open;
opening never converts, resets or deletes anything. Their legacy ordinal-suffix
cost remains explicit. Provide an independently reviewed, offline-tool-only forward/reverse converter,
with validation, atomicity, dry-run and rollback tests on synthetic fixtures.
Do not add a public SDK auto-upgrade or conversion-on-open API. The accountable
operator or an integrating developer may run the tool only through a separately
authorized, backed-up, offline maintenance procedure with no live owner. Building
and testing it is distinct from running it on any user's store; no user-store
conversion is authorized by this RFD. Older binaries reject the new format rather
than stripping fields.

The exact new marker/key codec and bounds require their own reviewed spec and
RED cases before implementation; this RFD approves no guessed bytes.

### F-B — retain format 2 everywhere

Reduce redundant scans and writes where possible, retaining all sorted
contiguous metadata ordinals. Arbitrary-order insertion still rewrites the
ordinal suffix, so the unconditional linear bootstrap criterion cannot be met.
For append-order keys this can reduce persistence to O(changed packets) per
page; arbitrary key order still needs O(all metadata packets) in the worst case,
so total fixed-page bootstrap remains quadratic in that case. This is a
compatibility-first partial improvement, not a full scaling fix.

### Rejected shortcut: conversion on ordinary open

Do not silently convert an existing store. No automatic reset, destructive
fallback or migration operation on user data is authorized.

## Integrity-preserving implementation constraints

The proposed fast path uses an exclusively owned store with full validation on
command-capable open, reset and migration. For native SQLite, exclusive locking
mode must be followed by BEGIN EXCLUSIVE and COMMIT during open, before owner
publication, so the connection actually acquires and retains its lease. External
readers and inspection tools are blocked until owner close; expose that change
and preserve the ownership failure through protected-open error mapping. Validate each touched/mutated row and
changed recovery packet before atomic commit. Compare exact prior model/header
and touched packet/row images. Treat the exclusive lease and validated cached
state as preconditions, not caller-supplied completeness flags.

Full content validation must actually occur at command-capable open: the current
catalog checks alone do not validate every field. This means one O(all application
rows plus recovery metadata) validation pass on every such open. It deliberately
moves cost to startup and does not promise constant-time open, lazy launch or a
latency target. A latent malformed untouched row now makes command-capable open
fail; previously that open could succeed and its first command then fail. There
is no automatic repair, reset, export or unprotected fallback. Existing lower-level
storage/model read interfaces remain separate and unchanged; whether they can
read unaffected rows depends on their existing validation and the specific
corruption. Startup time and memory must remain visible in the later measured
baseline; narrow-operation fitness functions begin after successful open. A corrupt untouched row must
be rejected there, with test coverage. Lease failure must be preserved through
the protected-open error mapping. Unknown/custom adapters must retain the existing
complete validation path unless they implement and demonstrate the reviewed
capability. Browser exclusivity is unproven and must be tested or retain its
current full-validation path and RISK-0003 limitation.

Moving checks is a reviewed trust-model change. Document precisely when untouched
out-of-band edits are detected. Do not claim disk tamper detection stronger than
the engine supplies, or weaken encrypted-page authentication/key/path controls.
No source change starts until the detailed invariants/cases have independent
review. If those invariants cannot be preserved, keep the conservative path and
report the performance requirement unmet.

## Acceptance and relevance

- Independent stores make progress while one store has a queued or active
  handler, under I-A; same-store concurrency controls remain exact.
- Native duplicate ownership rejects while the first owner keeps working, and
  crash/close releases ownership without stale lockfiles or data loss.
- Actual adapter/internal counters measure rows scanned/read/written, ledger
  packets encoded/parsed/visited and cloned entries. Trait call counts alone
  cannot establish the work bound. No contended timing is acceptance evidence.
- A narrow command/patch/page performs no work over unrelated application rows
  or metadata on the selected fast path. Read queries may naturally visit their
  candidates/results; a reset intentionally visits all data.
- Stable packet identity makes arbitrary-order fixed-page bootstrap perform
  work proportional to its actual changes. Existing legacy-format limits remain
  visible, with explicit conversion rather than a false universal claim.
- Memory is part of the owned fast-path claim: replace incidental full candidate
  copies with touched-entry journaling or an equivalently tested atomic mechanism;
  do not silently exclude first-party memory stores from the work denominator.
  Full reset/open/conversion remains intentionally proportional to total data.
- Memory/native/WASM, failure/restart and stale-plan controls cover supported
  capabilities; unexecuted targets are unknown and never counted as verified.

Functional Suitability, Reliability and Security preserve integrity and bounded
failure. Performance Efficiency uses deterministic work counts, not a new time
SLO. Compatibility and Flexibility make old-format/custom-adapter behavior
explicit. Interaction Capability gives distinct ownership errors and actionable
limits. Maintainability uses shared semantic helpers and regression oracles.
Safety has no physical-control application; no user data is touched here.

## Approval and rollback

The operator explicitly approved I-A per database and F-A. The first answer was
"can be per database if it's what 1 means"; root confirmed same-database
serialization and independent-database progress. The second was "Approve
new-ledger format and offline converter (Recommended)". ADR-0037 and ADR-0038
record those directions. Independent Opus review and detailed specs/cases still
precede implementation; no actual user-store conversion is authorized.

Code-only changes revert with their contracts and tests. New-format stores need
the explicit reverse converter before an old binary can read them. Stores retaining only their prior format-1/2
state remain format-compatible; an existing store newly adopting format-3 entity
sync also needs reverse conversion before downgrade, even without converting an
older entity ledger. Preserve every RED,
GREEN and review record. Existing D1/D2 security decisions, compiler/advisory
holds, dependency approvals and release gates remain unchanged.
