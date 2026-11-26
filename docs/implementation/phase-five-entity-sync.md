# Phase 5 entity sync, bootstrap and reset

Execution report for [SPEC-0031](../design/spec/SPEC-0031-entity-sync.md),
[API-0009](../design/contracts/API-0009-entity-pull-reset.md) and
[TC-0034](../design/tests/cases/TC-0034-entity-sync-conformance.md).
Phase 5 / IMPLEMENTATION_PLAN Stage 7 is complete for the existing experimental
Rust, Swift desktop and browser-worker targets. Implementation: `7ba538b`.
The operator-approved scope ends here; Phase 6 has not started.

## Scope and implementation

The operator approved RFD-0004; ADR-0033 adopts its policies. Preparation and
cases were committed before implementation (`8715d1b`, `7a85ec4`), followed by
reviewed host/correction/reconfiguration amendments. No runtime dependency was
added. The core and memory adapter remain dependency-free, and the existing
SQLite physical layout and C/WASM ABI are retained.

Configured ordinary mutations now produce partial entity diffs in the same
durable queue as commands. Context is captured once per operation/batch; missing
context, queue capacity and commit failure preserve data and identity counters.
The generated wire uses immutable table/recordId/writeId correlation, explicit
optional-field removals and byte-identical frozen retries without idempotencyKey.
Command scopes and inbound/inverse work produce no secondary diffs. Namespaced
entity identities, multi-parent dependencies and authoritative mappings rewrite
only declared references; resolved stable references survive history eviction.

Field stamps, optional removals and deletion tombstones are durable. The pure
resolver merges distinct fields, requires backend authority for differing equal
timestamps and validates per-table hook output. Guarded rollback restores both
rows and comparison metadata. Typed corrections install exact backend-selected
clocks despite newer optimistic clocks while retaining untouched fields and
independent work. Command-only format-1 packets remain unchanged; Phase 5 adds
exact format-2 extension packets in the existing logical sync table.

Pull uses original owner/checkpoint/generation/revision tickets and commits data,
comparison state, opaque continuation and progress atomically. Busy retains the
response/token; stale work requires a fresh fetch. Bootstrap exposes committed
partial pages and explicit readiness, resumes after interruption and cannot
advance a checkpoint past conflicting live recovery. Incoming pages may address
any declared table; outgoing table opt-in never grants membership entitlement.
Backend authorization, membership and server arrival ordering remain external.

Default reset refuses live recovery. Explicit discard clears local data/recovery
in one guarded transaction, retains schema/configuration/identity counters and
invalidates old tokens, subscriptions and host generations after success only.
An empty post-reset shell may be explicitly reconfigured; local-only rows or
retained history prevent rebinding. Active or queued handlers make reset Busy.

The existing Swift and browser reference hosts forward lossless owned frames to
one shared production Rust fixture. They own HTTP, waits, callbacks and pumps;
they do not implement a second queue or resolver. Pull uploads use bounded
sequential UTF-8 chunks without enlarging the existing 1 MiB complete frame.

## Acceptance mapping

| TC-0034 | Executable coverage |
| --- | --- |
| E01–E06 | `tests/conformance/entity_sync.rs`: automatic mutations, atomic admission/inverse, managed links, canonical mappings and multi-parent references; shared host fixture mixed arrays, receipt/retry and typed correction |
| E07–E10 | Pure resolver unit cases and `two_client_resolution`: two actual production owners, deterministic backend first-arrival fixture, opposite response orders, distinct fields, deletion and isolated hooks |
| E11–E15 | Shared `entity_pull.rs`, strict page codec, real SQLite deferred-COMMIT failure, original-token retry, protected recovery, bootstrap progress and stale request checks |
| E16–E19 | Shared reset/reconfigure inventory and transition guards; real transaction failure, independent processes, schema lifecycle guards; actual foreign callback/pump/reset/reload scenarios |
| E20 | All shared cases on memory/native SQLite/actual OPFS, actual Swift and browser HTTP/lifetimes, and the Phase 0–4 regression matrix |

The original TC-0011/TC-0013 cases are mapped by TC-0034. These tests establish
client behavior against a selected backend result, not production-server
membership enforcement or distributed transaction correctness.

## TDD and issue resolution

Compiling behavioral REDs preceded the resolver, automatic enqueue, comparison
inverse, status discriminator, same-value writes, managed-link identities,
canonical replacement result, pull/reset, typed correction and reconfiguration
fixes. Failures were observed at the required assertions, not accepted as
compiler failures. The [pull/storage subreport](phase-five-pull.md) records its
exact RED logs and targeted commands. Native and browser reports retain their
host-specific RED and generation-lifetime evidence.

The integration audit demonstrated a checkpoint-loss hazard: a newer optimistic
delete hid an authoritative update as a resolved no-op; checkpointing that page
then cancelling the delete restored an older value while skipping the update.
The shared regression passed on memory, SQLite and actual OPFS; the actual
Swift E-N03 regression also passed after its observed behavioral RED.

