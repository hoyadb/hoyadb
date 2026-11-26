---
id: RFD-0004
title: Phase 5 entity synchronization bootstrap and explicit reset policies
type: rfd
status: committed
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [OBJ-0003, ADR-0005, ADR-0007, ADR-0015, ADR-0017, ADR-0032]
depends_on: [SPEC-0010, SPEC-0011, SPEC-0012, SPEC-0013, DM-0003, RISK-0005]
tags: [phase-five, entity-diff, pull, bootstrap, reset]
---

# RFD-0004: Complete entity sync without inventing backend authority

Origin: the operator requested full roadmap Phase 5 after the independent Rust
formatting pass. This proposal resolves the accepted artifacts' explicit open
questions. The operator approved the recommended policies; implementation is still pending.
ARCH-3 and CHG-4 require reviewed architecture/contract decisions first.

## Facts and boundaries

SPEC-0010 and DM-0003 require partial patches, no entity idempotency key and the
same durable queue, but explicitly cannot correlate repeated record IDs.
SPEC-0011 leaves the override hook open and gives equal-time first-arrival
decisions exclusively to the server. SPEC-0012/RISK-0005 leave bootstrap and
reset with queued work undefined. API-0006 direct owner mutations are currently
local-only; API-0007 rejects schema lifecycle with live recovery. SyncPlan
already atomically compares model, recovery ledger and initial row guards.

The following policies extend those contracts explicitly. Accepted ADR bodies
remain unchanged. Identity, authorization, membership calculations, backend
business execution, authentication and network/timer ownership stay with the
integrator. No dependency, server product, independent concurrent store writers,
encryption, new binding platform or Phase 6/7 work is proposed.

## Recommended policies

### P1 — Opt-in automatic diffs and unambiguous correlation

An application explicitly configures entity-synced tables on its command owner
and supplies a trusted context/clock provider. Ordinary insert, replacement,
partial update and removal on those tables atomically commit their local effect
and generated diff in the existing ordered durable ledger. Add a partial-update
operation so only explicitly supplied fields travel. An unchanged value that was
explicitly written still belongs in the patch. Replacement writes all supplied
application fields and explicit Null/removal for previously present optional
fields that replacement omits; only partial update preserves omitted fields.
Create includes its supplied fields; delete has empty changes.
Invalid/missing context or capacity rejects before local mutation.

Command-scope writes, inbound application and inverse/reconciliation never
generate secondary diffs. Nonconfigured tables and standalone Database keep
their existing local behavior. Entity configuration is explicit on every open;
persist the enabled table set and scope binding once Phase 5 is used, and reject
incompatible reopen rather than silently sending with a different policy.

Add `writeId`, an immutable per-write **correlation** token, to diff envelopes
and require `table`, `recordId`, `writeId` in their responses. It is not an
idempotency key and creates no backend deduplication obligation. Keep
`idempotencyKey` absent. Tokens reuse the namespace/sequence allocation of the
single queue; multiple edits to one record, equal IDs in different tables,
reordered/partial/duplicate replies and mixed command/diff arrays are exact.
Reject ambiguous legacy recordId-only replies rather than guess. The explicit
contract extension replaces the disclosed recordId-only limitation.

### P2 — One queue, context, reference and recovery machinery

Use one discriminated command/diff recovery record, common ordering, capacity,
backoff, frozen retry bytes, receipt/outcome separation, status/observation and
guarded rollback. Status identifies kind/table/recordId without payloads; local
per-write tokens identify subscription/cancel/rollback targets for both kinds.
Terminal outcome content remains transport agnostic. No auto-cancel on rejection.

Entity-created local IDs include the persisted random store namespace and
monotonic local sequence, preventing two devices' first creates from sharing a
wire recordId. That recordId is stable across retries. Backend create must
preserve that identity, or maintain a stable original-record-to-final-ID mapping
if it remaps; natural create/update/delete idempotence still requires that record
identity, without requiring a writeId deduplication log. Use the existing
authoritative mapping machinery. Schema Reference cells produce durable dependencies; ordinary
String cells never do. Persist multiple/transitive parents, rewrite only before
first send, and release only after authoritative mapping. A diff update/delete
of an unresolved entity also depends on that entity's producer. Unknown or
retired producers reject. Inverse guards continue preserving independent edits.
Log optimistic field-clock/tombstone before/after images atomically with row
undo, and guard/restore them in the same inverse transaction. A rejected local
delete must not leave permanent comparison metadata after successful rollback.

