---
id: RFD-0003
title: command sync command execution and durable reconciliation policies
type: rfd
status: committed
owner: contact@btk-consulting.fr
created: 2026-10-23
updated: 2026-10-23
parents: [OBJ-0003, ADR-0025, ADR-0016, ADR-0031]
depends_on: [SPEC-0008, SPEC-0009, SPEC-0013, SPEC-0015, DM-0002, DM-0004]
tags: [command-sync, commands, sync, discussion]
---

# RFD-0003: Command sync lifecycle

Origin: operator request to resume roadmap command sync after accepting reactivity's
review closure. This phase covers IMPLEMENTATION_PLAN Stages 5 and 6, not just
local scopes. The operator approved the recommended policies below; their
adoption does not mean the behavior has been implemented.

## Facts and decision boundary

SPEC-0008 explicitly leaves argument serialization, handler lifetime and
interaction with other writers open. ADR-0025 requires serialization across the
whole app; the Stage 0 Store's per-owner scope alone does not satisfy it.
SPEC-0009/DM-0004 leave dependency detection, persistence, transitive chains and
queue/reset behavior open. SPEC-0013 leaves separate receipt/outcome delivery
open. API-0001 permits arbitrary backend results, so a generic result cannot
tell core how to correct every row a handler created. SPEC-0015 leaves retention,
backoff defaults and sent cancellation details open.

These choices affect core, storage and both bridges. ARCH-3 requires this RFD
before an adopting ADR; CHG-4 requires operator approval of architecture and
contracts. Existing accepted ADR bodies stay unchanged. No runtime dependency
or execution against an existing user database is proposed.

### P1 — Platform-driven handlers, shared serialization and explicit abandonment

Use the Stage 0 platform-driven/core-passive mechanism with checked execution
tickets. A shared app coordinator admits handlers FIFO, one active handler
across all its database owners. The supported native process has one shared
coordinator; browser callers route through one authoritative worker coordinator.
Separate workers/tabs/processes accessing one store remain unsupported, as already
disclosed in ROADMAP. Per-owner gates must not be described as app-wide gates.

The host invokes its async handler only after admission and supplies a scoped
database handle. Core owns the staged ordered log and read-your-own-writes view;
no database transaction or Rust borrow is held across host awaits/callbacks.
Committed reads, observation registration and drains remain available. Direct
public writes, reconciliation, schema changes and reset cannot interleave with
an active scope. A scope operation rejects re-entry into command execution;
independent concurrent execute calls wait in the host dispatcher, not inside an
active handler. Raw adapter access is not exposed by the command owner.

Success finishes once and atomically commits the log; handler failure aborts.
Explicit abort, host future/task disposal and owner close discard staging and
release the slot. A late callback/finish on an abandoned ticket rejects without
mutating a new scope. No implicit timeout or watchdog is introduced: a never-
resolving handler holds the slot until explicit abort/close. Hosts must arrange
abort in their cancellation/finally path; core cannot detect a forgotten promise.
No claim is made about forcibly stopping an infinite host loop.

### P2 — Structured JSON, captured context and generated keys

Use an owned JSON value tree with bounded encoding/decoding in the dependency-
free core: Null, Boolean, String, exact signed 64-bit Integer, finite Float,
Array and ordered Object. Default limits are 1 MiB of encoded UTF-8 per value,
64 nested containers and 100000 total value nodes, all checked before retaining
an untrusted tree. Arguments must be an Object; result and opaque
identity may be any supported JSON value. Reject duplicate keys, invalid UTF-8,
nonfinite numbers, invalid numbers and exceeded limits before mutation. Bindings
must reject unsafe integer coercion rather than rounding i64 values through a
JavaScript Number. Canonical serialization fixes object ordering, number and
string encoding; bytes frozen at first send are the retry authority. Unicode,
integer extrema and integral floats need explicit round-trip tests.

