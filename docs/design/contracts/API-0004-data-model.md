---
id: API-0004
title: Public Rust data model queries relations and schema lifecycle
type: api
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-11-10
parents: [ARC-0001]
satisfies: [REQ-0001, REQ-0002, REQ-0003, REQ-0004, REQ-0005, REQ-0006, REQ-0033, REQ-0034, REQ-0041]
depends_on: [SPEC-0026, API-0003, ADR-0031]
tags: [rust, schema, relations, migrations, data-model]
---

# API-0004: data model data model

Origin: operator request to start and finish full roadmap data model. This is the
public Rust library surface in `hoyadb_core::model`, composed with the existing
memory/native/browser adapters. Guide TypeScript names remain reference syntax,
not a promised SDK package. No new dependency, network, runtime or published
binding. Application data retains DM-0001 classification/retention; schema
definitions and metadata remain local until the database is reset/deleted.
Errors are returned without payload logging.

## Schema and projection

Reuse `storage::{Column, ColumnType, Value, Row, Operator, Direction}`.

```rust
pub struct SchemaDefinition { pub version: u32, pub tables: BTreeMap<String, TableDefinition> }
pub struct TableDefinition {
    pub columns: BTreeMap<String, Column>,
    pub relations: BTreeMap<String, RelationDefinition>,
}
pub enum RelationDefinition {
    ToOne { table: String, foreign_key: String, optional: bool },
    HasMany { table: String, foreign_key: String },
    ManyToMany { table: String, through: String, source_relation: String, target_relation: String },
}
impl SchemaDefinition {
    pub fn validate(&self) -> Result<(), StorageError>;
    pub fn storage_schema(&self) -> Result<storage::Schema, StorageError>;
    pub fn canonical(&self) -> Result<String, StorageError>;
    pub fn from_canonical(input: &str) -> Result<Self, StorageError>;
}
```

Schema version is 1..=u32::MAX, independent of library/private SQLite versions.
Table/column/relation/FK names must be nonempty, contain no dot, and avoid `id`
and the fixed `__hoya_` prefix. Names otherwise retain exact UTF-8, including
quotes/NUL. Relations and columns cannot shadow one another. Every target and
through table must exist. Custom validators/types are not in this catalogue.

ToOne declares an implicit Reference column at foreign_key, with its optional
flag. An explicit column at that name must have exactly the same kind and flag.
Multiple ToOne declarations sharing a key must agree. HasMany references a
Reference column on its target, supported by a ToOne back to its source.
ManyToMany names two distinct ToOne declarations on its explicit through table:
the source relation targets its source table, the target relation its target
table, and their FK names must differ. Both navigation directions have explicit
named declarations; the library never guesses inverse names. Cycles/self-links
are valid. Eager/recursive loading and to-many predicate traversal are absent.

Each persisted application row gains the managed Integer field
`__hoya_schema_version`, equal to its application schema version. It is in the
internal storage projection; it is absent from public Row.fields. An application
cannot declare, insert, replace or query managed names. The implicit primary key
is Row.id, not an application column. Compilation adds API-0003 ToOne paths and
to-many markers without changing API-0003 raw storage behavior.

Canonical encoding is versioned, length-prefixed UTF-8 tokens, sorted maps,
explicit primitive/relation/optional tags and all normalized implicit FK columns.
It includes schema version and every relation option. Decoder checks lengths,
UTF-8 boundaries, counts, duplicates, tags, complete consumption, schema validity
and equality with re-encoding; noncanonical or malformed input rejects. No
hash collision, debug-format persistence, external serializer or guessed schema.

## Database, writes and IDs

