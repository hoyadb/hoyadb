---
id: API-0003
title: Typed storage queries and committed row changes
type: api
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-27
parents: [ARC-0001]
satisfies: [REQ-0001, REQ-0003, REQ-0004, REQ-0005, REQ-0034, REQ-0040]
depends_on: [SPEC-0025, API-0002, ADR-0030]
tags: [rust, storage, query, internal]
---

# API-0003: Typed storage queries and committed changes

This additive, unpublished Rust contract supplies storage prerequisites for
data model data-model and reactivity reactivity work. API-0002 CRUD and atomic-batch
semantics remain available. There is no consumer schema builder, schema
persistence, migration, live-query subscription, transport or production bridge.
Core declares types and validates supplied schemas/queries; each adapter executes
filtering, direct to-one lookup, ordering and limit against its own storage.
Core MUST NOT obtain all rows and execute queries on an adapter's behalf.

## Types

All names below live in `hoyadb_core::storage`; maps are `BTreeMap` and method
futures follow API-0002's borrowing and `Send` bounds. Types support `Clone`,
`Debug`, `PartialEq`, `Eq`; the finite float wrapper stores canonical bits.

```rust
pub struct FiniteFloat(/* private */);
impl FiniteFloat {
    pub fn new(value: f64) -> Option<Self>; // None for NaN or either infinity
    pub fn get(&self) -> f64;
}
// Add to the existing Value enum:
// Float(FiniteFloat), Timestamp(i64), Reference(String), Null
pub enum ColumnType { String, Integer, Float, Boolean, Timestamp, Reference }
pub struct Column { pub kind: ColumnType, pub optional: bool }
pub enum Relation {
    ToOne { table: String, foreign_key: String },
    HasMany,
    ManyToMany,
}
pub struct TableSchema {
    pub columns: BTreeMap<String, Column>,
    pub relations: BTreeMap<String, Relation>,
}
pub struct Schema { pub tables: BTreeMap<String, TableSchema> }
pub enum Operator { Eq, Ne, Lt, Le, Gt, Ge, In }
pub struct Predicate {
    pub field: String,
    pub operator: Operator,
    pub values: Vec<Value>,
}
pub enum Direction { Asc, Desc }
pub struct OrderBy { pub field: String, pub direction: Direction }
pub struct Query {
    pub table: String,
    pub predicates: Vec<Predicate>,
    pub order: Option<OrderBy>,
    pub limit: Option<usize>,
}
pub struct CommittedChange {
    pub table: String,
    pub id: String,
    pub before: Option<Row>,
    pub after: Option<Row>,
}
```

Timestamp means signed UTC milliseconds since the Unix epoch, without timezone
conversion, clock reads or calendar parsing. References hold exact target row IDs,
are distinct from strings, and do not imply referential-integrity enforcement.
Finite float zero is canonical positive zero; all other finite f64 values retain
their exact value, including subnormals and extrema. No int/float coercion occurs
in the logical contract. Values and complete owned `Row`/`Mutation` retain Eq.

## Operations and failures

```rust
pub trait TypedStorage: Storage {
    fn query(&self, schema: &Schema, query: &Query)
        -> impl Future<Output = Result<Vec<Row>, StorageError>> + Send;
}
pub trait TypedTransactionalStorage: TypedStorage + TransactionalStorage {
    fn apply_typed_batch(&mut self, schema: &Schema, mutations: Vec<Mutation>)
        -> impl Future<Output = Result<Vec<CommittedChange>, StorageError>> + Send;
}
// Add to StorageError:
// InvalidSchema { message: String }
// InvalidQuery { message: String }
// InvalidRow { table: String, id: String, message: String }
```

