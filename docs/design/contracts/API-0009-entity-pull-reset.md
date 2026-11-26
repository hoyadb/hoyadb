---
id: API-0009
title: Entity queue comparison state atomic pull and generation reset
type: api
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-11-10
parents: [ARC-0001, ARC-0002]
satisfies: [REQ-0023, REQ-0024, REQ-0025, REQ-0026, REQ-0027, REQ-0029, REQ-0044]
depends_on: [SPEC-0031, SPEC-0037, ADR-0033, API-0006, API-0007, API-0008, TC-0034]
tags: [entity-sync, entity-diff, storage, pull, experimental]
---

# API-0009: Additive entity sync owner and storage contract

Extends the unpublished command owner, same production adapters and experimental
host fixture. This contract takes precedence over older disclosed entity/pull
gaps; command-only command sync wire behavior remains unchanged. Data classification
is Confidential for domain values, identity, scope and checkpoints. Retention
is the existing bounded queue/history plus comparison state and progress until
explicit reset; no telemetry/export or backend authorization is introduced.

## 1. Entity configuration, mutation and correlation — E01–E06

`command::entity` defines owned `EntityConfig { scope: String, tables:
BTreeSet<String> }`, `EntityContext { identity: Json, now_ms: i64 }`,
`EntityOperation::{Create,Update,Delete}`, and `EntityDescriptor { table,
op, timestamp_ms, removed_fields: BTreeSet<String> }`.

`configure_entities(config, provider)` on CommandDatabase atomically opts in and
installs `FnMut() -> Result<EntityContext, CommandError> + Send + 'static`.
Scope is nonempty bounded opaque text; every table must be declared. Repeat open
must supply the same persisted scope/table set; mismatch rejects unchanged.
Adopting a command sync ledger with live recovery rejects. First adoption, ordinary
or durable, also rejects with Protocol and no change when a table being
configured already holds a row whose ID has the reserved `local:` prefix: with
no live recovery no producer can still map or admit it, so every later entity
write addressing that row would return UnknownLocalReference. The standalone
default generator allocates `local:<counter>` and is unchanged; an application
that will adopt entity sync must propose IDs or install an ID generator for
ordinary inserts into those tables, or remove such rows first. Seeded rows with
any other ID, and rows in unconfigured tables, remain accepted. The check is one
limit-1 identity-range query per configured table polled once by this
synchronous entry; an adapter whose read suspends keeps only the later per-write
rejection. A pristine post-reset shell
may be explicitly reconfigured, never implicitly by a mismatched ordinary open.
The explicit async `reconfigure_entities(config, provider)` requires generation
above one, pristine comparison/progress state, no retained queue/history records
and zero actual application rows under idle admission. Its normal SyncPlan keeps
model/header/counters/generation unchanged and has no row guards/mutations.
Storage adapters validate the complete empty inventory inside their candidate/
transaction. Only successful commit replaces provider/configuration and retires
an outstanding pull; failure preserves those and the original request. Thus an
unconfigured local write after reset cannot accidentally reclassify its data
under another scope. Reset again before reconfiguration if any data was added.
An opened owner with entity metadata requires its configured context provider
for any mutation affecting configured tables. Reads and nonconfigured writes
remain available. Context is captured once per mutation/batch and validated
before staged mutation; a provider error commits nothing.

Existing insert/replace/remove/apply_batch/link/unlink methods stage all entity sync
owner mutations under one admission permit and one SyncPlan. Add
`patch(table, id, fields) -> Row`: omit untouched fields, validate supplied cells
and preserve even same-value writes. Replacement additionally emits removed
optional fields as Null in `changes` and lists them in `removedFields`; Null
without that list means a present Null cell. Required/undeclared fields cannot
be removed. Create has no removals; delete has empty changes/removals. Missing
remove is a no-op without a diff. Command scopes and inbound/inverse paths
generate no additional diffs. Standalone Database and command sync-only owners retain
their existing behavior.

Every generated diff uses the same SyncRecord/order/capacity/retry machinery:
`SyncRecord.entity: Option<EntityDescriptor>` discriminates it; internal `args`
is exactly `{recordId: String, changes: Object}`. Reference cells and update/
delete local record IDs use explicit internal reference paths. Create excludes
its own identity from dependency paths. Entity-created row/wire IDs are exactly
`entity:<32-lowercase-hex-store-namespace>:<16-lowercase-hex-local-counter>`
with a positive counter allocated from ModelState.next_id. They remain stable
across retries and distinct across stores. They are not command `local:` IDs.
A known producer without mapping blocks declared references to either kind.
An identity-preserving entity outcome supplies `{entityId: entityId}` mapping;
this marks the producer resolved and permits references after history eviction.
Unknown `local:` references still reject; a stable entity ID does not require
retaining a retired producer. A different non-local canonical ID remains allowed.
Record mapping, not writeId deduplication, preserves natural entity idempotence.

