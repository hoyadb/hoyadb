---
id: TC-0026
title: Public schema relations queries and migrations conform across adapters
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [TP-0001]
verifies: [REQ-0001, REQ-0002, REQ-0003, REQ-0004, REQ-0005, REQ-0006, REQ-0033, REQ-0034, REQ-0041, NFR-0005, THR-0015]
depends_on: [SPEC-0026, API-0004]
tags: [test, data-model, migration, conformance, phase-two]
---

# TC-0026: Phase 2 public data-model conformance

D01–D14 use shared public/core-lifecycle assertions against fresh memory and
native SQLite factories, then the same Rust core in the supported real browser
worker. D15 adds SQLite-specific physical failure/compatibility fixtures; D16
requires real native process and browser worker/page restart. Each case needs
observed expected-behavior RED before implementation and GREEN afterward.
Existing API-0002/0003 conformance and browser lifecycle assertions remain.

Original acceptance mapping: TC-0001 → D01/D02/D03/D05/D06;
TC-0002 → D04/D05/D06; TC-0003 → D02/D07/D09/D10/D11/D12/D16.
This mapping is not a claim that a Gherkin runner executed the historical files.
Complete expected rows, IDs, versions, definitions, inventories and counters are
asserted, rather than only counts or parity between two implementations.