`query` implements REQ-0003/0004/0005; `apply_typed_batch` implements REQ-0040;
both implement REQ-0001's storage-level declared-table/type precondition and
REQ-0034's adapter substitution. Variant and supplied table/id coordinates are
stable; diagnostic message text is not. No payload logging or telemetry is added.
Every operation first validates the supplied Schema, returning InvalidSchema on
failure. Query then validates the whole query, including when limit is zero or
the table empty. Typed writes process mutations in order and stop at the first
row/domain/operational error. All errors leave rows unchanged and return no
committed changes. API-0002 domain errors and BackendFailure remain distinct.
An undeclared requested table returns TableNotFound; a declared but uncreated
physical table also returns TableNotFound. Typed methods never create tables.
Each table referenced by a predicate or order path must also physically exist;
reject its absence as TableNotFound before executing, including at limit zero.

## Supplied-schema validation

Names are exact strings. Column and relation names MUST be nonempty, contain no
dot and differ from implicit `id`. A relation name MUST NOT shadow a column.
Every ToOne target table MUST be declared; its foreign_key MUST name a Reference
column on the source table. HasMany/ManyToMany are markers solely to reject
unsupported traversal; their navigation/target declaration is data model work.
The reserved public bookkeeping namespace and schema version lifecycle remain
undecided here; no private physical prefix becomes a public restriction.

Typed insert/replace MUST reject unknown fields, a field named `id`, missing
required fields, Null in required fields and wrong Value variants as InvalidRow.
Optional omission and explicit Null are query-equivalent but retain their exact
stored shape in returned rows and change snapshots. Replacement is complete,
not a patch. Typed remove validates the table and preserves API-0002 absent-row
no-op behavior. A single typed mutation is a one-element typed batch.

For a field consulted by a predicate, a present non-Null Value whose variant
differs from its declared ColumnType matches no operator, including Ne and In.
A required field that is absent or Null likewise matches no operator. For
ordering, these invalid field values sort as Null. Complete returned source
rows still undergo ordinary InvalidRow validation; this rule does not validate
complete nonreturned or related rows.

API-0002 raw writes deliberately remain unchecked. A query validates complete
rows it returns against the supplied schema; a malformed returned row produces
InvalidRow and no partial result. It need not scan or validate nonreturned rows.
Strict physical decoding failures remain BackendFailure. Typed replacement or
removal that needs a malformed existing snapshot fails without changing it.
Changing supplied schema cannot migrate or erase persisted data.

## Query semantics

Field paths address implicit own `id`, an own declared column, or exactly one
declared ToOne relation followed by `id` or a target column. More than one dot,
unknown names and HasMany/ManyToMany traversal return InvalidQuery. Predicates
and order use this same field resolution. Each non-In predicate has exactly
one value. In accepts any number, including zero and duplicates. Operands MUST
match the declared field kind exactly; Null is permitted only for optional
fields. Invalid paths, arity or operand types fail before executing a query.

Predicates combine by AND. All seven operators apply to each primitive type;
boolean order is false before true, numeric kinds use their own numerical
order, and string/reference/id order is unsigned UTF-8 byte order (no locale,
case folding or normalization). Empty In matches no rows. Eq/Ne and In compare
Null as a distinct value; ordered comparison with Null is false. For an
optional omitted field, use Null when evaluating predicates and ordering.

A null/omitted foreign key or dangling target has no related row. Every
predicate traversing that relation is false, including Ne and Eq Null; there
is no outer-join synthetic record. For ordering only, an unresolved related
field sorts as Null. A target row with an optional null field is a present
record and follows ordinary Null semantics. Reference cycles are harmless:
only one edge is traversed, never recursively. No cascade or FK existence
check is implied by a write.

Filter first, then order, then limit. A missing order means ascending own id.
An explicit order uses ascending own id to break every tie, including descending
field order. Null sorts before present values ascending and after them descending.
Limit None is uncapped; Some(0) returns no rows after schema/query validation.
All returned rows are complete owned snapshots; modifying them cannot alter
storage. One adapter operation has one local view; concurrent clients and
cross-process snapshots remain outside this contract.

## Committed changes and ownership

