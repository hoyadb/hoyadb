# Durable entity ID resolution

The Monstera migration retains IDs in navigation and pending image uploads.
The previous resolver consulted completed command history, so forgetting a
producer or reaching the history limit erased the mapping while an external
reference could still need it. [SPEC-0049](../design/spec/SPEC-0049-durable-entity-aliases.md)
and [TC-0053](../design/tests/cases/TC-0053-durable-entity-aliases.md) define the
separate opt-in capability.

`configureEntitiesDurable(scope, tables)` adopts persisted format 4 only when no
entity ledger has previously been configured. Ordinary seeded legacy rows and
nonentity migration markers are allowed. Existing format-2/3 entity stores reject
that operation unchanged. `resolveEntityId(table, id)` asks the core authority;
it does not query for row existence or imply that an unresolved create has synced.
Consumers resolve IDs before queries, navigation lookups or external API calls.
Query grammar and ordinary query equality have not acquired automatic rewriting.

The [private codec](../../crates/hoyadb-core/src/command/durable.rs) separates
aliases from the unchanged public `EntityLedger` and `SyncLedger` fields. It
validates the actual format-4 inventory, projects only its base header privately
for existing format-3 validation, and restores format 4 when encoding. The public
legacy decoder still rejects format 4. Ordinary configuration remains format 3;
format 1/2/3 readers and serializers retain their existing behavior. The current
offline converter rejects format 4 without writing or creating a backup.

Only successful entity reconciliation records aliases, in the same transaction
as row/reference rewrites, comparison metadata and recovery changes. The
[first-image journal](../../crates/hoyadb-core/src/command/ledger_txn.rs) restores
alias values and indexes on failure. [Full and delta validation](../../crates/hoyadb-core/src/storage/sync.rs)
check the actual persisted mode, immutable aliases, namespace and model allocation
bounds. Bundled memory/SQLite loaders validate the complete private inventory.
Known aliases are table-aware; an old ID cannot address a different configured
table. Ordinary writes and declared command-reference paths consult the same
core authority, without an application mapping ledger. A Reference cell whose
declared to-one relation names its target table resolves through that table
only: an accepted canonical ID there is left unchanged even when another table
has an alias source with the same literal, and an alias source of a different
configured table is rejected. Cells without a declared to-one relation and
command reference paths carry no table and keep table-independent resolution.

Either configuration entry verifies the retained mode. `configure_entities` on
an opted-in format-4 store returns InvalidCommand, exactly like
`configure_entities_durable` on a format-2/3 store; neither converts, installs
a provider or changes stored bytes. An opted-in application must keep calling
the durable entry after every reopen.

First adoption through either entry also refuses, with Protocol and before any
mutation, a configured table that already holds a reserved `local:` row
([API-0009](../design/contracts/API-0009-entity-pull-reset.md)). Seeded legacy
rows with other IDs and nonentity markers remain allowed.

Reset clears aliases with rows and recovery while retaining format 4. An empty
post-reset store can bind a new scope through the existing reconfiguration API.
Account changes cannot retain aliases from a prior scope. Schema changes reject
while aliases remain. Alias retention is independent of the bounded recovery
history; storage grows with accepted identity mappings until explicit reset.
An older binary cannot open an opted-in store. This feature provides no format-4
downgrade, implicit conversion or mobile activation claim.

## Test-first evidence, 2026-11-07

The first public-owner regressions used the previous configuration path and
failed after explicit forget/reopen and automatic zero-history eviction/reopen:
resolution returned the original generated ID instead of the server ID. The
unchanged default-format control passed. After implementing the explicit durable
path, both regressions passed.

Codec tests first failed against stub operations, then passed. A later regression
exposed a cross-table retained producer mismatch and passed after the validator
was corrected. The runtime wrong-table mutation regression likewise failed
before the write paths adopted table-aware normalization, then passed. Final
review added a full-plan regression that initially accepted a forged new alias
without a prior accepted producer. The transition validator now distinguishes
new aliases from history-independent existing aliases and checks that producer.
A further table-boundary regression showed the resolver following another table's
alias when its source literal equalled this table's accepted canonical ID. The
resolver now retains the canonical table identity; both focused table cases and
all 13 codec tests passed after that correction.

## Observed focused checks

- Private codec unit tests: 13 passed, covering canonical round trips, legacy
  controls, malformed aliases, namespace/table violations, collisions, cycles,
  retained producer consistency and indexed resolution.
- [Memory runtime cases](../../crates/hoyadb-storage-memory/tests/durable_entity_alias_cases.rs):
  10 passed on the final runtime suite, including seeded legacy ULID plus migration marker, reset/rebind,
  table-aware writes and canonical/source collisions, history/forget restart,
  default format-3 byte preservation,
  refused opt-in on an existing store, legacy-reader rejection, and injected
  commit failure followed by successful retry on full and delta paths.
- [Memory format cases](../../crates/hoyadb-storage-memory/tests/durable_format_cases.rs):
  8 passed, covering marker-only downgrade, alias removal/retargeting, alias-only
  account scope changes, allocation bounds, frozen envelope mutation, invalid
  row/stale guard atomic rejection, unchanged format-2/3 reopen/refused opt-in,
  and rejection of new aliases without an already accepted producer.
- [Native SQLite cases](../../crates/hoyadb-storage-sqlite/tests/durable_entity_alias_cases.rs):
  3 passed with `ledger-convert-tool`, covering actual close/reopen after history
  cleanup, both commit paths' injected failure/retry, and converter rejection
  for targets 2/3/4 with identical source file bytes and no backup.
- The fixed-size commit counter test passed with 10 and 1,000 retained aliases:
  packet encode/parse counts, row clone counts and metadata visits were identical;
  no application inventory validation occurred during the measured commit.

The shared runtime fixture is [durable_aliases.rs](../../tests/conformance/durable_aliases.rs).
Bindings and JavaScript forwarding evidence is recorded in the
[prepared batch report](entity-batch-aliases.md). Final workspace executable
suites passed 933 tests; an initial stale SQLite crate-metadata failure in the
doctest stage was resolved by a sequential rebuild and all six workspace doc
proofs passed. The final binding package rerun passed 59 tests and its doc
harness. The final SQLite durable suite passed all three cases with
`ledger-convert-tool`. Workspace all-target Clippy with warnings denied, global
formatting, source compatibility, fresh Swift source comparison, wasm32 core and
memory library build, sync-proof, traceability and RTM checks passed. The
prepared batch report records exact logs and the initial command's nonzero exit.

Fresh artifact inventory/rebuild after the final binding lint edit and actual
mobile application behavior remain with the coordinated artifact/application
lanes. No commit, publication, old user-store conversion or production data
operation was performed.
