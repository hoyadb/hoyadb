# Decisions log — hoyadb

Append-only, dated log of every decision record (ADR) in this project, in
chronological order (oldest first). One line per decision. Entries are only
ever added — never edited or removed. A reversed decision gets its own new
entry pointing at the ADR that supersedes it (TRACE-5).

**This file is not `legacy/PLAN_DECISIONS.md`.** That file is the pre-existing
historical record of *why* each open question in the plain-Markdown
documentation pass was answered the way it was, and it is left untouched.
This file is the governance decision log required by TRACE-5, summarizing
the ADRs in `decisions/`.

All entries below are dated 2026-09-18, the date the `ai-governance`
formalization pass converted the already-reached decisions in `legacy/VISION.md`,
`legacy/refined-vision/` and `legacy/PLAN_DECISIONS.md` into ADRs. The dates on which the
underlying decisions were originally *reached* are recorded inside each
ADR and in `legacy/PLAN_DECISIONS.md`.

---

- 2026-09-18 — ADR-0001 accepted: Platform & language — one Rust core, compiled to WASM and exposed via native bindings (see decisions/ADR-0001-rust-core-wasm-native-bindings.md)
- 2026-09-18 — ADR-0002 accepted: Data model — hoyadb owns its own relational schema (see decisions/ADR-0002-hoyadb-owns-its-schema.md)
- 2026-09-18 — ADR-0003 accepted: Storage — one narrow pluggable interface, SQLite-family engine as the default (see decisions/ADR-0003-pluggable-storage-interface.md)
- 2026-09-18 — ADR-0004 accepted: Reactivity — one query object, two modes, set-membership change as the default trigger (see decisions/ADR-0004-reactivity-set-membership-default.md)
- 2026-09-18 — ADR-0005 accepted: Sync mechanism — named commands as the primary write path, entity-diff as fallback (see decisions/ADR-0005-command-first-sync.md)
- 2026-09-18 — ADR-0006 accepted: Topology & scoping — one authoritative backend, server-computed membership (see decisions/ADR-0006-authoritative-backend-membership-scoping.md)
- 2026-09-18 — ADR-0007 accepted: Conflict resolution — per-field last-write-wins, deletes win, overridable per table (see decisions/ADR-0007-per-field-lww-conflict-default.md)
- 2026-09-18 — ADR-0008 accepted: Sequence the work as an implementation roadmap whose first step validates the Rust core shape (see decisions/ADR-0008-implementation-roadmap-validation-spike.md)
- 2026-09-18 — ADR-0009 accepted: Schema migrations — apply an authored chain, otherwise reset the local database (see decisions/ADR-0009-migration-chain-or-reset.md)
- 2026-09-18 — ADR-0010 accepted: Every command carries a client-generated, time-sortable idempotency key (see decisions/ADR-0010-client-generated-idempotency-key.md)
- 2026-09-18 — ADR-0011 accepted: Ack is durable receipt, separate from outcome, surfaced as one transport-agnostic enum (see decisions/ADR-0011-ack-receipt-and-outcome-enum.md)
- 2026-09-18 — ADR-0012 accepted: Corrections and rejections surface through the ordinary subscription path, with an explicit rollback (see decisions/ADR-0012-correction-via-subscription-and-rollback.md)
- 2026-09-18 — ADR-0013 accepted: Every envelope declares the client schema version; command names carry a version suffix (see decisions/ADR-0013-schema-version-and-versioned-command-names.md)
- 2026-09-18 — ADR-0014 accepted: Command determinism between client and server runs is an explicit non-goal (see decisions/ADR-0014-command-determinism-is-a-non-goal.md)
- 2026-09-18 — ADR-0015 accepted: On membership revocation hoyadb exposes a reset operation, it does not purge on its own (see decisions/ADR-0015-membership-revocation-developer-invoked-reset.md)
- 2026-09-18 — ADR-0016 accepted: Offline writes go through a durable, state-tracked, locally-persisted queue (see decisions/ADR-0016-durable-offline-queue.md)
- 2026-09-18 — ADR-0017 accepted: Identical-timestamp conflicts are broken server-side — first received wins (see decisions/ADR-0017-server-side-tie-break.md)
- 2026-09-18 — ADR-0018 accepted: Two devices under one identity are an ordinary conflict case, not a special one (see decisions/ADR-0018-multi-device-is-an-ordinary-conflict.md)
- 2026-09-18 — ADR-0019 accepted: Transport and auth standards are out of hoyadb's scope; at-rest encryption is engine-native (see decisions/ADR-0019-security-standards-scope.md)
- 2026-09-18 — ADR-0020 accepted: The developer supplies at-rest key material; hoyadb never generates, stores or manages it (see decisions/ADR-0020-developer-supplied-encryption-key.md)
- 2026-09-18 — ADR-0021 accepted: Command and trusted-context contents stay in local state; hoyadb sends no telemetry (see decisions/ADR-0021-local-only-logging-no-telemetry.md)
- 2026-09-18 — ADR-0022 accepted: Performance — establish a baseline once there is an implementation, then hold or improve it (see decisions/ADR-0022-performance-baseline-then-hold.md)
- 2026-09-18 — ADR-0023 accepted: hoyadb's public API follows semver; its own bookkeeping uses a reserved namespace (see decisions/ADR-0023-semver-and-reserved-namespace.md)
- 2026-09-18 — ADR-0024 accepted: No durability guarantees beyond the queue's; hoyadb targets a small, mobile-appropriate dataset (see decisions/ADR-0024-durability-and-footprint-scope.md)
- 2026-09-18 — ADR-0025 accepted: Command handlers are async and may read locally; hoyadb serializes them and commits each atomically (see decisions/ADR-0025-async-serialized-handler-execution.md)
- 2026-09-18 — ADR-0026 accepted: The reference transport always sends an array; each write resolves independently (see decisions/ADR-0026-batching-is-the-default.md)
- 2026-09-18 — ADR-0027 accepted: Query model — a deliberately minimal operator catalogue; indexing left undecided (see decisions/ADR-0027-minimal-query-model.md)
- 2026-09-18 — ADR-0028 accepted: Observability — per-command status, manual cancel, backoff with jitter, backend-owned key retention (see decisions/ADR-0028-per-command-observability.md)