Diff envelope exact keys: `table`, `op`, `recordId`, `changes`, `removedFields`,
`clientTimestamp`, `schemaVersion`, `context`, `writeId`. Its timestamp uses the
same validated UTC encoding as captured command time. Scalar values use the
existing schema types, exact JSON i64 and finite Float; Timestamp becomes i64,
Reference String. It contains no command/idempotencyKey/undo fields.
Responses require table/recordId/writeId and receipt or the existing outcome
status/result/reason. Validate the complete tuple against the frozen envelope,
including after an authoritative mapping. Unknown/ambiguous/foreign tuples
reject; partial arrays leave missing entries retryable. Queue status adds a
discriminated kind/table/record identity without payloads. Existing per-key
observation/cancel/inverse methods accept both write kinds' local tokens.

## 2. Comparison and resolver — E07–E10

`FieldStamp { timestamp_ms: i64, authoritative: bool }` and
`RowMetadata { fields: BTreeMap<String, FieldStamp>, tombstone:
Option<FieldStamp> }` retain stamps for absent/removed fields. `MetadataChange`
holds table/record_id and optional before/after metadata. Each SyncRecord gains
`metadata_undo: Vec<MetadataChange>`. Log it with row undo; guarded inverse
simulates both in reverse order before committing either. When that proof fails,
rollback (never cancel) of a processed rejected single entity Update may use the
same SPEC-0038 C4 fallback. An ineligible fallback keeps the original
RollbackConflict. Mapping rekeys both
current metadata and remaining metadata undo atomically. Reconciled/rolled-back
records release no-longer-needed metadata undo, preserving live dependencies.

`EntityChange { table, record_id, op, changes: BTreeMap<String, Value>,
removed_fields, timestamp_ms: i64, field_timestamps: BTreeMap<String,i64> }`
is validated against the normalized schema. Field timestamps name exactly the
changed fields; delete has none. All timestamps use the existing nonnegative
UTC-ms range. Unknown, reserved, wrong-type, incomplete missing-row updates,
invalid removals and oversized values reject before mutation.

`resolve_entity(schema, current: Option<&Row>, metadata: &RowMetadata,
incoming: &EntityChange, authoritative: bool, hook: Option<MergeHook>)`
returns `EntityResolution { row: Option<Row>, metadata: RowMetadata }`.
Default chooses newer per-field timestamps, preserves untouched cells, keeps
deletion precedence and rejects unresolved differing equal-time values with
AuthorityRequired. An authoritative backend-selected tie winner is accepted.
Default tombstones prevent resurrection regardless of later update/create time:
a losing update/create returns row=None and unchanged metadata successfully,
so an obsolete mutation cannot permanently block the rest of a pull page.
Current rows without stamps begin with comparison time zero.

`ReconcileWrite::Entity { change: EntityChange }` supplies an explicit backend-
authoritative partial correction, including the selected per-field timestamps or
Delete tombstone timestamp. It is an application-authored reconciliation plan,
not a new outbound write or a synthetic pull page. It requires an entity ledger
and may address any declared table. Entity-record reconciliation with nonempty
writes accepts only this typed variant; generic Put/Remove reject atomically
rather than inventing clocks. Existing command-origin Put/Remove behavior remains
unchanged; a command-origin plan may also choose typed Entity corrections.

Before any correction, prove the optimistic row and comparison-metadata closure
against current state, preserving independent edits on conflict. When that proof
fails, SPEC-0038 C4 permits one fallback for an existing-row single entity Update
closure whose plan holds only typed Update corrections of that row:
- changes confined to fields the closure does not own are preserved exactly;
- pending later entity writes on the row that provably still win are re-based;
- the fallback runs only after the unchanged SPEC-0039 validation and
  outside-closure refusals.
Its eligibility, baseline and recompute rules are normative there. An ineligible
fallback returns the original RollbackConflict with rows, metadata and ledger
unchanged. Then apply only
the supplied fields/removals and their exact backend-selected timestamps, marked
authoritative, regardless of the superseded optimistic client clocks. Preserve
untouched fields and their timestamps.

