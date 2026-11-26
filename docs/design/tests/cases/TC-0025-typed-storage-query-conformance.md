---
id: TC-0025
title: Memory native and browser storage share typed query and commit semantics
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [TP-0001]
verifies: [REQ-0001, REQ-0003, REQ-0004, REQ-0005, REQ-0034, REQ-0040, THR-0014]
depends_on: [SPEC-0025, API-0003]
tags: [storage, conformance, query, security, phase-one]
---

# TC-0025: Typed storage conformance

The same named P01–P10 assertions run against fresh memory/native SQLite and
the supported real-browser SQLite worker, with complete expected rows, exact
error variants and pre/post-state assertions. P11 adds native/browser physical
format fixtures and restart behavior; P12 records engine-specific costs and P13 covers configured CI build paths.
Keep existing API-0002 conformance unchanged. Observe genuine missing-behavior
RED before implementation, then GREEN; compilation alone does not pass a case.
No claimed execution is recorded here until the implementation report supplies
actual evidence. All paths, database names, SQL fixtures and data are test-owned.

```gherkin
@REQ-0001 @REQ-0034 @REQ-0040
Feature: Exact typed rows under an explicit storage schema
Scenario: P01 all primitive types roundtrip without coercion
  Given declared physical tables and a schema containing every primitive and optional fields
  When typed batches insert then replace complete rows
  Then strings integers finite floats booleans millisecond timestamps references and Null roundtrip exactly
  And integers include both i64 extrema and timestamps include negative zero and positive values
  And floats include subnormal fractional integral and maximum positive and negative finite values
  And constructing NaN or either infinity fails and negative float zero becomes positive zero
  And omitted optional fields remain omitted while explicit Null remains present
  And modifying returned rows or snapshots does not modify stored rows
Scenario: P02 typed writes validate schema and rows without changing raw CRUD
  Given an invalid relation target or foreign key or duplicate column relation name or invalid column path name
  When either typed method is called even with an empty batch or zero limit
  Then InvalidSchema is returned without modifying rows
  Given a valid schema and known physical table
  When a typed insert or replace supplies unknown fields implicit id missing required fields required Null or wrong value variants
  Then InvalidRow identifies the table and id and no mutation is committed
  When a typed mutation or query names an undeclared or uncreated table
  Then TableNotFound is returned without creating it
  And all original API-0002 CRUD and atomic cases retain their exact behavior

@REQ-0003 @REQ-0005
Feature: Engine execution of portable predicates and ordering
Scenario: P03 every supported comparison operator uses the declared type
  Given distinct ordered values and exact-boundary values of every primitive kind
  When each of Eq Ne Lt Le Gt Ge and In is queried with matching typed operands
  Then complete expected rows match API-0003 comparison rules for that kind
  And multiple predicates retain only rows satisfying every predicate
  And empty In returns no rows and duplicate In values do not duplicate rows
  And implicit id is compared as an exact string
Scenario: P04 optional values have explicit equality and ordering semantics
  Given rows with omitted optional fields explicit Null and present values
  When Eq Null Ne Null In containing Null and ordered comparisons are queried
  Then omission compares as Null and equality and In preserve the expected membership
  And ordered comparisons involving Null never match
  When ascending and descending ordering are requested
  Then Null sorts first ascending and last descending without rewriting stored shape
Scenario: P05 filter then order then limit has deterministic ties
  Given matching and nonmatching rows with repeated order values and Unicode ids
  When ascending or descending order and an uncapped zero one or larger limit are queried
  Then filtering precedes ordering and limit
  And every tie uses ascending own id even for descending field order
  And no explicit order produces ascending own id
  And strings references and ids use exact unsigned UTF-8 byte order including case Unicode and embedded NULs
  And booleans use false before true and numeric kinds use their own numerical order
Scenario: P06 direct to-one predicates and ordering stay inside one declared edge
  Given orders referencing customers with present optional null dangling and omitted references
  And a reference cycle among declared tables
  When querying customer.country and customer.id using predicates and ordering
  Then present targets match by target field with exact expected source rows
  And predicates on a missing target are false including Ne and Eq Null
  And ordering treats unresolved targets as Null
  And optional Null on a present target follows ordinary Null comparison rules
  And no recursive traversal cascade or foreign-key existence check occurs
  When a declared target table referenced by a predicate or ordering has not been physically created
  Then TableNotFound identifies that target even when limit is zero
Scenario: P07 invalid queries fail before execution on empty and nonempty tables
  Given a valid schema and a zero limit or empty or populated table
  When a query contains an unknown field an unknown relation a two-edge path or a to-many path
  Or a scalar operator has zero or multiple operands
  Or any operand including an In element has a different declared type or an invalid Null
  Then InvalidQuery is returned without partial rows
  And Integer and Float operands never coerce into one another

@REQ-0040 @THR-0014
Feature: Publication follows successful commit
Scenario: P08 committed changes are complete owned and ordered
  Given original rows across two declared tables
  When a typed batch inserts replaces removes and repeatedly changes the same id
  Then one vector returns complete before and after rows in mutation order
  And each later before snapshot sees preceding changes in that batch
  And an absent removal and an identical replacement emit no entry
  And an empty batch returns an empty vector
  And mutating returned snapshots cannot change any committed row
Scenario: P09 validation or domain failure publishes nothing
  Given complete original rows and a typed batch with a valid mutation prefix
  When a later operation has an invalid row duplicate insertion missing replacement or unknown table
  Then the first corresponding typed error is returned without a change vector
  And complete original rows and subsequent query results remain unchanged
  Given raw writes inserted an invalid row
  When a query would return that row or a typed write needs its invalid prior snapshot
  Then InvalidRow is returned without partial output or mutation
  And a query not returning that row need not scan it
  Given raw writes contain wrong-variant fields or required fields that are absent or Null
  When predicates consult these fields on source or related rows
  Then every operator including Ne and In fails for those invalid fields
  And ordering treats these invalid fields as Null while complete returned source rows still validate
Scenario: P10 query and indexes follow all write paths and rollback
  Given selective queries have built an adapter's physical lookups
  When typed batches or raw API-0002 operations change indexed values remove rows or replace complete rows
  Then later queries reflect the new complete state without stale membership
  When a batch fails after an indexed mutation prefix
  Then later queries match the complete pre-batch state
  And names paths and operand strings containing quotes NULs Unicode and SQL-looking text remain data

@REQ-0040 @THR-0014
Feature: SQLite physical compatibility and ordinary failures
Scenario: P11 native and browser SQLite preserve v1 data and strict decoding
  Given a preextension compatible private v1 database containing old value tags
  When the adapter opens it and writes new primitive tags through a typed batch
  Then existing table DDL marker old rows and old-tag behavior stay unchanged
  And native process restart or browser worker and page restart preserve complete new rows
  And actual added indexes survive or rebuild without changing logical data
  Given dedicated malformed tag payload float NaN or infinity nonrepresentable integer UTF-8 or storage-class fixtures
  When a returned row is decoded
  Then BackendFailure is returned without a coerced or partial row
  Given a fixture-local ordinary SQL write or commit failure after a valid batch prefix
  When the typed batch executes
  Then BackendFailure returns without changes and all original rows and indexed query results remain after reopen
  And failed unrelated or incompatible database opens still leave all schema and data unchanged

@REQ-0034
Feature: Physical indexes have observed costs
Scenario: P12 selective workload proves engine lookup and records costs
  Given recorded synthetic populations with a selective field and increasing unrelated rows
  When storage constructs its indexes during population or first use and the same selective equality query is repeated
  Then actual SQLite EXPLAIN QUERY PLAN shows index lookup for that predicate on native and browser
  And memory candidate counts demonstrate fewer examined rows than a full scan for the same predicate
  And complete selected rows match across all adapters
  When indexed values are updated raw writes occur a batch rolls back and storage reopens
  Then lookup results remain correct and construction update rebuild lookup and ordering work are recorded separately
  And measured sizes durations and plans are reported without inventing latency targets or claiming all operators avoid scans

@REQ-0034
Feature: Configured CI builds the supported targets
Scenario: P13 configured CI commands compile the browser adapter with the pinned graph
  Given the checked-in CI Rust image commands and approved locked native and browser manifests
  When locked dependency preparation workspace checks core and memory WASM builds and the browser adapter WASM build run in the configured environment
  Then the required compiler and linker are available and every selected command succeeds
  And user runner tags remain unchanged
  And no remote CI or real-browser execution is inferred from local compile checks
```

P03 uses equivalence partitions per kind and operator, not only an integer
example. P05's expected IDs must be explicit; comparing two implementations
alone could accept the same mistake twice. P11 corrupts only fixture-owned
SQLite files through fixture SQL, never a production corruption flag. Native
locks/aborting triggers and deferred constraint commit failure are appropriate
ordinary failure mechanisms; browser uses supported local SQL fixtures rather
than pretending simultaneous workers/connections are supported. Timestamp and
integer bridge payloads remain decimal strings, float payloads are finite and
lossless, and no i64 value is silently narrowed through a JavaScript Number.
P12 diagnostics are concrete-adapter evidence, not the shared storage trait.

## Executed acceptance — 2026-10-22

P01–P13 pass at the bounded SPEC-0025 storage boundary. The
[execution report](../../../implementation/phase-one-storage.md) records RED/GREEN,
review regressions, 70 host tests, real worker/page restart and cleanup, physical
index work/costs, source-only archive checks and the pinned-container build.
Remote CI, publication and broader production/release requirements are separate.