- 2026-10-20 — ADR-0029 accepted: Group formal artifacts under docs/design; retain guides, research, and provenance separately. Registry and RTM paths are relative to the formal artifact root; DECISIONS.md remains at repository root. See docs/design/decisions/ADR-0029-documentation-artifact-root.md.

- 2026-10-22 — [RFD-0001](docs/design/rfd/RFD-0001-storage-query-index-ownership.md) created at prediscussion: recommend engine-owned initial query indexes; operator choice pending, no implementation decision adopted.

- 2026-10-22 — [RFD-0001](docs/design/rfd/RFD-0001-storage-query-index-ownership.md) committed after operator approval of engine-owned indexes and continuation to finish Phase 1; concrete follow-on contracts and cases are SPEC-0025/API-0003/TC-0025.
- 2026-10-22 — [ADR-0030](docs/design/decisions/ADR-0030-engine-owned-storage-indexes.md) accepted: engines own physical indexes and execute portable typed queries; preserve compatible data, measure per-engine costs, and retain later schema/reactivity/security gates.

- 2026-10-22 — ADR-0031, following explicit operator approval of RFD-0002:
  Phase 2 uses explicit queryable through tables, library-managed lazy relations,
  separate application schema metadata and atomic authored upgrades or supported
  forward missing-chain reset. Downgrade, drift, unknown/future bookkeeping and
  queue-present stores reject unchanged; compatible populated Phase 1 adoption
  is limited to validated version 1. See SPEC-0026/API-0004/TC-0026.

- 2026-10-23 — [RFD-0003](docs/design/rfd/RFD-0003-command-sync-lifecycle.md)
  opened for discussion after the operator resumed Phase 4: proposes shared
  command scopes, durable dependency/receipt recovery, guarded reconciliation,
  queue-aware lifecycle and bounded scheduling. Operator approval is pending;
  SPEC-0029/TC-0032 are preparation, not implemented or accepted policies.

- 2026-10-23 — Operator approved RFD-0003's recommended policies; RFD-0003 is
  committed and [ADR-0032](docs/design/decisions/ADR-0032-command-sync-recovery.md)
  accepted. Exact per-slice API/storage review and TDD remain implementation gates.