```rust
pub struct Database<S> { /* owns engine and immutable normalized schema */ }
pub enum Write {
    Insert { table: String, fields: BTreeMap<String, Value> },
    Replace { table: String, id: String, fields: BTreeMap<String, Value> },
    Remove { table: String, id: String },
}
impl<S: storage::SchemaStorage> Database<S> {
    pub async fn open(storage: S, schema: SchemaDefinition, migrations: Vec<Migration>) -> Result<Self, StorageError>;
    pub async fn open_recoverable(storage: S, schema: SchemaDefinition, migrations: Vec<Migration>) -> Result<Self, (StorageError, S)>;
    pub fn with_id_generator(self, generator: impl FnMut() -> String + Send + 'static) -> Self;
    pub fn schema(&self) -> &SchemaDefinition;
    pub fn into_storage(self) -> S;
    pub async fn insert(&mut self, table: &str, fields: BTreeMap<String, Value>) -> Result<Row, StorageError>;
    pub async fn replace(&mut self, table: &str, id: &str, fields: BTreeMap<String, Value>) -> Result<Row, StorageError>;
    pub async fn remove(&mut self, table: &str, id: &str) -> Result<bool, StorageError>;
    pub async fn apply_batch(&mut self, writes: Vec<Write>) -> Result<Vec<storage::CommittedChange>, StorageError>;
    pub fn query(&self, table: &str) -> QueryBuilder;
    pub fn related(&self, table: &str, id: &str, relation: &str) -> QueryBuilder;
    pub async fn fetch(&self, query: &QueryBuilder) -> Result<Vec<Row>, StorageError>;
    pub async fn link(&mut self, table: &str, id: &str, relation: &str, target: &str, fields: BTreeMap<String, Value>) -> Result<Row, StorageError>;
    pub async fn unlink(&mut self, table: &str, id: &str, relation: &str, target: &str) -> Result<usize, StorageError>;
}
```

`open_recoverable` returns the still-owned engine alongside an error so callers
can inspect/retry an in-memory store without losing it to ownership transfer.
`open` is the convenience path that drops the returned engine on failure (closing
a persistent connection, or dropping the caller-transferred memory value).
Neither path commits any partial state. Public Database async futures follow
the engine's owner-thread affinity and do not promise Send; the underlying
API-0002/0003/SchemaStorage operation futures retain their Send bounds.

All public writes are schema-checked atomic typed batches, including the
metadata/counter update. Returns contain owned public rows, with managed fields
removed, and ordered effective before/after changes for app rows only. Failed
batches return no rows/changes, leave all persisted rows/counters unchanged,
and never create an undeclared table. Replace is complete, not a patch.
Removal of a missing row on a known table is false/no-op.

Default generated IDs are `local:<positive decimal counter>`. The global
per-store next counter persists atomically with the insert; it survives reopen
and is retained by reset. Skip already-existing generated IDs in the target
table, checking both stored rows and earlier inserts in the same batch. Reject
counter overflow instead of wrapping. An injected generator's exact nonempty
string replaces default generation, is checked for duplicates, and is not
silently retried; its external side effects cannot roll back. The library does
not claim cross-device globally unique IDs or backend remapping in this phase.

`link`/`unlink` accept a ManyToMany relation, verify source/target existence and
derive through keys from the declared relations. link fields carry additional
through-row payload; caller-supplied FK/managed keys reject. A new link creates
one normal typed row. An existing link returns its existing row only if its
additional payload equals the requested payload; otherwise InvalidRow, with no
silent update. Equality compares the exact stored shape: an omitted optional
field and an explicit Null are distinct payloads (API-0003). Duplicate through rows from explicit ordinary CRUD do not
duplicate navigation results. unlink removes all matching through rows in one
batch and returns their count; an absent link is zero. Other relation kinds
reject these methods. No cascade deletion or automatic FK existence enforcement
is imposed on ordinary writes. Missing source/target in link/unlink returns
RowNotFound with exact table/id.

## Lazy queries and navigation