After typed corrections and ID rekey, promotion follows SPEC-0038 C1a, which is
value-aware:
- Only fields the accepted write declared (changes and removals) are candidates.
- A target cell is promoted to authoritative only when its paired cell value, or
  its absence, exactly equals the accepted after-image, AND it carries a present
  stamp with the same timestamp. The flag is ignored in the comparison. An
  explicit Null is not absence. A missing stamp is never synthesized.
- A declared Delete promotes only a matching tombstone where the row is absent.
- Explicitly corrected fields keep their selected stamps.
- Explicit correction clocks and cleared optimistic tombstones are never
  overwritten by that promotion.

On the SPEC-0038 C4 path, these correction and promotion steps are applied only
while building C4's starting image eff0, before the pending later writes are
recomputed. No second ordinary correction or promotion runs over the recomputed
row, its metadata or their images. Delete records its exact authoritative
tombstone. A typed correction cannot resurrect an existing authoritative
tombstone; correcting an optimistic tombstone requires a schema-complete valid
row and clears only that optimistic tombstone. Use the normal exact field/time/
value/schema/page bounds; invalid later corrections discard the entire plan.
No merge hook or client LWW decision replaces an already-selected backend result.

Mapping is applied atomically first. A correction uses the final identity;
a genuinely renamed old ID rejects, while an identity-preserving entity mapping
permits correction at that same surviving ID. Corrected values and comparison
metadata, accepted outcome, mappings and dependent rewrites commit together in
one normal SyncPlan/revision and generate no secondary diff. Pure receipt/
outcome without corrective writes retains the existing optimistic-stamp promotion.

`MergeHook` is a pure function pointer taking borrowed `MergeInput` (schema,
current row/metadata, incoming candidate and default result) and returning a
resolution or error. Hooks may customize unequal-time fields. Validate final
schema, identity, bounds and metadata, preserve unrelated fields and reject
attempts to choose unresolved equal-time winners or undo confirmed tombstones.
Hook output retains exactly the default metadata, row identity/presence and
unrelated/equal-time cells; it customizes only addressed unequal-time values.
Hooks run before storage transaction; no storage borrow crosses a host await.
Hook registration is application-owned per owner/open, not persisted code.

## 3. Exact additive ledger/storage format — E02/E04/E12/E17

`SyncLedger.entity: Option<EntityLedger>` is absent for the old exact format.
EntityLedger contains config, checked i64 generation/revision/pages/changes,
checkpoint Option<String>, ready bool and per-(table,id) metadata map. Its source
representation also carries `ledger_format: u8`, restricted to 2 or 3 by intrinsic
validation; other internal values cannot serialize. Generation
starts at 1 and other counters at zero. A pristine shell has no checkpoint,
readiness, counts or metadata; generation/revision do not make it nonpristine.

Preserve old format-1 state/command packets byte-for-byte when entity sync is absent.
entity sync uses exact format-2 additive state/descriptor/metadata packets inside
the same `__hoya_sync` logical table; physical SQLite DDL remains unchanged.
Each independent packet obeys Json limits, not one aggregate metadata blob.
Exact additional format-2 packet IDs and data shapes are:

- `entity:state`: `{config:{scope,tables:[sortedUniqueTable]},generation,revision,
  checkpoint,ready,pages,changes}`; counters are Json Integers, checkpoint Null
  or String. No metadata is aggregated here.
- `entity:record:<key>`: `{entity: Null|{table,op,timestamp_ms,
  removed_fields:[sortedUniqueField]},metadata_undo:[{table,record_id,before,after}]}`.
  Before/after are Null or RowMetadata. Only records with a descriptor or nonempty
  metadata undo have this packet. The base format-1 command packet is retained;
  its command String is empty exactly for an entity record. Command-only records
  retain their original nonempty versioned name and can omit the extension.
- `entity:metadata:<16-lowercase-hex-ordinal>`: `{table,record_id,metadata}` with
  contiguous zero-based ordinals in sorted (table,id) order. RowMetadata is
  `{fields:{column:{timestamp_ms,authoritative}},tombstone:Null|FieldStamp}`.
  Absent fields may retain comparison stamps; table/record identities are unique.

An extension requires entity:state, a record extension requires its base record,
entity descriptors require empty base command, and incomplete/duplicate/gapped
metadata ordinals reject. Old state/command packets remain exactly format 1;
only entity-prefixed packets use format 2.