```gherkin
@REQ-0001 @REQ-0033 @REQ-0034
Feature: A declared schema owns typed local records
Scenario: D01 the public schema compiles portable types and implicit fields
  Given a positive-version walkthrough schema declaring customers orders order_items products and order_notes
  When a Database opens against a fresh supported adapter
  Then all declared tables exist with typed columns an implicit id and implicit __hoya_schema_version bookkeeping
  And to-one foreign keys omitted from columns materialize as Reference columns with declared optionality
  And explicitly declared compatible Reference columns are reused
  When complete records using every primitive optional omission and explicit Null are inserted and fetched
  Then exact strings i64 extrema finite float boundaries booleans timestamps and references remain typed and owned
  And persisted implicit row schema versions match the committed schema version while public fields and changes omit them
  And changing returned records cannot modify storage
Scenario: D02 invalid schemas and reserved inputs reject before mutation
  Given zero version invalid field paths conflicting fields or relations undeclared relation targets wrong foreign-key kinds or invalid through mappings
  When the public schema is declared or opened
  Then a typed schema error is returned before catalog table or row mutation
  Given reserved names in application tables columns relations or supplied record fields
  When a schema or public write uses those names
  Then it is rejected and all existing bookkeeping and application rows remain unchanged
  When an application reads or writes undeclared invoices or a reserved metadata table
  Then the operation is rejected without creating or exposing that table
Scenario: D03 generated IDs and public writes commit their counter together
  Given a supported managed database and an injected deterministic ID generator
  When an order is inserted without an application-declared id column
  Then it receives the exact nonempty generated id and correct implicit schema version
  Given the default generator and a committed persisted counter
  When records are inserted and later updated or deleted through the public API
  Then generated IDs use local followed by a colon and the persisted counter sequence
  And records retain complete typed field and missing-field semantics under API-0004
  When an injected generator yields an empty or duplicate ID or another write precondition fails
  Then the exact typed error is returned with no partial row change or committed counter advance
  And the custom generator is not silently retried and its external side effects are not claimed to roll back
  When default IDs collide with target-table rows or earlier inserts in the batch
  Then generation skips those collisions and the committed next counter remains correct
  And counter overflow rejects atomically without wrapping
  And application fields cannot override implicit bookkeeping

@REQ-0003 @REQ-0004 @REQ-0005
Feature: The public query builder preserves engine semantics
Scenario: D04 public predicates ordering and limit use the existing portable model
  Given records with distinct boundary values of every primitive kind and optional values
  When Eq Ne Lt Le Gt Ge and In are composed through the public builder
  Then complete expected records follow API-0003 exact types null and UTF-8 comparison rules
  And multiple predicates combine as AND and empty or duplicate In operands have their documented effect
  When ordering and zero one uncapped or larger limits are composed
  Then filtering precedes ordering and limit with ascending own-id ties even for descending order
  And no explicit ordering means ascending own id
  When paths operand types arity or a to-many traversal are invalid
  Then a typed query error is returned even on an empty table or at limit zero
  And predicates ordering and limit are executed by the adapter rather than a core table scan

@REQ-0002 @REQ-0004
Feature: All declared relation kinds are lazy first-class queries
Scenario: D05 to-one and has-many navigation performs no eager read
  Given customers with orders and optional dangling or cyclic references
  When a to-one or has-many relation query is constructed
  Then no source membership or target read occurs before fetch
  When the query is resolved with additional target predicates ordering and limit
  Then to-one yields its exact present target or no target for a null or dangling reference
  And has-many yields exactly the declared referencing rows
  And every returned row is complete owned and schema-valid
  And direct to-one dot-path predicates preserve API-0003 missing-target semantics
  And inverse navigation uses its declared relation without an application join
Scenario: D06 many-to-many link unlink and both directions are library-managed
  Given orders and products with a declared application-queryable order_products through table
  When the application links an order to two products using the declared relation
  Then navigating the order yields those two products and navigating either product yields that order
  And the application never constructs a join query or writes pivot rows to establish those memberships
  When relation fetch adds target predicates ordering and limit
  Then the target set is deduplicated before ordering and limit
  And unrelated memberships and dangling through targets do not create duplicate or synthetic records
  When a membership is unlinked through either declared direction
  Then both directions reflect its removal while unrelated links remain
  And unlink removes all duplicate membership rows atomically and reports their exact count including zero for an absent link
  When an existing link is requested with identical extra through payload
  Then the first existing membership is returned without a duplicate insertion
  When link payload differs or supplies FK or managed fields
  Then InvalidRow is returned without a membership mutation
  When source or target is missing in link or unlink
  Then RowNotFound identifies its exact table and id
  And malformed or ambiguous through definitions reject before any mutation

@REQ-0006 @REQ-0041 @NFR-0005
Feature: Opening a database respects its owned schema state
Scenario: D07 equal versions require the identical canonical definition
  Given committed managed rows schema metadata and an ID counter
  When the database reopens with the same application version and definition
  Then all application rows IDs bookkeeping inventory and counter remain unchanged
  When the same version declares a different column optionality relation or table
  Then the open is rejected without an inferred migration or reset
  And changing application schema version never changes SQLite private format marker 1
Scenario: D08 initialization and bounded Phase 1 adoption are explicit
  Given an empty unversioned logical store
  When any supported positive declared version opens
  Then declared tables canonical metadata counter and implicit bookkeeping initialize atomically
  Given a populated unversioned Phase 1 store whose every table and complete row matches a version-1 declaration
  When that declaration opens
  Then all application values and IDs are preserved and managed metadata and bookkeeping commit atomically
  And missing declared application tables are created without losing an existing table
  Given a populated unversioned store with extra tables invalid rows or a declaration above version 1
  When adoption is attempted
  Then it rejects with original inventory rows private DDL and metadata state unchanged
  And no destructive inference or implicit legacy reset occurs

@REQ-0006 @REQ-0041
Feature: An authored migration chain is one validated transaction
Scenario: D09 all explicit migration primitives reach their declared endpoints
  Given version 1 with three orders and full canonical metadata
  And authored complete ascending migrations from 1 to 2 and 2 to 3
  When the schema at version 3 opens
  Then each step runs in order and the final version is 3
  And AddTable DropTable AddColumn DropColumn RenameColumn Backfill and SetRelations have their exact authored effect
  And required added columns use explicit valid backfills or defaults rather than inferred values
  And retained application IDs and unrelated data remain unchanged
  And all final rows implicit row versions inventory and relation definitions match the declared endpoint
Scenario: D10 a failed step or endpoint rolls back the complete chain
  Given complete version-1 rows metadata inventory and ID counter
  And a valid first migration followed by an invalid typed backfill invalid operation or endpoint mismatch
  When the target version opens
  Then a typed migration or validation error is returned
  And all original version-1 tables complete rows metadata counter and implicit versions remain unchanged
  And no earlier migration prefix or later operation is published
  And the failed migration never falls through to a reset
  And the recoverable open returns the original owned engine for inspection and retry
Scenario: D11 a supported forward missing chain resets atomically
  Given a known supported queue-free version-1 database containing three orders
  And no complete authored migration chain reaching the declared version 4
  When version 4 opens
  Then owned application tables are recreated at version 4 with zero old orders
  And canonical target metadata and implicit schema state commit together while retaining the next-ID counter
  And no available chain prefix or inferred transformation is applied
  And private physical SQLite tables file and format marker remain intact
  And no backend resync or network call occurs

@REQ-0033 @REQ-0041 @THR-0015
Feature: Unsupported metadata cannot become permission to erase data
Scenario: D12 downgrade future state unknown bookkeeping or queues reject unchanged
  Given a committed managed store with a downgrade request unsupported future metadata unknown reserved table or queue-present state
  When open migration or missing-chain reset is attempted
  Then an explicit typed error is returned without mutation
  And complete original application and bookkeeping rows inventory counter and private DDL remain unchanged
  And no queue is interpreted dropped migrated or silently discarded
Scenario: D13 canonical metadata decoding is strict and lossless
  Given canonical definitions containing exact Unicode NUL quotes and distinct delimiter-looking names
  When metadata is encoded persisted decoded and compared
  Then distinct definitions remain distinct and the same definition has deterministic bytes
  Given unknown duplicate or missing metadata fields invalid versions counters tags UTF-8 lengths truncated values trailing bytes or inconsistent definitions
  When catalog metadata is read or the managed Database opens
  Then decoding or validation rejects without default metadata partial results or state changes
  And malformed metadata is never treated as a fresh empty store
Scenario: D14 schema plans compare expected state and validate the complete candidate
  Given one catalog snapshot and a different already committed current state
  When a schema plan with the stale snapshot is submitted
  Then it rejects before Create Drop Write or metadata mutation
  Given a matching expected state but a candidate with an undeclared table missing required field wrong row version or incompatible value
  When the schema plan would commit
  Then complete final inventory and all rows are validated against the target storage schema
  And the whole plan rejects with the exact pre-plan state preserved
  And lifecycle scan results are owned snapshots and normal query APIs do not use that scan capability

@REQ-0041 @THR-0015
Feature: SQLite lifecycle operations preserve physical ownership and atomicity
Scenario: D15 actual ordinary SQL errors roll back data metadata and counters
  Given dedicated native or supported browser SQLite fixtures with compatible private version-1 physical tables
  And fixture-local SQL aborts or a native lock or actual commit failure after an earlier successful SQL prefix
  When initialization adoption public insertion migration or supported missing-chain reset executes
  Then BackendFailure is returned without a partial Database row change schema definition or counter advance
  And fresh reads or reopen see the entire original state
  And later operations can succeed after the fixture-local failure condition is removed
  And unrelated incompatible files still reject without initialization migration or reset
  And all logical schema names values and codec bytes remain bound data rather than executable SQL
Scenario: D16 independent process worker and page restart preserve managed state
  Given a writer process or real browser worker initializes a managed schema inserts generated-ID records and commits an authored migration
  When that owner terminates and a separate native reader process or new browser worker opens the same dedicated store
  Then complete migrated rows relation navigation canonical schema version implicit row versions and ID counter remain exact
  And generating another default ID continues the persisted sequence without collision
  When a full browser page reload reads the same store before any fixture insertion
  Then it sees the same committed managed data and metadata
  And a rolled-back migration or reset failure remains rolled back after restart
  And native process results are not claimed as crash or power-loss durability evidence
```