- 2026-10-24 — ADR-0033 adopts the operator-approved RFD-0004 Phase 5
  entity correlation, field comparison, paged pull/bootstrap and explicit reset
  policies. No dependency or later roadmap phase is approved.

- 2026-10-24 — Operator explicitly approved RFD-0005's exact dependency graph;
  [ADR-0034](docs/design/decisions/ADR-0034-protected-engines-and-diagnostics.md)
  adopts supplied-key SQLCipher protection with verified profiles, native backup
  hints and bounded redacted diagnostics. No automatic conversion, rekey, user
  store operation or publication. Phase 7 was subsequently requested separately.

- 2026-10-24 — Phase 6 evidence and independent crypto/lifecycle reviews close
  SPEC-0032, API-0010 and REQ-0045 as accepted, TC-0035 as active and THR-0019
  as mitigated within the existing native/Swift/OPFS scope. See
  [execution evidence](docs/implementation/phase-six-security.md) and its
  explicit backup, key-copy, process and unsupported-platform limits.

- 2026-10-24 — [RFD-0006](docs/design/rfd/RFD-0006-general-bindings-and-baseline.md)
  records the operator-requested Phase 7 recommendations after Phase 6 closure
  `c01d2ec`: general one-core bindings, protected platform packages, deterministic
  helpers and measured budgets. SPEC-0033/API-0011/TC-0036/0037 prepare acceptance;
  exact new runtime graphs and the explicit TC-0016 ID example remain pending.

- 2026-10-24 — The operator explicitly approved the exact Phase 7 dependency
  bundle and restricted build policy, including disclosed build-only advisory
  risk, and the exact prefixed command-ID clarification. ADR-0035 adopts those
  choices; the dependency approval record preserves review-byte hashes. Actual
  mobile execution, complete performance evidence and final review remain gates.

- 2026-10-25 — [ADR-0036](docs/design/decisions/ADR-0036-near-total-code-coverage.md)
  accepts the operator's requirement for code coverage close to 100%, targeting
  100% line and branch coverage with narrowly justified reviewed gaps and
  component-level reporting. Coverage remains unmeasured; the operator requested
  this decision only, with no implementation yet.

- 2026-10-25 — The operator explicitly approved the exact reviewed compiler
  build bundle and requested security fixes and Phase7 completion, followed by
  two clean final Claude Opus5.5 reviews, delegated ADR-0036 coverage work and
  a proposal for the next phase based on verified gaps. The
  [approval and ordered gates](docs/implementation/phase-seven-kotlin-compiler-approval.json)
  preserve the frozen proposal hashes, restricted build policy and actual
  completion state. This subsequent instruction authorizes future coverage
  implementation after the preceding gates; it does not claim coverage exists.

- 2026-10-26 — The operator approved per-database handler serialization and
  stable metadata keys for new entity ledgers with an explicit offline reversible
  converter in RFD-0007. [ADR-0037](docs/design/decisions/ADR-0037-per-database-command-serialization.md)
  replaces ADR-0025's app-wide boundary; [ADR-0038](docs/design/decisions/ADR-0038-stable-entity-metadata-identities.md)
  preserves existing entity formats on open/reset/rebind and authorizes no user
  conversion. Detailed independent review, test-first implementation and three
  issue-free final Opus reviews remain required; zero are counted.

- 2026-10-26 — The operator selected "Add guarded correction API (Recommended)"
  for server effects outside a command's own rows. The reviewed
  [SPEC-0039](docs/design/spec/SPEC-0039-guarded-cross-row-corrections.md) requires
  explicit expected values or absence, atomic comparison, stale-expectation
  rejection and protection of other live command intents and entity metadata.
  This authorizes test-first implementation of the additive contract; no user
  store operation, conversion or convergence claim is authorized.

- 2026-10-27 — The operator approved the reviewed Stop/Discard design in
  [SPEC-0040](docs/design/spec/SPEC-0040-simple-command-conflict-policy.md),
  noting that the library is unreleased and may be modified. This accepts the
  specified API and stored-state changes: current rows stay intact, new builds
  report Discarded truthfully, and older builds refuse to open a store while a
  discarded record remains, with no guaranteed history-trimming deadline.
  [TC-0044](docs/design/tests/cases/TC-0044-simple-command-conflict-policy.md)
  still requires test-first implementation and verification; no user-store
  operation or completed implementation is claimed.

