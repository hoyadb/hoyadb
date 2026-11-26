# Changelog

## Unreleased

### Implementation review corrections — 2026-11-10

- Keep the offline converter's store lease across the backup copy, create the
  backup no more permissive than the store and refuse a backup path naming a
  store sidecar (DEF-0064).
- Treat a pulled first tombstone on an unseen identity as insertion for unmapped
  local-ID producers, so it cannot occupy a pending mapping target (DEF-0065).
- Add the Kotlin backup-policy lock and opposite-parent check to protected open;
  compiled only, not yet executed on a device (DEF-0066).
- Keep a manual discard exit after a conflicted cancel of a processed rejection,
  renormalize conflict-cancelled dependents and limit the outside-closure refusal
  to live records (DEF-0067).
- Correct scope queries over self relations, ticket retirement before publication,
  same-migration backfill of an implicit to-one key and the early record bound in
  `begin` (DEF-0068); resolve durable aliases through the reference table and
  verify the retained durable mode (DEF-0069).
- Correct JavaScript application entity links, observer error routing and disposal,
  reject `Closed` from `execute` on a closed database, refuse undeliverable receipts
  and bound batch preflight in one pass (DEF-0070).
- Stop the Swift sync deadline spin, re-arm the count timer, buffer Kotlin drain
  events during registration and clear Swift entity context on close and reset
  (DEF-0071).
- Bind notice locks and Rust sources in Android packaging and refuse stale WASM
  or an unchecked test-control guard in the JavaScript and web builds (DEF-0072).
- Refuse first entity adoption of a table holding default `local:` rows
  (DEF-0073); let pulled inserts pass a fully rolled-back sent cancellation
  (DEF-0065).
- Add the additive paged reads `pending_reconciliations_page` and
  `queue_status_page` with SDK fallback loops (DEF-0070); align Swift raw
  reconcile, failed-receipt and Busy pull behavior with Kotlin (DEF-0071).
- Add the backup-policy lock to the React Native Android host (DEF-0066) and
  correct the Apple notice lock label; the rebound Apple provider digests await
  review (DEF-0072).

### Engineering follow-up — 2026-10-31

- Implement the approved Stop/Discard conflict policy in the core, facade and
  JavaScript, Swift and Kotlin SDKs, including resolving reports, private-open
  configuration and contained current-epoch callbacks (SPEC-0040).
- Enforce reviewed source/API/export compatibility, preserve the historical
  Swift initializer overload and correct the custom-ID guidance to match
  ADR-0035 (DEF-0056–0057).
- Forward validated queries through the facade and include omitted Cargo,
  build-script and source-resource inputs in Apple build evidence
  (DEF-0058, DEF-0060).
- Persist touched recovery packets through validated delta transactions; retain
  conservative full validation for unproven adapters. New entity ledgers use
  stable format-3 metadata identities, while existing format-2 stores retain
  their encoding through reset and reconfiguration (SPEC-0037 R9–R10).
- Restore exact memory index shape after rollback, preserve subscriptions when
  publication is cancelled, and exercise SQLite correction rollback at its
  actual delta write (DEF-0059, DEF-0061–0062).
- Compile ledger observation helpers only for tests or counters and deny
  warnings in the ordinary CI WASM build (DEF-0063).
- Add the explicit offline ledger converter `hoyadb-convert-ledger` behind the
  non-default `ledger-convert-tool` feature: a pure doc-hidden core re-encoder
  and a native SQLite tooling entry that validates read-only, hands off to a
  reserved writer without releasing its read lease, byte-verifies a durable
  backup before any write, and classifies a keyed WAL refusal with the
  operator-approved full-file integrity check. No facade, SDK or owner path
  converts stores (SPEC-0037 R11).
- Retain source-bound platform verification, behavioral regressions, independent
  cross-review and remaining converter/platform decisions in the
  [engineering follow-up](docs/implementation/approved-engineering-completion.md).
  Rust, Node/WASM, Chrome OPFS, macOS Swift and Android consumer checks pass
  within their recorded scopes; the offline converter remains gated.

### Review remediation — 2026-10-30

- Cancel and await explicit application synchronization, retain responses and
  mapper proposals through contention, clean up failed opens and tolerate
  reconciliation settlement between list/context reads (DEF-0044–0046, DEF-0054).
