---
id: API-0012
title: JavaScript application binding for schemas records and commands
type: contract
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-28
updated: 2026-11-10
parents: [ARC-0002]
satisfies: [REQ-0001, REQ-0003, REQ-0007, REQ-0010, REQ-0011, REQ-0023, REQ-0046]
depends_on: [API-0011, API-0009, SPEC-0003, SPEC-0008, SPEC-0040, ADR-0034]
tags: [javascript, application-api, bindings]
---

# API-0012: JavaScript application binding

## Purpose and compatibility
The operator requests implementation of the application-facing JavaScript
examples originally described in the guides. API-0011 remains the advanced,
tagged, lossless boundary. This additive layer translates application values
and calls that boundary; it implements no storage/query/sync algorithm.
Existing `@hoyadb/client/host` exports and tagged DTO callers remain compatible.
An explicit `applicationSchema(wireSchema)` conversion can brand an existing
wire schema deliberately. Numeric version alone never selects a new API.
The browser root dispatches to the application layer only for a schema explicitly branded by the new
`table`/`column` helper path, and otherwise retains its
existing tagged behavior. `@hoyadb/client/node` supplies the same application API
with automatic ordinary WASM initialization. React Native exposes the same
builders and wraps its existing native owner for an application schema.

## Open and schema
`defineSchema({version:1,tables:{notes:table({columns:{body:column.string()}})}})`
produces an immutable wire-compatible schema. Column helpers are string,
integer, float, boolean, timestamp and reference, with `{optional:true}`.
Relations are `toOne(table,{foreignKey,optional})`,
`hasMany(table,{foreignKey})` and
`manyToMany(table,{through,sourceRelation,targetRelation})`; the through
relations remain explicit, as required by API-0004.

`defineSchema`/`applicationSchema` run a local pre-check that mirrors some core
schema rules (version range, reserved/dotted/empty names, to-one aliases sharing
a foreign key, explicit foreign-key columns, relation names shadowing columns)
only to build per-column value metadata before open. The core validator at open
remains authoritative: the pre-check must never refuse a schema core accepts,
and rules it does not mirror (relation targets, has-many inverses, through
tables, names inside raw relation DTOs) are refused by core at open. The
schema conformance suite runs one table of schemas through both and pins each
result, so a diverging core rule fails that suite. This is an interim
arrangement, not a second schema authority.

`await openDatabase({schema,engine:"memory",context,clock,idGenerator})` opens
real memory with automatic runtime bootstrap on browser/Node. A supplied runtime
remains a test/host capability. Memory is ephemeral unless a retained store is
explicitly supplied. Browser persistence uses `filename` (or an OPFS engine DTO)
and a caller-owned 32-byte `encryptionKey`. Absence of the key fails closed;
no plaintext fallback, key generation, store conversion or user-store operation.
Clock may return a Date, integer milliseconds, bigint or canonical decimal.
Context defaults to anonymous JSON null; it conveys no authorization guarantee.
Entity opt-in stays explicit: `entities:{scope,tables}` or
`await db.configureEntities(scope,tables)`; ordinary unconfigured writes stay local.

## Plain rows, queries and records
Application rows expose `{id, ...fields}`, without tags or `fields.value`.
Columns determine encoding; Reference accepts an ID string, Timestamp a Date
or signed milliseconds, Float a finite number (plain negative zero rejects),
Integer a safe integer or an explicitly opted-in bigint. Canonical integer
strings can be accepted only when they satisfy that column's selected range/type policy. Integer columns default to Number and reject unsafe values on both read/write.
`column.integer({bigint:true})` instead always uses bigint and requires bigint
support. Timestamp columns default to a fresh Date and reject unrepresentable
values; `column.timestamp({bigint:true})` always returns bigint milliseconds.
These policies are per-column host metadata, never extra wire-schema keys. Null remains null;
missing optional fields stay missing. Business JSON uses safe integral Number as Integer, nonintegral finite Number
as Float, bigint as Integer, and never guesses that a tag-shaped object is a
wire tag. Unsafe Number, negative zero, nonfinite values and untyped Date reject;
a returned typed record encodes its Timestamp columns explicitly. Untyped JSON
decode uses Number for safe integers, bigint for wider integers; TypeScript
declares this union. Reference columns decode as strings and re-encode by schema. No arithmetic/predicate engine is added.

`db.query(table).where(field,"=",plainValue).orderBy(field,"desc").limit(n)`
also accepts !=, <, <=, >, >= and in, plus API-0011 enum spellings.
`fetch()` returns plain snapshots. Direct to-one dotted paths use the declared
schema. `record.relationName` and `db.related(...)` return lazy queries.
`db.table(table).insert(fields)` returns a snapshot.
`db.table(table).findById(id)` returns an explicit record handle with asynchronous
`fetch()`, partial `update(fields)` and `remove()`; it does not pretend a read is
synchronous. Entity-configured updates call core patch; unconfigured and scoped
updates use existing core patch (outside a handler) or one admitted scoped
read/replace (inside a handler), never an unprotected read/merge/replace across
owner awaits. An interleaved owner write returns Busy rather than being lost. No method overwrites an
application field with the same name.