SPEC-0037 R10 adds format 3 for a newly created entity ledger, including a
command-only store newly adopting entity sync. The outer entity packet `format`
remains 2; the exact `entity:state` data object adds `ledger_format: 3`. Absence of
that key means legacy format 2. Existing ledgers retain their format through
open, configuration, reconfiguration, both reset modes and rebinding; normal
and reset storage plans cannot implicitly change the marker.

Format-3 metadata IDs use `entity:metadata:` plus lowercase hex of the fixed
four-byte big-endian UTF-8 table-byte length, table bytes and record-id bytes.
The payload shape remains `{table,record_id,metadata}`. The decoder checks the
raw even-length suffix against `2 * (4 + Json MAX_BYTES)` before decoding,
then requires a valid length, UTF-8, existing table-name validity,
nonempty record id and exact identity/payload agreement. Comparing decoded
bytes directly to the payload strings requires no decoded allocation. Mixed ordinal/identity
formats reject. Format-3 `to_rows` sorts all packets by id; format-2 serialization
and ordinal suffix updates retain their previous exact bytes and order. This
representation change does not authorize an online conversion operation.

Unknown formats/prefixes/keys, duplicate packets and malformed/noncanonical data
reject without defaulting or deleting. Opt-in may upgrade old resolved history
but preserves every frozen command envelope. Validate all metadata against the
schema and forbid migration with nonpristine comparison/checkpoint state or
live recovery. A pristine shell is not itself a migration block.

SyncPlan adds explicit `SyncTransition::{Normal,Reset{discard_recovery:bool}}`.
Normal transitions preserve namespace/generation, monotonic identity counters
and frozen envelopes. Every visible data/comparison change increments revision
in the same transaction, including local-only mutations on a entity sync owner.
Reset validation requires a previously loaded exact ledger, next generation and
revision incremented once, no next records/metadata/checkpoint/progress, retained
scope/table configuration/options/namespace/sequence and unchanged next local-ID
counter. Preserve reset rejects actual live recovery. Guards/mutations must
cover every actual application row on reset; adapters validate that completeness
inside the same candidate/transaction before publication. No separate clear.

## 4. Pull, bootstrap and progress — E11–E15

`PullPage { checkpoint: String, complete: bool, changes: Vec<EntityChange> }`
has canonical response bound 1 MiB and at most 1000 changes. Complete body
decoding is strict, bounded and exact; no foreign Number coercion. A page is an
owned server-confirmed selection, so equal-time values carry backend authority.
Scope authorization remains a transport/backend responsibility.

`begin_pull()` returns owned `PullBatch { request: PullRequest, since:
Option<String>, scope: String, generation: i64 }`. PullRequest is an opaque
process-unique checked owner token; private state captures current checkpoint,
generation/revision. One outstanding pull per owner. `abort_pull(request)`
retires it without data effects. No permit is held across transport await.
`apply_pull(request,page)` acquires idle admission, validates the original
token/checkpoint/generation/revision and complete page, resolves against current
rows and commits data/comparison/checkpoint/progress atomically. Busy retains the
ticket; StalePull retires it and requires a new transport fetch, never rebinding
cached response bytes. Duplicate/cross-owner/consumed requests reject unchanged.

A change that would modify a live recovery-protected row OR its comparison
metadata returns PullConflict
for the whole page. Insertion into a table with unresolved local-ID producers
also conflicts, preventing lost-outcome mapping collisions. A Delete that would
record a first tombstone for an absent row counts as such an insertion: the
tombstone would occupy the identity a pending create may still be mapped to,
and reconciliation refuses an occupied comparison target. A Cancelled record
whose inverse completed (rolled back and reconciled, API-0006's live sent
tombstone) has no local row and can never be mapped: it is not an unresolved
producer for this insertion rule, although its own created identity and every
other protection below still apply until forget. Protection includes
all ledger.live_keys dependency closure members and their undo, metadata undo,
created IDs and mappings, not only records directly marked live. Resolver equality
alone does not prove a harmless protected no-op: optimistic field clocks or a
pending deletion can suppress a backend change which would be lost when local
recovery later restores an older row. Conservatively return PullConflict for
protected identity overlap even when the resolved row and metadata are equal.
The narrow exception is an existing authoritative tombstone: obsolete incoming
changes remain harmless no-ops under its permanent deletion precedence and must
not block the page. No optimistic rebase or rollback simulation is introduced.
Failed validation, conflict or storage commit preserves checkpoint/progress/data;
no success event.
The host may abort/refetch after resolving outbound work. Nonterminal pages must
advance checkpoint. `ready` becomes sticky on the first complete page until
reset; page/change counts accumulate checked commits, including empty pages.
Null checkpoint never causes implicit purge. Reopen resumes the last complete
page transaction. Subscription membership never changes a pull request.