D05 uses an adapter wrapper/counter to prove query construction does no storage
read; recording only elapsed time cannot prove laziness. D09 supplies explicit
expected values and canonical endpoints for each operation rather than merely
asserting the version advanced. D10–D15 compare complete state after failure,
including catalog and counter; counting surviving application rows is insufficient.
D13 metadata corruption uses only dedicated fixture rows/SQL and never a
production validation bypass or reset flag.

D15 runs SQLite failure partitions where supported: browser fixtures do not
pretend simultaneous connections/workers are supported. Actual SQL trigger
abort proves an ordinary write failure; it is labeled separately from a genuine
commit failure. Browser capability/dispatcher simulations do not satisfy D15.
D16 closes every live connection before using fixture inspection or cleanup;
OPFS namespace cleanup retains the existing acknowledged shutdown protocol.
No test clears user files or an entire browser origin.

The implementation report records exact shared case/function mapping, RED/GREEN,
restart observations, commands and limitations. No case is marked executed by
this specification alone. Required formatting, lint, build, traceability and
RTM checks remain part of delivery; local success is not remote CI evidence.

## Execution — 2026-10-22

The [Phase 2 report](../../../implementation/phase-two-data-model.md) records
actual RED/GREEN observations, shared memory/native/browser execution and
limitations. This is an explicit Rust assertion mapping, not a claim that a
Gherkin runner executed this Markdown file.
