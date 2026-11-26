# Current architecture decisions

| Decision | Contract |
| --- | --- |
| [ADR-0001](docs/design/decisions/ADR-0001-rust-core-wasm-native-bindings.md) | Platform & language — one Rust core, compiled to WASM and exposed via native bindings |
| [ADR-0002](docs/design/decisions/ADR-0002-hoyadb-owns-its-schema.md) | Data model — hoyadb owns its own relational schema |
| [ADR-0003](docs/design/decisions/ADR-0003-pluggable-storage-interface.md) | Storage — one narrow pluggable interface, SQLite-family engine as the default |
| [ADR-0004](docs/design/decisions/ADR-0004-reactivity-set-membership-default.md) | Reactivity — one query object, two modes, set-membership change as the default trigger |
| [ADR-0005](docs/design/decisions/ADR-0005-command-first-sync.md) | Sync mechanism — named commands as the primary write path, entity-diff as fallback |
| [ADR-0006](docs/design/decisions/ADR-0006-authoritative-backend-membership-scoping.md) | Topology & scoping — one authoritative backend, server-computed membership |
| [ADR-0007](docs/design/decisions/ADR-0007-per-field-lww-conflict-default.md) | Conflict resolution — per-field last-write-wins, deletes win, overridable per table |
| [ADR-0008](docs/design/decisions/ADR-0008-implementation-roadmap-validation-spike.md) | Sequence the work as an implementation roadmap whose first step validates the Rust core shape |
| [ADR-0009](docs/design/decisions/ADR-0009-migration-chain-or-reset.md) | Schema migrations — apply an authored chain, otherwise reset the local database |
| [ADR-0010](docs/design/decisions/ADR-0010-client-generated-idempotency-key.md) | Every command carries a client-generated, time-sortable idempotency key |
| [ADR-0011](docs/design/decisions/ADR-0011-ack-receipt-and-outcome-enum.md) | Ack is durable receipt, separate from outcome, surfaced as one transport-agnostic enum |
| [ADR-0012](docs/design/decisions/ADR-0012-correction-via-subscription-and-rollback.md) | Corrections and rejections surface through the ordinary subscription path, with an explicit rollback |
| [ADR-0013](docs/design/decisions/ADR-0013-schema-version-and-versioned-command-names.md) | Every envelope declares the client schema version; command names carry a version suffix |
| [ADR-0014](docs/design/decisions/ADR-0014-command-determinism-is-a-non-goal.md) | Command determinism between client and server runs is an explicit non-goal |
| [ADR-0015](docs/design/decisions/ADR-0015-membership-revocation-developer-invoked-reset.md) | On membership revocation hoyadb exposes a reset operation, it does not purge on its own |
| [ADR-0016](docs/design/decisions/ADR-0016-durable-offline-queue.md) | Offline writes go through a durable, state-tracked, locally-persisted queue |
| [ADR-0017](docs/design/decisions/ADR-0017-server-side-tie-break.md) | Identical-timestamp conflicts are broken server-side — first received wins |
| [ADR-0018](docs/design/decisions/ADR-0018-multi-device-is-an-ordinary-conflict.md) | Two devices under one identity are an ordinary conflict case, not a special one |
| [ADR-0019](docs/design/decisions/ADR-0019-security-standards-scope.md) | Transport and auth standards are out of hoyadb's scope; at-rest encryption is engine-native |
| [ADR-0020](docs/design/decisions/ADR-0020-developer-supplied-encryption-key.md) | The developer supplies at-rest key material; hoyadb never generates, stores or manages it |
| [ADR-0021](docs/design/decisions/ADR-0021-local-only-logging-no-telemetry.md) | Command and trusted-context contents stay in local state; hoyadb sends no telemetry |
| [ADR-0022](docs/design/decisions/ADR-0022-performance-baseline-then-hold.md) | Performance — establish a baseline once there is an implementation, then hold or improve it |
| [ADR-0023](docs/design/decisions/ADR-0023-semver-and-reserved-namespace.md) | hoyadb's public API follows semver; its own bookkeeping uses a reserved namespace |
| [ADR-0024](docs/design/decisions/ADR-0024-durability-and-footprint-scope.md) | No durability guarantees beyond the queue's; hoyadb targets a small, mobile-appropriate dataset |
| [ADR-0025](docs/design/decisions/ADR-0025-async-serialized-handler-execution.md) | Command handlers are async and may read locally; hoyadb serializes them and commits each atomically |
| [ADR-0026](docs/design/decisions/ADR-0026-batching-is-the-default.md) | The reference transport always sends an array; each write resolves independently |
| [ADR-0027](docs/design/decisions/ADR-0027-minimal-query-model.md) | Query model — a deliberately minimal operator catalogue; indexing left undecided |
| [ADR-0028](docs/design/decisions/ADR-0028-per-command-observability.md) | Observability — per-command status, manual cancel, backoff with jitter, backend-owned key retention |
| [ADR-0029](docs/design/decisions/ADR-0029-documentation-artifact-root.md) | Keep formal artifacts under docs/design |
| [ADR-0030](docs/design/decisions/ADR-0030-engine-owned-storage-indexes.md) | Engines own physical indexes behind the portable storage query contract |
| [ADR-0031](docs/design/decisions/ADR-0031-schema-lifecycle.md) | Explicit relations and atomic managed schema lifecycle |
| [ADR-0032](docs/design/decisions/ADR-0032-command-sync-recovery.md) | Host-driven command scopes and durable guarded recovery |
| [ADR-0033](docs/design/decisions/ADR-0033-entity-sync-recovery.md) | Correlated entity diffs atomic paged pull and explicit recovery reset |
| [ADR-0034](docs/design/decisions/ADR-0034-protected-engines-and-diagnostics.md) | Supplied-key SQLCipher storage and bounded security diagnostics |
| [ADR-0035](docs/design/decisions/ADR-0035-general-sdk-and-restricted-mobile-builds.md) | General SDK bindings and explicitly approved restricted mobile builds |
| [ADR-0036](docs/design/decisions/ADR-0036-near-total-code-coverage.md) | Require code coverage close to 100 percent with justified gaps |
| [ADR-0037](docs/design/decisions/ADR-0037-per-database-command-serialization.md) | Serialize handlers per database while independent databases progress |
| [ADR-0038](docs/design/decisions/ADR-0038-stable-entity-metadata-identities.md) | Use stable metadata identities for new entity ledgers with explicit offline conversion |