One bounded [Claude Opus 5 checkpoint](phase-five-claude-review.md) returned
PASS with no additional demonstrated defect. It reviewed before the known pull
fix; that fix was separately reviewed against the conservative contract and
verified by shared/native regressions. Hypotheses were kept separate from bugs.

## Verification record

| Gate | Observed result |
| --- | --- |
| Host workspace | 289 passed; 571.36 seconds for the complete workspace command |
| Host isolated Rust spikes | 11 cross-platform and 26 native tests passed |
| Actual Swift | 24/24 named runtime groups on one final static library; five consumers compiled with warnings as errors and strict Swift formatting |
| Actual optimized browser | 24 Phase 5 + 27 Phase 4 + 36 persistence initial + 12 actual reload + 17 reactive = 116/116 report groups; owned namespaces cleaned |
| Foreign deterministic tests | 14 Node + 5 Python HTTP tests passed |
| Formatting/lint/examples/builds | All five fmt/Clippy surfaces, layout regression, both examples and required WASM builds passed; browser release Clippy also passed |
| Fresh pinned Linux container | 326 Rust tests, all required fmt/Clippy/examples/WASM gates; exit 0 and PHASE5_CONTAINER_GREEN |
| Governance | Trace checker 0 violations; RTM current; final diff whitespace clean |
| Independent review | Opus checkpoint PASS plus independent post-fix pull/contract/evidence closure review PASS |

The [host output](phase-five-host-output.txt) and
[per-command results](phase-five-host-results.json) retain the exact commands,
counts and timings. [Container commands](phase-five-ci-commands.sh),
[output](phase-five-ci-output.txt) and
[image/source manifest](phase-five-ci-source-sha256.json) record fresh caches,
the read-only source snapshot and exact pinned Rust image. All 107 captured
build/test inputs matched the working sources after execution. This is local
container verification; remote GitLab CI was not observed. The container also
passed 289 workspace + 11 cross-platform + 26 native tests; it is a second run
of those tests, not 326 additional distinct acceptance cases.

The [Swift matrix](phase-five-native.md) and
[browser matrix](phase-five-browser.md) have separate pre-runtime artifact/source
manifests, hash closure and actual cleanup results. Report groups are not counted
as Rust tests. Browser setup registration timeouts were recovered as automation
setup failures; no failed assertion was hidden or payload/deadline reduced.
Phase 4's historical debug-profile browser timeout remains failed evidence;
Phase 5 runtime acceptance is for the ordinary optimized release artifact.

A validation-only [footprint sample](phase-five-pull.md) measured exact comparison
serialization at 10/100/1000 two-field rows. At 1000 rows it retained 191000 bytes
of metadata data-JSON; a checkpoint-only page still carried 1002 logical sync
packets. Its SQLite apply took 1459.550 ms in one contended debug run including
snapshot/validation/counting. This is not disk size, an isolated SQL benchmark,
a percentile or an SLO. Current adapters rewrite the complete logical ledger;
the retained-state cost is an explicit implementation limit.

## Change provenance and preservation

Reviewed specification/case preparation preceded runtime changes. The final
behavior commit is `7ba538b`; evidence records the exact tested working-source
hashes, not an assertion that preparation HEAD already contained implementation.
The pre-existing formatting pass remains separate. Behavior-only staging used
three-way candidates, formatter normalization and literal/comment token checks.
A clean standalone candidate tree passed five prior-format checks and workspace/
native all-target compilation without the pending formatting configuration.
[Provenance checks](phase-five-change-provenance.json) verify that every remaining
Rust difference is formatting, and the recorded pre-existing CI/config/spec
bytes match their baseline. User skills and runner-tag edits were not committed.

The full recorded formatter/layout commands test the frozen working tree,
including the pending SPEC-0030 configuration and script. Replaying that exact
container run from a clean checkout requires the separate preserved formatting
patch as well. The feature commit itself was independently checked with the
prior formatter and compiles without that maintenance patch. No claim is made
that the evidence-capture HEAD alone contains every tested working-tree file.

## Limits and rollback

This remains an unpublished experimental implementation for the existing Rust,
Swift desktop and dedicated-browser-worker surfaces. No Phase 6 encryption,
backup hardening, production SDK packaging, broader platforms, multi-writer
support, release, push or merge is included. SQLite is still plaintext at rest.
No power-loss guarantee is inferred from process termination or COMMIT faults.
Tombstones/comparison state remain until explicit reset; there is no invented
safe compaction policy or universal latency/footprint SLO.

Source rollback must retain compatible readers for new ledger packets. Never
strip unknown metadata or recovery to make an older reader open a store. Reset
verification uses synthetic owned stores; explicit discard cannot undo backend
effects. Pre-existing formatting, runner tags and user skills remain separate
from this task's behavior changes. Remote GitLab CI is not claimed unless its
actual result is observed.