Identity comes only from the configured trusted-context provider; handler
context is an immutable copy. Missing provider rejects execution, without an
authentication attempt. Capture time once; ctx.now() returns that value. Core
generates time-sortable keys from a persisted nondecreasing timestamp, sequence
and a persisted random 128-bit per-store namespace. The host supplies fresh
platform entropy when a new command store is initialized; an installation seed
shared by multiple stores is not a sufficient namespace. Different owners of
different stores use distinct namespaces even at equal clocks/sequences; reopen
reuses the stored namespace. Cloning a writable store into concurrent independent
clients is unsupported, like other multi-instance access. The host obtains
entropy using its platform facility; no dependency, networking or unaudited PRNG is added.
Check collisions/exhaustion and preserve identity across restart and clock
regression. Optional debug execution-log export is not implemented.

### P3 — Durable explicit dependency graph, rewrite before first send only

Command definitions declare exact argument reference paths, including array
indices where necessary. Do not guess that arbitrary strings are references.
Every declared unresolved local ID must have a known producing command; persist
its owner, dependency edges and final mapping atomically with local changes.
Support transitive and multiple-parent DAGs; reject unknown references or cycles
without committing. Only declared argument paths and schema Reference cells
participate in ID replacement; identical text in other strings is unchanged.

An entry stays queued until all dependencies have authoritative mappings.
Receipt alone never releases it. Apply parent ID replacement and dependent row/
argument changes in one transaction, rejecting ID collisions unchanged. Freeze
the outgoing envelope at first send; later retries use those exact bytes/key.
Mapping before a dependent's first send is not a retry and may alter its args.
Retain producer ownership/final mappings while any live dependent needs them.
After that, retain them only with the bounded recent producer history (P7).
Evicting history may evict an unneeded final map; a later command supplying that
old local ID rejects as UnknownLocalReference, rather than guessing a backend ID.
Applications should use the authoritative ID after resolution. A map still
needed by live work is recovery state and cannot be evicted by history pressure.
The first implementation supports command dependencies; entity-diff correlation
and mixed command/diff arrays stay entity sync work and are reported as such.

### P4 — Separate receipt, outcome and retry storage

Persist queued/sent transitions before dispatch. On receipt, atomically remove
the entry from the retry set but retain a durable awaiting-outcome record with
the data necessary for later ID mapping/reconciliation/rollback. Never resend an
acknowledged command. Receipt without an outcome is not a terminal success and
must survive restart. Outcomes correlate by key, not array index; independent
batch members resolve independently. Missing responses remain retryable.

Identical duplicate receipts/outcomes are no-ops; a conflicting second terminal
outcome is a typed protocol error without replacing an accepted outcome.
Unknown keys reject without modifying another command. Sent cancellation is
explicitly best-effort: persist a cancellation tombstone, stop retrying, roll
back locally if safe, and ignore late receipt/outcome for local mutation. It
cannot undo or prevent an authoritative backend action.

A final transport outcome implies receipt. Persist receipt and the original
authoritative result as awaiting local reconciliation before invoking the
application's plan builder. An interruption or invalid plan after that point
must recover that result without resending the command. Duplicate comparisons
are guaranteed while correlation/recovery/history/tombstone state is retained.
After history eviction or explicit forget, treat a late key as unknown and
reject without recreating work; do not promise indefinite duplicate comparison.

### P5 — Explicit reconciliation and guarded rollback

An application-supplied reconciliation function translates its arbitrary backend
result into a validated mutation/ID-map plan. This is client-side interpretation
of an outcome, never a replacement for backend handler execution. Stage plans
outside owner borrows; validate and commit application changes, dependency/map
updates and outcome state atomically. A malformed/colliding plan changes no
application rows, mappings or accepted terminal state, and retains the durably
received result for local reconciliation, without resending acknowledged
work. A correction uses ordinary data notifications; outcome events use the same
owned host drain, with no HTTP code/correlation field in CommandOutcome.

Persist first-before/final-after images needed for explicit rollback. Rollback
walks the dependency closure in reverse execution order. Before writing, compare
the complete affected current rows against the expected optimistic after-images
after accounting for that closure. If a later independent write changed an
affected row, return RollbackConflict atomically and preserve all data/undo state
for application reconciliation. Never restore an old image over unrelated work.
This conservative conflict is preferable to guessing business-specific replay.
Explicit rollback of an unconflicted rejection, or explicit cancellation,
fully reverts the parent-dependent closure; receipt of rejection alone never
invokes application rollback policy. Calling rollback again is idempotent.
Correction uses the same overlap guard.