For each effective successful mutation, return table/id plus complete owned
before/after snapshots in mutation order: insert None/Some, replacement
Some/Some, removal Some/None. An absent removal or identical replacement emits
no entry. Repeated changes to one ID are not coalesced: later before snapshots
reflect earlier operations in that same batch. Empty batches return an empty
vector. No changes escape before successful commit, including an operational
commit failure; failed batches preserve pre-batch state. Raw API-0002 calls do
not emit changes. Callers receive the vector once; no callback, durable change
log, backlog, global revision or subscription handle is created.

Rust reactivity can consume these snapshots later, but result membership,
incremental maintenance, subscription sharing, scheduling and platform callback
ownership are not implemented by returning this vector. Native/worker SQLite
operations retain API-0002's synchronous ready-future behavior and ownership;
no additional runtime, thread transfer or simultaneous connection is promised.

## Private SQLite representation and indexes

Keep the four-table private version-1 DDL and existing tags 1/2/3 unchanged.
New tags: 4 finite Float in int_value with text_value NULL; 5 Timestamp integer;
6 Reference text; 7 Null with both payloads NULL. Float binds an SQLite REAL;
INTEGER affinity can retain an exactly integral float as INTEGER. The decoder
MUST accept only finite REAL or exactly f64-representable INTEGER for tag 4,
canonicalize zero, and reject every incompatible storage class/payload. Timestamp
requires INTEGER, Reference requires UTF-8 TEXT, and Null forbids payloads.
Do not encode floats as decimal/bit text and compare SQL casts.

Engine-owned secondary indexes may be added non-destructively after successful
format validation; existing table DDL/version and rows stay unchanged. Older
code can still open old-value databases; it may explicitly reject a new value
tag when reading it. Rollback preserves data and requires a compatible reader
for rows using new tags. No automatic downgrade, reset or application migration.
Native and browser SQLite share SQL query planning/execution; logical names
and operands remain bound values. Memory owns its lookup structures and may
build them lazily, invalidating/rebuilding after raw writes. All writes must
keep query results/index state coherent, including rollback and reopen.

Adapter-specific diagnostics can expose query_plan (SQLite EXPLAIN QUERY PLAN)
and candidate_count (memory rows examined) without extending the shared trait.
They are test/evidence helpers, not a portable optimizer contract. Record actual
index construction, update, lookup and ordering costs separately; predicate
parity alone is not performance evidence (TC-0025 P12).

## SQLite structural complexity limits — reviewed clarification

Portable semantics do not imply unlimited SQL statement complexity. SQLite
limits expression depth and join count; excessive separately chained predicates
can return BackendFailure without mutation even if memory can evaluate them.
The pinned native build rejects a query of 1000 distinct ANDed Ne predicates;
this is a structural statement limit, not an In operand-count limit. SPEC-0028
removes operand-generated expression/bind growth through native IN and temporary
sets, while arbitrary numbers of independent Boolean clauses remain subject
to the selected engine's limits and available resources.

## Validated command-owner query capability (SPEC-0037 R8)

TypedStorage gains a provided `query_validated(proof: ValidatedQuery<'_>)`
method with the same borrowing, Send and result contract as query. Its default
calls query with the proof's schema and query, so existing adapters need no
change. This optimization is unavailable through raw query or plain Database.
ValidatedQuery has private borrowed schema/query fields, public read-only
getters, a core-only constructor and no Clone, Copy or Default. No public API
returns one. Exactly two core sites construct it: fetch and the overlay
base-window query, each gated on successful strict command-owner open and
Exclusive storage ownership. The common overlay over an Unproven owner's
private snapshot never receives a proof. Thus a public caller cannot
assert validation using a Boolean or a doc-hidden method alone. The schema and
query come only from the proof, with no separately supplied alternative schema.
Adapters may optimize only SPEC-0037 R8's eligible shapes and delegate all other
shapes to the existing query. The SQLite path uses existing indexes without
changing persisted data. Raw unchecked-row normalization and ordering stay exact.