`sync_progress()` returns redacted owned phase/readiness/page/change/tombstone
counts, with checkpoint only in begin_pull. `observe_progress()` uses existing
SyncSubscription and `CommandEvent::Progress`. Registration immediately queues
one owned snapshot; unsubscribe removes pending events. Failed pages emit none.

## 5. Reset and actual foreign consumers — E16–E20

`reset(ResetMode::{PreserveRecovery,DiscardRecovery})` requires an idle shared
coordinator. Preserve rejects live recovery; Discard explicitly allows removal
of all local recovery, never claiming reversal of server effects. No side effect
on Busy/storage failure. Success uses the reset SyncPlan and only then clears
data/status/outcome/progress subscriptions, outstanding pull handles/buffers,
pending owned frames and owner-generation runtime work. Retain schema/config/
options and all identity counters; new subscriptions see empty data and new
pull starts with null. A different physical store remains independent.

API-0008's C/WASM ABI and 1 MiB complete frame stay unchanged. Extend shared
fixture operations, existing JS/Swift reference hosts and real HTTP fixture.
All clocks/counters/handles in outer frames stay decimal Strings; domain body
and envelope strings remain opaque until Rust decoding. Pull page upload uses
at most 256 KiB UTF-8 chunks, checked sequential byte offsets, one bounded
buffer under the original pull token, total at most 1 MiB; finish parses/applies
once. No partial page event/commit. Abort/reset/close dispose buffers. Complete
frame preflight still applies to every request/reply, with no raised limit.

Host reset rejects queued dispatcher jobs and active handlers before core call;
application may explicitly cancel/abort then retry. Only successful reset changes
generation, drops pending deliveries and invalidates callbacks/sync pump/timers.
Late completions cannot clear or mutate a newer generation's pump. Busy received
pages retain their original token/body; StalePull forces a fresh transport call.
Actual Swift/browser tests must cover those lifetime rules, page restart and
large valid chunked upload, using production Rust/SQLite rather than host state.

Rollback: source revert with compatible readers; never rewrite or discard an
unknown ledger. Only synthetic task-owned files may be reset by verification.

## 6. Exact experimental host messages — E01–E20

The machine-readable HTTP fixture is [entity-sync-openapi.json](entity-sync-openapi.json).
It reflects this additive contract; API-0001's original OpenAPI remains historical.
Schema validation does not replace Rust's field/time/value and whole-page bounds.

These additive operations use API-0008's exact outer request/success/error frame,
owned decimal-string handles and existing tagged row/domain values. Existing
operations and command-only requests remain unchanged. The shared Rust fixture,
not foreign code, owns the production owner and all checked core tokens.

| Operation | Exact args | Success value |
| --- | --- | --- |
| entity_configure | scope:String, tables:[String] | Null |
| entity_batch | writes:existingWriteArray, identity:taggedNode, now_ms:String | existing ordered public changes |
| entity_patch | table:String, id:String, fields:taggedCellMap, identity:taggedNode, now_ms:String | existing application row |
| receive_entity | table:String, record_id:String, write_id:String, outcome:Null or existing tagged outcome | Null |
| begin_pull | empty | {request:String,since:String or Null,scope:String,generation:String} |
| pull_chunk | request:String, offset:String, chunk:String | next UTF-8 byte offset as String |
| apply_pull | request:String | Null |
| abort_pull | request:String | Null |
| sync_progress | empty | {phase:String,ready:Boolean,pages:String,changes:String,tombstones:String} |
| observe_progress | empty | subscription handle String |
| reset | mode:"preserve" or "discard" | Null |

`entity_configure` installs the real context provider through a private shared
context cell. Each entity_batch/entity_patch validates its whole request and
complete reply budget, places that operation's immutable context in the cell,
invokes exactly one immediate-ready production mutation, and clears the cell on
success or failure. The provider samples it once for that mutation/batch. There
is no separate context-setting RPC that can race another mutation. A legacy
direct_batch affecting configured tables has no context and rejects unchanged.
Entity batches use the production patch/enqueue path; foreign code never builds
a durable envelope. Generated-row reply budgets use the maximum namespaced
entity-ID width, rather than API-0008's shorter command-only default ID.