### P3 — Field timestamps, authoritative ties and bounded override hooks

Persist per-field comparison timestamps and deletion tombstones with sync state,
distinguishing optimistic metadata from backend-confirmed metadata.
Default LWW merges each field independently, never a whole incoming record over
untouched fields. A delete wins over updates regardless of timestamp; updates
cannot resurrect a tombstone. Create-existing is a partial field merge; an update
to an unknown, nontombstoned ID must produce a complete schema-valid row or
reject the entire page. Reusing a tombstoned ID requires explicit reset/new ID.
Backend-confirmed tombstones remain until reset; expose their count, do not silently evict them
without a backend compaction protocol. This is a measured footprint limitation.

A reusable pure resolver accepts current values/field clocks/tombstone, the
incoming partial mutation and its clocks. An unresolved unequal-value equal-time
comparison returns AuthorityRequired. A backend-confirmed winner can be applied
on a tie; neither client ID, device ID, lexical value nor local arrival chooses
it. The reference fixture proves backend first-arrival, with no claim that the
library can enforce backend transactions or authorization.

Per-table deterministic synchronous override hooks receive those bounded current
and incoming candidates plus the default decision and return a validated merge
or error. They may customize unequal-time field resolution, but cannot invent
authorization, choose an unresolved equal-time winner, or resurrect an
authoritative tombstone. All outputs must pass the normal schema/value bounds.
No host callback runs while a storage transaction is open.

### P4 — Serialized atomic pull and conservative optimistic conflicts

Begin a pull with an owned request token carrying scope, owner generation,
durable data/comparison revision and the last committed opaque checkpoint
(null initially). Every committed data/comparison mutation advances that revision
atomically, including command reconciliation and unconfigured local writes.
The host awaits its
transport outside the admission permit. Allow one outstanding pull per owner;
abort/failure releases it. Applying a response obtains the same shared idle
permit as other committed writers and compares the original checkpoint/token.
Stale, closed, duplicate, cross-owner and pre-reset responses reject unchanged.
An intervening data/comparison revision makes the whole response StalePull,
even if its changes address unrelated rows; retry from the unchanged checkpoint.
This intentionally favors a small provable contract over parallel pull throughput.
Busy responses remain host-owned for retry after the active scope ends, only
while the original revision is still valid. StalePull retires the ticket: start
a fresh request from the unchanged checkpoint and refetch. Never rebind cached
response bytes to a new ticket.

Validate a complete bounded page before committing. Application data, field
clocks/tombstones, checkpoint and bootstrap progress commit in one SyncPlan.
Use current local state at application time, never a pre-await captured row
snapshot. If the page would change a row
protected by live optimistic/undo/mapping work, return PullConflict and preserve
the entire page/checkpoint. Also reject insertion into a table with live
unresolved local-ID producers: different IDs alone cannot prove independence
from an authoritative mapping whose outcome was lost. The caller resolves
outbound work and refetches from the unchanged checkpoint with a fresh ticket
and revision. No guessed rebase/replay of handlers.
Identical no-op changes need not conflict. All post-commit data notifications
use the existing reactive path; no notification for a failed transaction.

### P5 — Resumable bounded bootstrap, explicit progress and scope binding

Extend pull responses with `complete` and use the returned opaque checkpoint
as the continuation for the next page. Each page is independently atomic and
visible through ordinary subscriptions. Persist not-started/bootstrapping/ready
state, committed page and change counts, and expose owned progress observation.
`complete=false` continues initial sync; `complete=true` marks ready. Readiness
is sticky after first completion until reset; later incremental pagination does
not make already-bootstrapped data unready. Empty
pages are valid; a nonterminal page must advance its continuation to avoid a
tight replay loop. A fresh null checkpoint never implies deletion of local data.
Reopen resumes after the last committed page, never a half-applied page.