- 2026-10-30 — Operator approved [REQ-0048](docs/design/requirements/REQ-0048-public-contributor-checkout.md) / [SPEC-0045](docs/design/spec/SPEC-0045-public-repository-layout.md): organize production Rust bridges under crates, npm packages under packages and tutorials under examples; make contributor instructions and artifact checks independent of private maintainer governance. Preserve public APIs, historical records and publication holds.

- 2026-10-31 — Operator answered "Allow the full-file check" for the offline
  converter's wrong-key/WAL classification in
  [SPEC-0037 R11](docs/design/spec/SPEC-0037-storage-isolation-and-scaling.md).
  This accepts the existing SQLCipher integrity check's full-main-file reads
  and proportional failure-row memory, including work performed during statement
  preparation. It runs only after a protected read-only WAL admission refusal,
  on the same connection with its read lease retained. Authentication failures
  remain redacted exit 5; a clean main file still gives unsupported-source exit 2
  and never authorizes WAL conversion. No source write, journal recovery,
  automatic conversion, user-store operation, new dependency or publication is
  authorized by this decision.


- 2026-11-07 — The operator requested coordinating the local HoyaDB migration of
  Monstera and Backoffice while preserving existing behavior and API compatibility.
  [SPEC-0048](docs/design/spec/SPEC-0048-entity-batch-aliases.md) records the necessary
  additive atomic prepared-batch capability: temporary explicit references,
  core-allocated entity IDs and partial patch intent with local completion markers.
  Existing entityBatch and native ABI symbols remain unchanged; this decision
  introduces no dependency, publication or arbitrary entity ID allocation.

- 2026-11-07 — Within the authorized local Monstera/Backoffice migration, the
  coordinator selected [SPEC-0049](docs/design/spec/SPEC-0049-durable-entity-aliases.md):
  a separate fresh-entity-store opt-in to durable core identity aliases and a
  table-aware resolver. A private persisted format-4 wrapper preserves all public
  ledger struct shapes and existing format-1/2/3 defaults/readers. Seeded ordinary
  legacy rows are allowed; existing configured stores cannot convert implicitly.
  Format 4 is intentionally unreadable by older binaries and unsupported by the
  existing converter. Reset clears aliases before account rebind. No dependency,
  old user-store conversion, publication or platform validation is implied.

- 2026-11-10 — The operator requested a review of the implementation, then asked
  for all findings and the remaining deferred items to be fixed where possible.
  The corrections are recorded as DEF-0064 through DEF-0073. Contract changes
  chosen within that request: a Cancelled record that still stores its Rejected
  outcome is discardable (SPEC-0040, API-0006); a first tombstone on an unseen
  identity conflicts for unmapped producers while a fully rolled-back sent
  cancellation no longer blocks pulled insertion (API-0009); first entity
  adoption refuses tables holding default `local:` rows and ordinary
  configuration of a durable store rejects (API-0009, SPEC-0049); `begin`
  applies the record bound earlier (API-0006); JavaScript `execute` on a closed
  database rejects `Closed`; additive paged read operations
  `pending_reconciliations_page` and `queue_status_page` join ABI 1 with existing
  operations unchanged (API-0011). Pulling into non-configured schema tables,
  the empty-store definition and exact-shape link payload equality were reviewed
  and kept. No dependency, publication, persisted-format change or platform
  validation is implied; Android device execution and review of the rebound
  Apple provider digests remain open.

- 2026-11-26 — The operator requested GitHub checks, builds, publishing and Pages,
  chose MIT for first-party code and identified the hoyadb GitHub organization.
  SPEC-0050 / TC-0054 prepare delivery for hoyadb/hoyadb while preserving the
  existing GitLab gates and native dependency/provenance requirements. Public
  PR checks use hosted runners; reviewed native builds and documentation use a
  restricted provisioned runner. Maven namespace ownership, registry setup and
  actual platform acceptance remain external gates. The operator also requested
  a complete local/GitHub history reset preserving source files, leaving GitLab
  untouched, and conditioned committing/pushing on readiness. Preserve a separate
  Git recovery bundle before that reset; do not infer publication authorization.