- Serialize React Native Android owner publication and invalidation (DEF-0047).
- Add Kotlin's declared redacted background-error sink and terminal observation
  delivery, preserving existing observer calls and containing throwing sinks
  (DEF-0040). Broader Kotlin feature and artifact gates remain explicit.
- Align Kotlin sync-contention tests and active guidance with accepted native
  per-database admission, retaining independent-database progress (DEF-0055).
- Supply real WASM artifacts to clean JavaScript CI; correct quality inventories,
  fail closed on unusable trace inputs and update active setup/recovery guides
  (DEF-0048–0053). Preserve historical evidence and publication holds.
- Record regressions, Claude Fable 5.1 cross-review and remaining work in the
  [remediation report](docs/implementation/review-remediation.md).

### Public repository preparation — 2026-10-30

- Group production Rust bridges under `crates`, npm packages under `packages`
  and executable tutorials under `examples`, preserving package names and APIs.
- Make the contribution contract and traceability checks self-contained. Keep
  private maintainer governance and agent skills local/ignored, with no required
  private submodule checkout. Preserve historical records and notice bytes.
- Add public setup, security-reporting status, release-check guidance and a
  [historical path map](docs/guides/repository-layout.md). Publication and license
  selection remain pending. See the [verification record](docs/implementation/public-repository-layout.md)
  for passing checks and unresolved baseline/platform gates (SPEC-0045/TC-0049).

### Test quality and browser recovery — 2026-10-29

- Retry a failed default browser runtime initialization while preserving shared
  concurrent initialization and explicit-worker isolation (DEF-0041).
- Recover new default opens after a browser worker retires, keeping existing
  handles closed with no replay (DEF-0043). Forward native encrypted fixture
  capabilities and add the regression for their parity (DEF-0042).
- Add pinned coverage/mutation commands, measured platform-specific floors,
  evidence validation and focused CI jobs under SPEC-0044/TC-0048. The reviewed
  decoder gate catches 41 viable mutants; one is unviable, with no survivors or
  timeouts. Broader/platform scopes remain explicit. See the
  [quality review](docs/implementation/test-quality-review.md) for actual scope,
  results and remaining gaps.

### JavaScript application binding — 2026-10-28

- Add plain-value schema builders, automatic browser/Node bootstrap, lazy queries,
  synchronous observer disposal handles, record updates and reusable async command
  definitions bound to explicit databases. The tagged `/host` API stays compatible.
- Add a plain fake backend, application-only automatic pull, cancellable background
  sync and persisted creation context for reconciliation after reopen. Protected
  browser storage still requires a caller-owned key; Node defaults to memory.
- Rework public HTML getting started around a complete executed JavaScript app,
  including local handlers, backend handlers, ID mapping and cleanup. Keep the Rust
  quickstart and advanced host reference separately. See API-0012 and TC-0046.

### Planned guarded-reconciliation compatibility — 2026-10-27

- Add lossless `CorrectionGuard` / `reconcileGuarded` SDK helpers. Managed
  reconciliation reports a core `RollbackConflict` once per key per pass and
  continues to unrelated pending entries, including when the conflict is in
  the command's own optimistic closure. The failed entry remains unresolved;
  its dependent work stays withheld. Callback failures and other core errors
  retain their existing behavior. Implementation verification is pending.
- Only the exact storage guard-conflict error becomes `RollbackConflict` during
  reconciliation. Other storage errors remain unchanged; invalid plan and guard
  structure are rejected before the optimistic closure proof. See
  [SPEC-0039](docs/design/spec/SPEC-0039-guarded-cross-row-corrections.md) and
  [API-0006](docs/design/contracts/API-0006-command-owner.md).

### Planned compatibility change — 2026-10-26

- Require explicit expected values or absence for correction writes outside a
  command's own rows. The operator-approved guarded API preserves atomic server
  effects while refusing stale expectations and protected pending intents; old
  unguarded cross-row writes will return conflicts. See
  [SPEC-0039](docs/design/spec/SPEC-0039-guarded-cross-row-corrections.md).
  Implementation and regression verification are pending; no store conversion
  or release is implied.

### Added — 2026-10-24

- Protect native and OPFS SQLite with supplied-key SQLCipher 4.19.0 and verified
  authenticated profiles; reject missing/wrong keys without plaintext fallback.
