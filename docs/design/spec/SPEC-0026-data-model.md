---
id: SPEC-0026
title: Complete data model public Rust data model and atomic schema lifecycle
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [ARC-0001, RFD-0002]
satisfies: [REQ-0001, REQ-0002, REQ-0003, REQ-0004, REQ-0005, REQ-0006, REQ-0033, REQ-0034, REQ-0041, NFR-0005]
depends_on: [SPEC-0003, SPEC-0004, SPEC-0005, SPEC-0025, API-0003, API-0004]
tags: [data-model, schema, migration, data-model, rust]
---

# SPEC-0026: Public data model and schema lifecycle

## Need, authority and scope

The operator requested starting and finishing roadmap data model, then approved
RFD-0002's recommended relation and lifecycle policies. storage provides
supplied-schema storage operations, not public schema ownership or migrations.
This slice implements SPEC-0003/0004/0005 through the exact Rust API-0004 and
TC-0026, including the original TC-0001/0002/0003 acceptance behaviors.

Implement the public API inside the existing dependency-free Rust core, using
memory/native SQLite and the supported single-worker browser SQLite adapter.
Keep API-0002/0003 storage contracts and their existing tests. All execution
uses isolated synthetic fixtures; no existing user database is opened, adopted,
reset or migrated by this work. No runtime dependency is introduced.

Live subscriptions, incremental reactive maintenance, sync/commands/queues,
production platform SDKs, publication, encryption, backup exclusion, shared
concurrent clients and crash/power-loss guarantees remain in their roadmap
phases. A lazy query's one-shot fetch is implemented here; its future observe
mode remains reactivity. No OR, aggregates, pagination beyond limit, recursive or
to-many predicate traversal, implicit join tables, custom types/validators or
eager/batched relation loading is added.

## Public schema, records and queries

API-0004's SchemaDefinition has a positive u32 application version, named typed
tables and full relation declarations. Reuse the six API-0003 primitives and
its finite-float, timestamp, reference, optional/null and comparison rules.
Compile the definition into existing storage::Schema; do not change the meaning
of its HasMany/ManyToMany rejection markers or make applications use raw storage.

ToOne declares its target, foreign key and optionality. Its missing foreign-key
column is materialized as a Reference column; an explicitly declared compatible
Reference column with exactly matching optionality can be reused. Validate all
to-one/has-many targets and keys,
through-table relations and ambiguous or inconsistent mappings before storage
mutation. Many-to-many uses explicit application-queryable through tables with
named to-one relations identifying source and target references. Named inverse
relations support both directions without guessing inverse relation names.

Reserve the established __hoya_ prefix for managed bookkeeping. Reject reserved
application table, column, relation and input-field names. Add implicit id and
__hoya_schema_version bookkeeping to managed rows; applications never supply
or overwrite the latter. Database owns the schema and storage; public writes
always use validated API-0003 mutations with the private metadata schema added
internally. Application queries cannot expose or alter reserved metadata tables.

Provide generated-ID insertion, typed mutation and query composition. Default
IDs use local:<persisted counter>, skipping target-table collisions in stored
rows or earlier batch inserts and rejecting counter overflow; an injected FnMut
generator supplies its exact ID, with nonempty/duplicate validation and no retry. A failed write cannot consume committed
counter state or publish changes. Data rows, per-row schema bookkeeping and ID
counter metadata commit atomically. Public rows and change snapshots omit the
managed version field; persisted rows retain it. Custom generator side effects
are not rolled back. User-supplied fields remain typed and owned.

Normal query filtering, direct to-one lookup, ordering and limit execute in the
adapter. The public builder reuses API-0003 semantics, including validation on
empty tables/limit zero, deterministic id ties and strict errors. Navigating a
relation constructs a lazy query; loading source references, memberships or
target rows waits until fetch. First-class link/unlink manages through rows;
applications never construct a join query or maintain pivot rows to use a
relation. Returned target rows are distinct even if stored membership repeats.
Link/unlink require present source and target records. Extra through payload
rejects caller-supplied FK or managed keys. An existing link returns its first
existing row only for identical extra payload; a different payload rejects.
Unlink removes every duplicate membership atomically and returns its count,
including zero for no link. Target predicates, ordering and limit apply to the
resolved target set, with limit after deduplication. This does not enable to-many dot-path predicates.

## Managed schema state and opening

Keep application schema state separate from SQLite's private format marker 1.
Store the complete canonical application definition and the committed ID counter
as strict known-field metadata in reserved logical rows. API-0004 fixes the
standard-library length-framed codec, field names and exact version/counter
representation. Decode lengths, UTF-8, tags, duplicate names, trailing data and
metadata shape strictly; never guess a default after decoding failure.

Opening an empty unversioned logical store can initialize any positive declared
version. A nonempty unversioned storage store can be adopted only at version 1,
only when every existing logical table and complete row matches the compiled
application schema. Preserve its application data and IDs; add managed metadata
and bookkeeping in one transaction. Other ambiguous legacy stores reject
unchanged. Private physical tables do not make an otherwise empty logical
store count as populated application data.