Keep core Json's 1 MiB/depth/node value limits; additionally cap each page at
1000 changes and 1 MiB canonical encoded response. Reject oversize input with
no checkpoint advancement. The backend owns stable pagination and membership;
core neither enumerates buckets nor requests data outside the returned scope.
The UI can distinguish partial query data from readiness through progress.
Local writes remain available between pages, under P4's conflict policy.
The experimental foreign RPC keeps its independent 1 MiB complete-frame bound.
To carry a valid core page without number coercion or escaping overflow, upload
bounded owned UTF-8 chunks under the original pull ticket, with checked offsets
and a total 1 MiB assembly limit, then validate/apply the complete page once.
Reset/abort/close invalidate and clear that ticket's buffer; no chunk commits
data or advances progress. Do not enlarge the existing frame limit or decode
domain numbers through JavaScript Number/Foundation as a workaround.

Bind Phase 5 recovery and checkpoints to a required application-supplied opaque
backend/tenant/environment scope string. A mismatched ordinary reopen rejects
without mutation. Scope is a partition binding, never an authentication proof.
Existing Phase 4 stores adopt the supplied scope atomically on explicit Phase 5
opt-in; reject adoption with live recovery whose original scope is unknown.
Membership changes within that scope never trigger automatic purge or reset.

### P6 — Total explicit reset, with preservation by default

Reset requires an idle shared coordinator; active or queued handlers return Busy
without aborting unrelated work. The application first cancels/aborts its active
handler when needed. Default PreserveRecovery reset rejects live outbound or
rollback/mapping/tombstone recovery, preserving Phase 4's no-loss policy.
An explicit DiscardRecovery mode permits the integrating application to discard
all local queued/sent/acknowledged/undo/rejected/cancelled work. Its API warning
states that this cannot cancel or reverse an already-performed backend action.

Successful reset atomically clears every application table, all queue/history,
field metadata/tombstones, checkpoint and bootstrap progress, while retaining
the declared schema and explicit options/table configuration. Advance a durable
generation and preserve monotonic namespace/sequence identity so keys/tokens
are never reused. Only after commit, invalidate data/status/outcome/progress
subscriptions and outstanding pull tokens; late host replies and pending cached
delivery frames cannot mutate or notify the new generation. Failed reset leaves
all of those intact. New pull starts with null, new queries observe empty data.
Reopen a different file leaves the original untouched. Authored migration still
rejects live recovery; changing schema with Phase 5 comparison/checkpoint state
requires an explicit reset first, avoiding reinterpretation of old field clocks.
Resolved authoritative deletion tombstones are comparison state, not live
outbound recovery, so they do not prevent PreserveRecovery reset. The retained
empty Phase 5 configuration shell also does not prevent authored migration.
After reset, explicit reconfiguration may change enabled tables with schema
validation; this is never inferred from a mismatched ordinary reopen.

## Acceptance, alternatives and rollback

SPEC-0031 / TC-0034 will cover all original TC-0011/0013 and mixed TC-0014
scenarios, cross-engine conformance, atomic faults, native process and browser
reload recovery, suspended handlers/transport, stale reset responses, callback
invalidation and the no-loss conflict paths. No assertion is satisfied by merely
writing this document or a substitute host queue.

Serializing one diff per record avoids an extra wire field but leaves late
duplicate replies ambiguous. Automatic rebase is more available but requires
inventing application replay policy. Atomic whole-bootstrap staging hides partial
queries but duplicates the dataset and delays offline access. Silent queue loss
on reset is simpler but changes recovery guarantees without caller intent.
These alternatives are not recommended.

Keep old queue-free and Phase 4 formats readable. Phase 5 metadata is explicitly
versioned and exact; unknown/future/corrupt state rejects unchanged. No new
physical SQLite table or runtime dependency. A source rollback uses a compatible
reader; never delete metadata to make an older reader open it. Verification
creates/deletes only task-owned synthetic files and OPFS namespaces, never user
stores. No push, merge, release or Phase 6 implementation.

## Discussion

- 2026-10-24: repository and independent read-only reviews confirmed the above
  decisions are genuinely open. Policy proposal prepared for review before
  asking the operator. No policy-dependent implementation existed at preparation.
- 2026-10-24: independent preparation review passed after correcting replacement
  omission, optimistic metadata inverse, unresolved-mapping pull collisions and
  fresh-ticket refetch after a data revision. Separate implementation/consumer
  investigation confirmed atomic revision/reset requirements and the need to
  respect complete foreign-frame bounds. Operator policy approval was pending at that review.

- 2026-10-24: operator answered "Approve recommended policies" to the reviewed
  P1–P6 bundle. ADR-0033 adopts those policies; implementation still requires
  reviewed exact contracts, behavioral RED/GREEN and real execution evidence.