- Verify encrypted restart, recovery, sidecars, corruption and real Swift backup
  hints, and expose bounded diagnostics with redacted foreign error boundaries.
- Preserve complete data/retry APIs and explicit legacy controls; document key
  ownership, backup limits and upstream licenses in [Phase 6 evidence](docs/implementation/phase-six-security.md).

Operator-approved RFD-0005/ADR-0034 and exact reviewed runtime dependencies.
No user-store conversion, merge, publication or claimed mobile certification.

### Added — 2026-10-24

- Generate opt-in entity patches in the common durable queue with writeId
  correlation, explicit removals and guarded field comparison metadata.
- Add per-table merge hooks, backend-selected correction clocks, atomic paged
  pull/bootstrap, progress and explicit recovery-preserving or discard reset.
- Verify real Swift/OPFS host lifetimes and interruption, plus the full host and
  pinned-container regression matrices; see [Phase 5 evidence](docs/implementation/phase-five-entity-sync.md).

Operator-approved RFD-0004/ADR-0033; implementation `7ba538b`. No new runtime
dependency, merge, publication or Phase 6 implementation.

### Changed — 2026-10-23

- Define stable Rust formatting and editor defaults, expand compact layouts,
  and separate declarations and methods across workspace and spike sources.
- Add a formatter layout regression to CI; see
  [SPEC-0030](docs/design/spec/SPEC-0030-rust-formatting.md). No runtime behavior
  or public API changes; no merge or release.

### Added — 2026-10-23

- Add opt-in staged command scopes with shared FIFO admission and atomic local
  rows, durable envelopes, undo images and counters.
- Add frozen retries, dependency mapping, receipt/outcome recovery, guarded
  correction and rollback, cancellation, capacity and owned status events.
- Add experimental Swift/browser command hosts over the same SQLite owner,
  lossless tagged values, bounded reply pages and an opt-in loopback HTTP fixture.

Operator-approved RFD-0003 policies; core implementation `d465dc2` and preflight
hardening `d619820`, native host `7578c2d` and browser host `dc93546`.
See [Phase 4 evidence](docs/implementation/phase-four-command-sync.md) for
verification and profile-specific limits. No merge, release or publication.

### Fixed — 2026-10-23

- Remove the SQLite `In` expression-depth ceiling for large operand sets using
  flat lists and memory-only temporary sets.
- Report each failed explicit reactive refresh and isolate row delivery failures
  from valid count observations.
- Add shared, real SQLite decoder, browser and Swift regressions; retain the
  [independent review and verification evidence](docs/implementation/phase-three-review-resolution.md).

Operator-requested Phase 0–3 review resolution; local implementation `094d59a`
and expanded tests `2ffa1bd`. No release, merge or publication.

### Added — 2026-10-23

- Add shared incremental reactive queries, membership and named-column events,
  projected throttled counts, owned snapshots and explicit error recovery.
- Exercise real browser main-thread and Swift desktop delivery, including
  reentrant callbacks, cancellation, replacement and close.
- Record 158 workspace tests, browser reload checks, pinned-container validation
  and the runnable reactive example in [Phase 3 evidence](docs/implementation/phase-three-reactivity.md).

Operator-requested next phase; local implementation `58101a9`. No merge request,
release, publication or new external runtime dependency.

### Added — 2026-10-22

- Add the public Rust data model: owned schemas, generated IDs, lazy relations,
  managed many-to-many links and composable queries.
- Add atomic authored migrations and guarded missing-chain reset across memory
  and native/browser SQLite, with strict managed metadata and recovery.
- Record 122 host tests, real browser restart/rollback checks and the runnable
  example in [Phase 2 evidence](docs/implementation/phase-two-data-model.md).

Operator-requested Phase 2, approved RFD-0002 policies; local implementation
commit `45e7c5c`. No merge request, release or publication in this task.

### Changed — 2026-10-20

- Consolidate current design artifacts under `docs/design/`, supporting guides
  under `docs/guides/`, and prior-art research under `docs/references/`.
- Remove superseded legacy documentation and temporary consolidation steps;
  retain a source ledger and pinned historical provenance.
- Update documentation navigation and repository workflows for the new layout.

Requested and approved in the operator's documentation-consolidation session;
local commit and merge, with no merge request or publication.