An equal stored application version requires the identical canonical definition;
no structural change is inferred. Reject downgrade, future/unsupported metadata,
malformed metadata, unknown reserved bookkeeping and any queue-present state
without mutation. This phase neither interprets nor discards future queue data.
Known supported queue-free forward opens follow the authored-chain-or-reset
policy below. Native and browser persistence use the same metadata codec.

## Authored migrations and atomic reset

A migration declares its from_version, complete to_schema endpoint and ordered
steps. The explicit catalogue is AddTable, DropTable, AddColumn, DropColumn,
RenameColumn, Backfill and SetRelations. API-0004 fixes operands/defaults and
validation. No transformation is inferred from the endpoint difference.

Core resolves the complete ascending chain before applying any step. Validate
migration declarations and each authored endpoint; all migrated rows and table
inventory must match the final target. An absent complete chain on a supported
known queue-free store triggers the accepted reset policy. Reset clears and
recreates owned logical application tables and writes target metadata in one
transaction, retaining the next-ID counter, SQLite file, private v1 DDL and
format marker.

A step, endpoint-validation, storage or commit failure rolls back the entire
chain, including earlier successful steps, metadata, counter and implicit row
versions. Return failure; do not turn a failed migration into reset. Invalid
plans or unsupported states likewise reject rather than selecting reset.
Reset is not a resync operation and does not contact a backend.

## Engine boundary

Add API-0004's schema lifecycle capability without weakening API-0002/0003:
catalog reports state plus logical inventory; scan(table) supplies owned rows
only for migration/adoption work; apply_schema_plan compares expected state
and atomically applies ordered Create/Drop/Write operations plus next state.
Final inventory and all rows are validated against the target storage schema
before commit. A stale expected state rejects before mutation. Core owns policy
and plan construction; engines own persistence, query execution and atomicity.

Memory commits a validated candidate state. SQLite uses one real transaction
for schema-plan data, catalog and metadata, with logical names and values bound.
Preserve physical version-1 tables/value tags and engine-owned indexes; new
managed metadata occupies ordinary reserved logical rows, not a new incompatible
private-table layout. Full scans are migration/adoption operations, never a
shortcut replacing engine-owned normal queries.

## Verification and stop condition

Commit/review this spec, API-0004 and TC-0026 before failing tests and code.
TC-0026 D01–ADR-0024 enumerate the acceptance partitions, including original
TC-0001/0002/0003 cases. Execute shared public API assertions against memory and
native SQLite, then the same Rust core in the supported real browser worker.
Observe genuine behavior RED and then GREEN. Preserve storage tests and browser
lifecycle assertions; record exact test mappings rather than claiming the old
Gherkin was automatically executed.

Native writer/reader processes and browser worker/page restart must demonstrate
metadata, ID continuity, managed rows and migrations without reinserting fixtures
before reading. Fixture-only corrupt metadata and actual SQL failures prove
strict rejection, atomicity and recovery. Run relevant workspace tests, format,
Clippy, core/memory WASM and browser builds, actual browser checks, traceability
and regenerated RTM. Preserve existing user CI runner tags. Local checks do not
establish remote CI execution or a consumer release.

## Quality characteristics and threat boundary

- Functional Suitability: TC-0001/0002/0003 mapped to TC-0026 public behavior.
- Performance Efficiency: engine-owned normal queries; scans restricted to
  lifecycle work. Preserve storage index evidence; no new numerical latency or
  migration-size budget is invented for NFR-0001/NFR-0004.
- Compatibility: distinguish application versions from private format/library
  versions; preserve v1 DDL/data, API-0002/0003 and NFR-0005's release policy.
- Interaction Capability: Rust API errors and open outcomes are explicit;
  no graphical interface or accessibility surface is introduced.
- Reliability: complete-chain/reset and ordinary writes commit metadata and rows
  together; failures return no partly initialized Database or partial upgrade.
- Security: REQ-0041/THR-0015 cover strict metadata, namespaces, bound values,
  expected-state checks and refusal to reset unknown/future/queue state.
- Maintainability: one public core, explicit migration catalogue and shared
  conformance rather than backend-specific schema policy.
- Flexibility: existing adapter substitution remains; no runtime/dependency or
  schema-serializer dependency is added.
- Safety: this library slice controls no physical system and introduces no
  safety-critical interface; functional data-loss hazards are THR-0015.

## Rollback

Revert task code through review, preserving databases, user changes and allocated
artifacts. Managed data model stores require a compatible schema-aware reader;
storage raw code does not provide schema lifecycle protection. Never delete a
file, clear an origin, downgrade metadata or run a missing-chain reset merely
as implementation rollback. Test cleanup removes only harness-owned fixtures.
All release/security/crash-durability claims remain separate verification gates.

## Implemented data model Rust boundary

SPEC-0026/API-0004 define the Rust schema, one-shot queries, lazy relations and
atomic lifecycle boundary. ADR-0031 fixes `__hoya_`, explicit queryable through
tables, seven authored migration primitives and strict queue-free reset/adoption
guards. TC-0026/0027/0028 define conformance and regressions. Use the
[runnable model example](../../../crates/hoyadb-storage-memory/examples/data_model.rs)
for the Rust surface; packaged SDK and sync acceptance remain separate.