### P6 — Queue-aware lifecycle without destructive reinterpretation

Add versioned, exact reserved logical bookkeeping through an additive storage
capability; keep SQLite's four private physical tables and format marker intact.
Recognize old queue-free managed stores without rewriting them at ordinary open;
initialize command bookkeeping atomically when opting in. Old readers may reject
new reserved metadata and must never reset it. Unknown future/corrupt metadata
reject unchanged. No persistence substitute bypasses SQLite or approval gates.

Reject authored schema migration and reset while any unacknowledged entry,
awaiting-outcome entry, unresolved mapping/dependency, pending rollback or sent
cancellation tombstone remains. Do not discard envelopes, reinterpret their
schema version or replay them against a different handler/schema. Preserve the
existing known queue-free chain-or-reset policy. Clearing resolved diagnostic
history is explicit and cannot clear live recovery records. Membership reset
with live sync state requires a later explicit policy; command sync must reject it.

### P7 — Bounded state and host-driven transport scheduling

Default capacity per store across all tables is 1000 live command recovery
records, configurable positive; distinct stores have independent capacity even
though their handlers share the app coordinator. Necessary mapping/undo/edge
records belong to their live producer/consumer records and do not create an
unbounded auxiliary retention path. In particular,
queued, sent, awaiting-outcome, unresolved rejected/rollback-conflict records and
cancellation tombstones all consume capacity. Admission checks before running a
handler and commit rechecks prevent exceeding it. A queued cancellation whose
work was never sent and is fully reverted releases capacity. Sent tombstones
retain capacity until an explicit integrator-authorized forget, because arbitrary
network silence cannot prove no late reply. No automatic rejection cancellation.
Bound fully resolved diagnostic history to the newest 100 entries by execution
order (configurable count, no implicit time-based expiry). History eviction never
discards live rollback/mapping/awaiting-outcome work. Long offline periods may
therefore block new commands until the application resolves/forgets work.

Use configurable exponential retry delays: initial 1000 ms, multiplier 2, cap
60000 ms, positive jitter in [0, 25%] of the capped base. Inject clocks/jitter
for tests; persist attempt/deadline information, clamp backward clock samples.
New commands and explicit retry wake scheduling but cannot bypass backoff.
Host wrappers schedule the next due drain/send; core owns no threads, timers or
network. A fake transport and an opt-in host HTTP reference exercise the same
array seam, including actual loopback request/response tests. Auth/TLS/request
timeout remain transport responsibilities and are documented at that seam.

## Alternatives and tradeoffs

Holding SQLite transactions over host awaits risks locks/re-entry and differs
from the proven staged mechanism. Per-table concurrency changes ADR-0025 and is
not proposed. String-prefix reference guessing corrupts ordinary business text;
explicit paths require declarations but are testable. Dropping all state at ack
loses later rollback/mapping; retaining a separate recovery record costs storage.
Automatic replay over later edits invents application policy; guarded conflicts
preserve data at the cost of explicit resolution. Arbitrary JSON requires a
bounded codec and cross-language number checks; a new dependency would require
separate approval and is not requested here.

## Acceptance, rollback and discussion

SPEC-0029/TC-0032 enumerate delivery slices and evidence. Operator approval is
recorded below; ADR-0032 adopts the policies. Exact slice contracts/cases still
require review before tests and policy-dependent implementation.
Synthetic owned fixtures only; no push/merge/release or actual user data operation.
Source rollback preserves stores; use a compatible reader for new bookkeeping,
never delete/downgrade metadata to make old code accept it.

- 2026-10-23: proposed following actual repository and accepted-contract review.
  No operator approval recorded. Independent preparation review identified
  cross-store key namespaces, receipt-before-reconciliation recovery, final-map
  retention, duplicate behavior after eviction, explicit rejection rollback and
  per-store capacity scope. The proposal and cases were clarified accordingly;
  independent re-review passed with no remaining blocking findings. This was a
  design review, not an implementation verdict or operator policy approval.
- 2026-10-23: operator answered "Approve recommended policies" to the complete
  reviewed policy bundle. Discussion/publication is complete; the RFD is
  committed and ADR-0032 adopts P1–P7. No runtime dependency was requested.