```rust
impl QueryBuilder {
    pub fn where_(self, field: &str, operator: Operator, value: Value) -> Self;
    pub fn where_in(self, field: &str, values: Vec<Value>) -> Self;
    pub fn order_by(self, field: &str, direction: Direction) -> Self;
    pub fn limit(self, count: usize) -> Self;
    pub async fn fetch<S: storage::SchemaStorage>(&self, db: &Database<S>) -> Result<Vec<Row>, StorageError>;
}
```

Building query/navigation objects performs no storage reads. fetch resolves
against the current database, including source FK/through membership at fetch
time. Unknown table/relation/path and invalid operands reject even on empty
results/zero limit, before data-dependent early returns. Missing navigation
source or optional/dangling ToOne yields no rows. HasMany uses target FK equality;
ManyToMany queries through membership then target id In, deduplicating targets.
Filters, order and limit execute in the target engine, after membership
restriction, using all API-0003 null/kind/UTF-8/tie semantics. Source/through
queries also execute in the engine; core does not load/filter a raw table for
ordinary reads. No OR, offset/cursor, aggregate or live observe is introduced.
Query objects contain a portable owned plan and can be reused on another
compatible Database; no dangling engine borrow, handle or materialized relation.

## Authored migration lifecycle

```rust
pub struct Migration { pub from_version: u32, pub to: SchemaDefinition, pub steps: Vec<MigrationStep> }
pub enum MigrationStep {
    AddTable { table: String, definition: TableDefinition },
    DropTable { table: String },
    AddColumn { table: String, name: String, column: Column, default: Option<Value> },
    DropColumn { table: String, name: String },
    RenameColumn { table: String, from: String, to: String },
    Backfill { table: String, column: String, value: Value },
    SetRelations { table: String, relations: BTreeMap<String, RelationDefinition> },
}
```

Open validates declaration and all authored migration descriptors before changing
storage. Each from_version is unique and positive; every edge strictly increases
to its explicit target definition. Ambiguous outgoing edges, invalid endpoints,
overshooting edges on the selected path and malformed operations reject without
reset. A gap in an otherwise valid graph means no complete chain: reset the
supported forward store, without partially running the available prefix.

Stored and requested versions equal: require identical canonical definition,
then no write. Greater stored version: reject downgrade. Ascending complete
chain: scan owned rows for migration only, execute each explicit step against
the prior definition/data, update managed row versions and validate each exact
endpoint; commit all steps and final metadata in one adapter transaction.
Table/column source must exist, destination must not; required AddColumn needs
a valid constant default if rows exist, optional no-default leaves omission;
Backfill sets the declared field on every row; the implicit key of a ToOne set
by an earlier SetRelations in the same migration is such a declared field, with
that relation's optional flag. Rename changes only its named
field; affected relation declarations must be explicitly updated using
SetRelations. Schema differences are never used to infer operations. Wrong
default/type, invalid final rows, endpoint mismatch or any storage/COMMIT failure
rolls back every row/table/version/counter; it never triggers reset.

Supported forward missing-chain reset atomically clears/recreates application
tables at the declared definition, preserves the next-ID counter, and commits
metadata. Never delete the SQLite file/private tables. No metadata + empty store
initializes at any positive version. No metadata + populated store may adopt
only version 1 after every existing table/row validates and no table is lost;
create missing declared tables and add managed row-version fields atomically.
Other unversioned populated stores reject unchanged. A present-but-empty,
malformed or future metadata table is an error, not an unversioned store.
Here "empty" means the store has no logical application table; an existing
application table counts as population even when it holds no row, so that no
table is lost. Private physical tables never count.

Unknown reserved bookkeeping, including a future queue table, always rejects
before migration/reset. The supported format is queue-free; this guard does not
claim recovery for an unspecified queue or external queue file. Future sync must
extend the format/policy explicitly. Database ownership remains exclusive;
multi-client snapshot/concurrency and process-crash durability remain separate
roadmap gates. No sync starts before open/migrations complete.

## Additive storage capability and metadata