A reconcile-plan write may additionally be exactly
`{op:"entity",change:<the typed change object used by pull>}`. Rust decodes the
change through the shared PullPage codec; foreign wrappers do not decode its
numbers or synthesize comparison clocks. Put/Remove keep their API-0008 shapes.

Mixed receive_wire arrays accept either the unchanged command outcome keys or
the exact entity tuple table/recordId/writeId plus status/result or reason.
Validation and complete response-report preflight precede per-member receipt
application. An individual member error never rolls back independent receipts.
Prepared mixed envelopes remain opaque canonical Strings and are sent as the
same nonempty arrays, in production queue order. No foreign Number conversion
or replacement queue is introduced.
An entity-only receipt has exactly table/recordId/writeId/status, with status
"acknowledged" and no result/reason. The command-only receive_wire grammar
remains unchanged: its HTTP entries require applied/corrected/rejected outcomes;
separate command receipt delivery continues through the existing receive RPC.

The experimental HTTP pull response body is an opaque UTF-8 String with exact
shape `{checkpoint:String,complete:Boolean,changes:[change]}`. Each change is
exactly `{table:String,record_id:String,op:"create"|"update"|"delete",
changes:taggedCellMap,removed_fields:[String],timestamp_ms:String,
field_timestamps:{field:String}}`, where timestamp strings are decimal i64
milliseconds. The shared Rust boundary decodes this into the actual PullPage;
schema, metadata and authority validation remain in production code. Foreign
wrappers forward the raw response without parsing its domain values. This is
the isolated host fixture's explicit typed representation, not an assertion
that every integrating backend must use it.

Each fixture pull handle retains its actual PullRequest and one initially empty
upload buffer. pull_chunk accepts the exact next byte offset, at most 262144
UTF-8 bytes per chunk and at most 1048576 total bytes, with complete outer-frame
preflight; hosts split smaller when escaping would exceed the frame limit.
Chunks cannot commit data or progress. apply_pull strictly decodes the whole
bounded body once and calls production apply_pull. Busy keeps the original
ticket/body for retry; StalePull retires and clears it, requiring a fresh HTTP
request. Abort/close dispose the buffer. Malformed/oversized upload or failed
application cannot advance progress/checkpoint; the caller may abort the handle.

Progress uses existing paged drain delivery with kind "progress" and the exact
sync_progress projection as value. Registration, callback liveness, unsubscribe,
oversized-event retention and snapshot boundaries remain API-0008 behavior.
Reset first calls production reset; only success clears request/pull registries,
event/push/upload buffers and subscriptions. Hosts then advance generation,
invalidate callbacks and stop the old sync pump/timers. Failed/Busy reset keeps
those values usable. An old transport/mapper/handler completion cannot act on a
new generation or clear a replacement pump. A post-reset owner may explicitly
entity_configure again; ordinary mismatched configuration remains an error.

Before implementation, add compiling functional-stub RED for configuration,
generated patch, mixed receipts, paged progress, chunk rejection, guarded/default
and discard reset and stale post-reset replies. Then run the shared real-SQLite
fixture assertions, JS deterministic dispatcher tests, actual Swift and browser
handlers/push/pull/reload/reset/large-upload scenarios, plus platform validation–4 regressions.
The loopback-only stdlib HTTP fixture adds mixed entity outcomes and deterministic
two-page GET /hoyadb/changes responses; HTTP/timer/auth ownership stays foreign.

The additional test-only `entity_conformance(index:u32)` export and separate
worker `{op:"entity_conformance",index:Number}` control run these unchanged
shared functions: indices 0–2 are entity_sync::{automatic_entities,
admission_and_inverse,two_client_resolution}; 3–8 are entity_pull::{
pull_atomicity_bootstrap_and_reset,reset_inventory,lifecycle_comparison_guard,
pull_recovery_protection,progress_transition_guard,reset_modes_and_coordinator_guard}.
Indices 9–10 are entity_sync::{managed_links_and_mapped_rows,
reconfiguration_guards}; 11–12 are entity_pull::{typed_correction_clocks,
reconfigure_inventory}. Indices 0–8 keep their existing meanings. Require an
integer index 0–12 and no raw/reactive/command owner. Each fresh adapter
gets a unique phase-five-conformance-*.sqlite name. After all case owners close,
delete only its exact generated prefix and require identical pre/post named-file
inventory. Keep the existing SAH pool24, unchanged shared assertions, ordinary
60000ms per-case deadline and report actual runtime/cleanup. This test control
is outside the production lossless RPC and never substitutes a host sync engine.