`query.observe(rows=>...)` returns an idempotent unsubscribe function immediately.
Its `.ready` Promise resolves when registration/early disposal completes,
or rejects with the actual typed registration error; calling unsubscribe before ready
suppresses all callbacks and retires the eventual token. Unsubscribe returns a
Promise. Initial snapshot arrives on a host turn, later changed fields emit
plain snapshots (column-aware observation). `observeCount` supplies the count.
Optional error callback and existing background sink report registration/delivery
errors; close/reset invalidate callbacks. No callback can reverse a commit.

## Definitions, binding and handlers
`defineCommand(name,{args,referencePaths?,handler})` returns an immutable reusable
definition. Argument shorthand includes string, integer, float, boolean, array,
object, any and reference (a string argument with a declared root reference path).
Nested references use explicit arrays of keys/indexes in `referencePaths`; the
binding never guesses that arbitrary text is an ID.
`const addOrder = db.command(definition)` explicitly binds a definition to one
owner; there is no ambient or most-recent database. Definitions need not be registered
at open: the core retains received outcomes and the application-wide reconcile
callback handles recovery after reopen, independently of local handler objects. `addOrder.execute(plainArgs)`
returns the optimistic handler value after atomic finish.
`executeWithReceipt` returns `{key,result}` for observing backend outcomes.

The local `async handler(db,args,ctx)` receives a tracked scoped view supporting
query/table writes and read-your-own-writes across awaits. It cannot execute a
nested command or reach the outer owner through that view. Throw or cancellation
aborts staging. `ctx.identity` is deeply read-only and `ctx.now()` returns a copy
of the captured Date. Created IDs retain core local: rules. Every field/value is
encoded before the existing atomic finish and response budget checks.

Backend handlers remain application code, not client callbacks run remotely.
`db.observeOutcome(key,callback)` and `rollback`, `cancel`, `forget` wrap existing
operations. A React-specific `useManageRollback` framework hook is not mandated
by the original illustrative example; document the actual outcome subscription.

## Transport, reconciliation and pull
Application `transport.push(envelopes)` receives plain decoded canonical
envelopes and returns correlated outcome objects. The adapter uses core codecs
in both directions; bigint never passes through JSON.parse/Number coercion.
A transport using JSON.stringify must handle bigint explicitly or reject it;
the library supplies no HTTP request or authorization implementation.

Application pending records additionally expose persisted `kind` and `created`
(ID-to-table map) through a read-only `reconciliation_context {key}` projection.
This reads the core recovery ledger, never a host cache. The projection accepts
only retained pending-outcome records. The tagged API-0011 pending DTO stays
unchanged. Entity-created final IDs receive identity mappings when omitted by
the application plan, using this persisted context; command mappings remain
explicit. Fill occurs immediately before the existing reconcile operation,
only for missing entity-created IDs; supplied mappings are never overridden.
The core validates the complete mapping keys and entity identity rules. On
reopen these facts are available before binding any handler.

Reconciliation remains explicit via `reconcile(pending)` returning the existing
plan with mappings/writes; no generic mapping is inferred from a result.id.
For the complete example, include every created-ID mapping in the backend's
business result and map it in the application reconciliation callback. This
convention changes no wire outcome/ledger format. Reopen recovery uses retained
core pending outcomes; no shadow queue or volatile-only mapping authority.
`db.sync()` runs an explicit push/reconciliation pass, then pull when supplied.
`startSync` also schedules pull immediately and at `pullIntervalMs` (positive,
default 30000 ms) IN THE APPLICATION LAYER ONLY, only when transport.pull and configured entity tables exist.
One application pull is in flight; timers/late callbacks are epoch-checked and
stopped by stopSync, close and reset. Busy on pull is skipped until the next
scheduled tick without an error report. Other failures use existing background
source sync; API-0011 advanced startSync remains byte-identical and push-only.
Busy/StalePull handling stays in the existing core/SDK pull session machinery.
In background sync, a RollbackConflict is reported once per key per epoch and
that key is skipped while later pending records continue. Explicit recovery
actions clear its skip; a new start/reset/close epoch clears the set. Awaited
sync still returns its failure. Close/reset cancel and await both push and pull
runs before disposing or resetting the owner.

Testing adds an application fake transport for plain handler values while
preserving the existing runtime-supplied tagged fake. It correlates command
keys and exact entity write identities, and never implements database logic.

## Approved command-conflict extension (SPEC-0040 / SPEC-0047)

Application open options inherit API-0011's optional `conflict` and
`onResolution`. Default Stop is unchanged. `setConflictPolicy` updates only the
active owner; reopening reapplies constructor configuration. `reconcileResolving`
accepts the same plain application plan as `reconcile`, including persisted
entity identity fill, and returns API-0011's immutable lossless `Resolution`.
`rollbackResolving` and explicit `discard` return the same report shape. Existing
unit methods preserve their return shape and use the shared resolving host path.
The core owns eligibility, closure refusals, guards and atomicity. Callback
failures never become sync failures or retire `onResolution`.

## Limits
All API-0011 limits, guards, admission, losslessness and protected defaults stay.
The original API-0012 slice excluded automatic Discarded policy; the optional
SPEC-0040 extension above is tracked by SPEC-0047. R9/R10/R11 implementation,
Kotlin/JVM execution, new dependencies, a transport server, release and existing
store conversion are not established by this application contract.
