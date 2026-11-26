---
id: SPEC-0028
title: Resolve reproduced query-size and observation failure defects
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-23
updated: 2026-10-23
parents: [ARC-0001, ARC-0002]
satisfies: [REQ-0007, REQ-0008, REQ-0009, REQ-0042]
depends_on: [API-0003, API-0004, API-0005, SPEC-0027, SPEC-0025]
tags: [reactivity, review-resolution]
---

# SPEC-0028: Resolve reproduced query-size and observation failure defects

Origin: operator requested three Claude Opus 5 review/resolution rounds after
an independent platform validation–3 review, explicitly prioritizing real issues over
invented findings. Preserve existing user CI tags. No new runtime dependency,
storage format, migration, service, sync behavior or performance SLO.

Confirmed faults: SQL In expands into OR and fails near 1000 IDs; repeated
explicit recovery failures are silent; successful projected counts share the
failure of unrelated full-row materialization. DEF-0006/0007/0008 and TC-0031
provide permanent reproduction and acceptance coverage.

Use native flat SQL IN for ordinary sets, separating Null semantics from typed
non-null comparisons and keeping every application name/value bound. Bound
parameter growth must not simply move the failure to SQLite's variable limit:
large operand sets use a private connection-local TEMP value table loaded with
bounded statements and read by IN subqueries. Require connection-local
PRAGMA temp_store=MEMORY, so temporary operands cannot consume OPFS pool files.
Use the existing kind/text_value/int_value encoding and affinities. There is no
fixed operand-count ceiling introduced by this lowering; work/memory grow with
operand count and remain subject to available resources. Table identifiers/set indices are
internal literals; values stay bound. Clear temporary operands on success and
failure, never change persistent inventory, and share this execution path for
fetch/project/query-plan. Preserve full SQL order/limit/AND semantics and strict
requested-cell/physical decoding. Adapters remain single-owner synchronous.

Candidate refresh failure still invalidates a whole group. Full-row delivery
failure keeps valid candidate keys and counts, marks only affected row listeners
for retry, and emits typed errors only to them. Explicit refresh retries dirty
candidate groups or row listeners; each failed explicit attempt emits an error
again so empty drain is not ambiguous. Successful row retry emits the current
exact snapshot even without a membership change. Count recovery retains the
sixteen millisecond deadline and no full rows; no write is relabeled failed
because post-commit reads fail. Registration/cancellation/lifetime semantics
remain API-0005. No notification carries partially refreshed rows.

Test boundaries include >=1100-row observations and navigation, >=33000 In
operands, multi-predicate typed/Null/Unicode/float extrema parity, more than 1000
reverse dependencies, repeated refresh failure, count/row isolation, cancellation
of dirty listeners and real SQLite malformed-cell read failure after successful
commit then restoration/recovery. Native fixture must deliver a typed error to
actual Swift and recover with retained timing/ownership semantics; browser uses
the same new shared regressions and verifies actual error callbacks. Synthetic
fault wrappers are distinguished from real SQL decoder errors.

Acceptance: TC-0031 RED before correction; all previous tests preserved, real
browser/Swift regression checks, relevant fmt/Clippy/WASM, trace0/currentRTM.
Run three independent Claude review rounds on fixed snapshots; reproduce and
resolve actionable supported-contract findings between rounds, accept no-finding
results, record exact scope/limits. Existing snapshot O(window) delivery, memory
clone and lifecycle full scans are documented costs, not a new optimization
project. Current ready-future adapter restriction/cancellation limits are stated
without pretending future async adapters have been validated.

Rollback: source revert preserving all application databases. TEMP sets are
connection-local and discarded at close; no new persisted metadata to migrate.
No push/merge/publish or production-data operation.