The following types live in `hoyadb_core::storage`. They are an internal trusted
core/adapter capability alongside raw API-0002/0003, not a data authorization
sandbox. Ordinary data-model reads never call scan.

```rust
pub struct ModelState { pub version: u32, pub definition: String, pub next_id: i64 }
pub struct ModelCatalog { pub state: Option<ModelState>, pub tables: Vec<String> }
pub enum SchemaMutation { CreateTable(String), DropTable(String), Write(Mutation), Validate { schema: Schema, version: u32 } }
pub struct SchemaPlan { pub expected: Option<ModelState>, pub next: ModelState, pub operations: Vec<SchemaMutation>, pub schema: Schema }
pub trait SchemaStorage: TypedTransactionalStorage {
    fn catalog(&self) -> impl Future<Output=Result<ModelCatalog,StorageError>> + Send;
    fn scan(&self, table: &str) -> impl Future<Output=Result<Vec<Row>,StorageError>> + Send;
    fn apply_schema_plan(&mut self, plan: SchemaPlan) -> impl Future<Output=Result<(),StorageError>> + Send;
}
```

The schema codec token sequence is: `hoyadb-model-1`, decimal version, table
count, then for each sorted table its name, column count, each sorted column's
name/kind/optional flag, relation count, and each sorted relation's name/tag and
operands. Kinds are String/Integer/Float/Boolean/Timestamp/Reference; optional
flags are 0/1. Tags are ToOne (target/key/optional), HasMany (target/key), and
ManyToMany (target/through/source relation/target relation). Every token uses
`<UTF-8 byte length>:<exact token>`; there are no other separators. The canonical
form includes normalized implicit FK declarations but not managed row fields.

Metadata is reserved logical table `__hoya_model`, exactly one row id `state`,
with exactly `format: Integer(1)`, `version: Integer(1..=u32::MAX)`,
`definition: String(canonical)`, `next_id: Integer(positive)`. Its definition
must decode and agree with version. No unknown fields/rows/reserved tables or
orphan physical catalog/row/field state is accepted. Physical SQLite's four
v1 DDL definitions, private marker and existing value tags remain unchanged.

catalog returns sorted logical application table names, excluding the supported
metadata table, and strictly validated state. A managed inventory must exactly
match its decoded definition. Every managed row's bookkeeping field must have
the exact stored application version; unknown reserved row fields reject.
An unversioned store with any reserved row field rejects as ambiguous ownership. scan returns complete owned raw
rows ordered by id; missing table is TableNotFound, bad physical payload is
BackendFailure. Neither operation creates metadata. A schema plan first checks
the complete expected state (including counter) inside its transaction, with
SchemaConflict on mismatch. Its next.definition must decode/project exactly to
plan.schema. Create-existing is InvalidSchema; drop-missing is TableNotFound.
Reserved table operations and arbitrary reserved row fields reject; the single
managed row-version field is permitted and checked by schema validation.
Validate checkpoints and final validation require exact application inventory
and complete typed rows, excluding only the known metadata table. Persist next
state and commit only after all checks. Ordinary ID writes use the existing
typed batch over an internal schema containing both application and metadata
tables, so row/counter publication remains atomic without scanning all data.

StorageError gains `MigrationFailure { message }` and `SchemaConflict { message }`.
Declaration/type/row/query/domain/backend failures retain existing variants.
SchemaConflict covers downgrade, same-version drift, unsupported/ambiguous
legacy ownership and compare-state failure. Malformed physical/canonical stored
metadata is BackendFailure. Authored migration structure/step/endpoint failures
are MigrationFailure. Diagnostics are unstable and never include raw payloads.

Rollback is reviewed revert plus a compatible reader, preserving data. Authored
migrations and missing-chain reset intentionally change/delete test-owned local
data under ADR-0009; no existing user database is opened by implementation or
verification. Release, real production migration, push and merge remain separate
operator actions.
