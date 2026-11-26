---
id: SPEC-0037
title: Exclusive store ownership, per-database admission and change-proportional command and sync work
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-26
updated: 2026-10-31
parents: [ARC-0001, ADR-0037, ADR-0038]
satisfies: [REQ-0011, REQ-0007, NFR-0001, NFR-0003]
depends_on: [RFD-0007, DEF-0034, DEF-0035, API-0002, API-0003, API-0005, API-0006, API-0007, API-0009, SPEC-0038]
tags: [purpose-audit, remediation, storage, ownership, admission, scaling]
---

# SPEC-0037: Store ownership, per-database admission and bounded command/sync work

## Need and verified root cause

The diagnosis below describes the remediation baseline 7058b29 and its recorded
working overlay, before the component fixes. It is historical evidence, not a
claim that every baseline behavior remains in the current source.

### DEF-0035: one database's command work blocks unrelated databases

`CommandDatabase::open` (`crates/hoyadb-core/src/command/owner.rs:127`)
registers every owner with `ExecutionCoordinator::global()`
(`command/execution.rs:53-61`), a single process or worker state.

`idle_permit` (`execution.rs:151-173`) returns Busy whenever any request is
active or queued, so a queued or active handler on database A makes database
B's writes Busy. Root's public-API counterexample shows exactly this:
`target/purpose-audit-20261026/availability.txt`.

A native persistent store has no owner exclusion either:
- `SqliteStorage::open_inner` (`crates/hoyadb-storage-sqlite/src/lib.rs:85-168`)
  takes no lease;
- the facade rejects a duplicate path only among its own owners;
- RISK-0003 records the cross-instance gap.

### DEF-0034: command and entity/sync work grow with the whole store

**Command staging.**
- `StagedScope::begin` (`execution.rs:216-243`) copies every row of every
  declared table into a `SnapshotStorage` and re-validates the whole inventory,
  for every command and every entity staging path
  (`entity_owner.rs:115,249,404,474`).
- Whole-store reads also occur at:
  - `pull.rs:357,399`;
  - `entity_owner.rs:81`;
  - `merge.rs:107,157`;
  - `command/sync.rs:147,166`.

**Ledger commits.** Every ledger commit:
- serializes the whole ledger (`owner.rs:519`; `records.rs:277-306`);
- compares the full prior and next row vectors (`storage/sync.rs:24-32,42-90`);
- in SQLite, runs the full catalog twice, loads and parses the ledger three
  times, and deletes and reinserts every sync row (`storage-sqlite/src/sync.rs:27-73`);
- in memory, re-parses five times and copies the whole store twice
  (`storage/snapshot/sync.rs:41-80`).

**Quadratic bootstrap.** Format-2 metadata packets carry contiguous sorted
ordinals (`records.rs:230-261,296-305`). An arbitrary-order insertion therefore
renumbers a suffix, and fixed-size-page bootstrap is quadratic in the worst
case.

**Memory engine.** It copies the whole store and its indexes for every write
batch (`snapshot/mod.rs:92`, `snapshot/typed.rs:185`).

**Ordered-limit queries and reactive.** Both sort every match:
- SQLite orders by a `CASE` expression (`storage-sqlite/src/query.rs:184-205`);
- memory sorts every candidate (`snapshot/typed.rs:133-160`);
- reactive re-sorts all keys on each update (`model/reactive.rs:122-143,612,641`).

### Operator decisions (RFD-0007)

- **I-A (ADR-0037):** per-database command serialization. The same logical
  database stays serialized; independent databases progress.
- **F-A (ADR-0038):** a new-ledger format and an offline converter. Existing
  format-2 stores keep format 2 on open, reset and rebind. No user store is
  converted.

## Scope

Three phases. Each phase is implemented only after its own case RED review.
Phase 1 is independent of Phases 2 and 3.

- **Phase 1:** R1 lease, R2 fault stimulus, R3 ownership capability and
  admission with global identity, R4 counters.
- **Phase 2:** R5 open validation and trust model, R6 overlay staging, R7
  memory touched-entry atomicity, R8 ordered-limit and reactive bounds.
- **Phase 3:** R9 delta sync commit, R10 ledger format 3, R11 offline
  converter tool.

**Out of scope:**
- multi-writer or cross-tab coordination;
- browser exclusivity claims;
- automatic conversion;
- SDK conversion APIs;
- numeric latency or throughput targets;
- cancellation of non-yielding host handlers;
- JDK/Android execution, which stays held.

## Behavior

### Phase 1

**R1. Exclusive native ownership lease.**

1. Native targets (`#[cfg(not(target_arch = "wasm32"))]`). In
   `SqliteStorage::open_inner`, at a fixed position: after keying
   (`lib.rs:103-110`, which executes only `PRAGMA` statements and reads no page),
   after `PRAGMA temp_store=MEMORY` (L111-113) and after
   `busy_timeout(Duration::ZERO)` (L114), and before the emptiness probe (L115):
   - execute `PRAGMA locking_mode=EXCLUSIVE`, then `BEGIN EXCLUSIVE`, then
     `COMMIT`;
   - `SQLITE_BUSY` or `SQLITE_LOCKED` from these statements returns the new unit
     variant `StorageError::StoreInUse`;
   - any other failure keeps its current mapping.

   Because no page is read before the lease:
   - a wrong-key open of an **owned** store returns `StoreInUse`;
   - a wrong-key open of an **unowned** store still fails at the first page read
     inside `BEGIN EXCLUSIVE` and stays redacted.
1a. Journal persistence in exclusive mode is disclosed. With the rollback
   journal in EXCLUSIVE locking mode, SQLite keeps the `-journal` file between
   transactions: its header is zeroed and it is not deleted. While a store is
   owned, a `-journal` sidecar may therefore exist. Requirements:
   - on protected stores it holds only SQLCipher-encrypted page images;
   - reopen treats a zeroed-header journal as not hot, and the data is intact;
   - whether it remains after close is recorded by SI-L08, not assumed;
   - it lives in the store's directory, and SPEC-0036's family-file rule already
     names `-journal`.
2. WASM sends none of these statements. Its open path is byte-for-byte
   unchanged, and no exclusivity is claimed (RISK-0003 retained).
3. `open_with_options` (`lib.rs:56-65`) returns `StoreInUse` unchanged. Every
   other protected-open error stays the redacted `protected storage open failed`.
4. The facade maps `StoreInUse` to the existing wire kind `Busy` (the
   `StorageError` table in `crates/hoyadb-bindings/src/lib.rs`). The per-Facade
   duplicate-path check is unchanged.
5. The lease lasts as long as the connection. It is released on drop, close or
   process termination. No lock or marker file is created.
6. External readers, including SQLite tools, cannot attach while the store is
   owned. This is a disclosed behavior change.

**R2. Fault stimulus that stays in the default suite, with no production
surface.**

1. Under R1, a second connection cannot hold a lock, so `sqlite_cases.rs` q05
   (L182-215) can no longer cause COMMIT failure that way.
2. **A real SQLite COMMIT failure is preserved.** q05's commit-failure
   assertions move into a `#[cfg(test)]` unit-test module inside
   `crates/hoyadb-storage-sqlite/src/lib.rs`, using a deferred foreign-key
   constraint on the store's private connection.
   - Prove the triggering statement succeeds and COMMIT fails with
     `SQLITE_CONSTRAINT_FOREIGNKEY`; an early statement abort or a manual
     ROLLBACK is not a substitute.
   - **Assertions kept from q05:**
     - each failed `replace` and `apply_batch` returns `BackendFailure`;
     - the complete changed row and the valid batch prefix are rolled back;
     - nothing is published;
     - the indexes are intact;
     - the next insert succeeds and is readable.
   - **Scope.** The stimulus exists only in `cfg(test)` code. No normal-build
     fault API exists. q05's lock-held write assertions become SI-L05.
3. **Other tests: preserve same-live-owner recovery.** SI-K02 identifies
   fixtures that arm a fault, observe it and retry on the same owner without
   closing its subscriptions or pull ticket.
   - A native-only, non-default `test-hooks` feature of
     `hoyadb-storage-sqlite` forwards `rusqlite/functions`. The pinned
     rusqlite declares `functions = []`, adding no dependency.
   - Only under this feature, register a doc-hidden SQL function
     `hoya_test_fault(name)` backed by a per-path armed-fault registry. Tests
     install their fixed triggers while closed, then arm/disarm live faults.
     Scope registry entries and cleanup to the fixture so parallel tests cannot
     affect another store.
   - Retain each original stimulus: real RAISE(ABORT) statement failures,
     deferred-FK COMMIT failures and in-transaction corruption. Keep CS07's
     statement-abort label; do not trade these properties for close/reopen.
   - A fixture holding SqliteStorage directly may also use a doc-hidden
     `execute_test_sql(&self, sql: &str)` method under the same native-only
     feature. This preserves a corrupt/read/restore/read test on one live
     connection without adding artificial writes or clearing its temporary
     operand state. The method is absent from normal builds and is not
     reachable through CommandDatabase, the facade or an SDK.
   - Native target-gated self dev-dependencies enable the feature in the
     default native suite. WASM dev builds do not enable it.
   - SI-W02 proves `test-hooks` and `rusqlite/functions` are absent from all
     production/no-dev feature graphs and shipped build scripts. There is no
     fault SQL function or registry control in a normal build.
4. Closed-store raw tamper and inspection tests keep their raw connections,
   unchanged.
5. Child-process kill fixtures publish their named readiness file only after
   its full mode bytes have been written and synced to an owned sibling
   temporary file, then renamed in the same directory. Existence must never
   expose an empty or partial mode. The parent keeps its exact-mode assertion,
   real child termination and encryption/recovery checks. SI-K03 deterministically
   observes the publisher between file creation and writing; it is a fixture
   correctness fix, not a change to database durability semantics.

**R3. Ownership capability, per-database admission and globally unique
identities.**

0. **The capability.** `SyncStorage` (`crates/hoyadb-core/src/storage/sync.rs:35`)
   gains the provided method
   `fn ownership(&self) -> StoreOwnership { StoreOwnership::Unproven }`, with
   `pub enum StoreOwnership { Unproven, Exclusive }`.
   - Native `SqliteStorage` returns `Exclusive` (R1 holds for its whole life).
   - Memory `SnapshotStorage` returns `Exclusive`: it is an owned value, and the
     facade leases a memory store handle to one owner (`hoyadb-bindings/src/lib.rs`
     `stores`).
   - WASM SQLite and every other adapter return `Unproven`. This is additive,
     with no required method.
   - **The promise.** `Exclusive` means that while the adapter value is owned, no
     writer exists through any process, connection or adapter instance. The
     trait documentation of `ownership()` and `StoreOwnership::Exclusive` states
     this. A custom adapter may return `Exclusive` only if it proves the
     equivalent of SI-L01 and SI-L03 (a second same-process open and a
     second-process open are rejected), or if it restricts construction so that
     no second instance can reach the same store.
1. **Coordinator selection.**
   `ExecutionCoordinator::for_database(ownership, target_is_wasm)` returns a fresh
   coordinator only when `ownership == Exclusive` and the target is not `wasm32`.
   Otherwise it returns `global()` (`execution.rs:53-61`).
   - This is the conservative fallback for unknown custom adapters and all WASM
     builds (ADR-0037), kept until a supported-facade duplicate-owner rejection
     test exists.
   - `CommandDatabase::open` (`owner.rs:127`) passes `storage.ownership()`.
2. Owner and request ids come from process-global checked allocators:
   - `static NEXT_OWNER: AtomicU64` and `static NEXT_REQUEST: AtomicU64`, using
     `fetch_update` with `checked_add`; exhaustion returns the existing errors;
   - `State` (`execution.rs:9-16`) no longer stores `next_owner` or
     `next_request`.

   An `ExecutionRequest` or `CommandRequest` (`owner.rs:64`) from any coordinator
   is therefore never valid in another, and an id is never reissued after an owner
   closes.

   The pinned `wasm32-unknown-unknown` target has `target_has_atomic="64"`
   (root-verified with `rustc --print cfg`). Core already uses
   `static AtomicU64` (`command/pull.rs:9`, `model/reactive.rs:58`), so no mutex
   allocator is needed. Actual WASM build verification is preserved as a later
   step.
3. Per-coordinator FIFO, admission, cancel, idle-permit and drop semantics
   (`execution.rs:111-205`) are unchanged.
4. Same-database guarantees are unchanged. Direct writes, entity mutation, pull,
   merge, reconcile and reset take that owner's idle permit, which is Busy during
   that database's queued or active work.
5. JavaScript host jobs and the pump are per Database instance. Every handler
   still waits for its real core request/begin admission; independent host
   pumps confer no permit and do not infer ownership from an engine name.
   Same-instance jobs remain FIFO, including cancellation and close cleanup.
   Reset is Busy while that instance has jobs. Other instances' jobs do not
   prevent its reset or sync attempt; a conservative global core may still
   return Busy to reset, as before. Background sync uses the existing
   resumeOwned Busy/release wait without an S failure or polling. Release
   notifications are wakeups only, including the existing native broker for
   cooperating runtimes. No invocation order is promised across instances;
   global fallback remains FIFO among real core tickets. This completes the
   operator-approved ADR-0037 host scheduling change without a new open reply.

**R4. Physical work counters (test instrumentation; off in production).**

- `hoyadb-core` gains a non-default feature `work-counters`, with forwarding
  features of the same name in `hoyadb-storage-memory`, `hoyadb-storage-sqlite`
  and `hoyadb-bindings`.
- Each crate's `[dev-dependencies]` self-enables it (for example
  `hoyadb-core = { path = ".", features = ["work-counters"] }`, and the
  dependency crates with the feature). So ordinary `cargo test --workspace`
  compiles and runs the counter cases, and production builds (the facade, native,
  web and Apple build scripts) never enable it. SI-W02 guards this.
- With the feature, `#[doc(hidden)] pub mod hoyadb_core::storage::counters`
  holds a thread-local `WorkCounters` of `Cell<u64>` fields, with `reset()` and
  `snapshot()`.
- Without it, every increment site compiles to nothing.

Counts are of **entries**, not calls:

| Counter | Incremented per |
| --- | --- |
| `rows_cloned` | `Row` value cloned by staging, snapshot, sync-plan, candidate or overlay code |
| `ledger_entries_cloned` | each command record or metadata entry copied, including nested map and ledger clones |
| `metadata_entries_visited` | each metadata entry examined for validation, encoding, rank, progress or planning |
| `packets_encoded` / `packets_parsed` | ledger packet built by `records.rs` / decoded by `from_rows` |
| `rows_validated` / `inventory_rows_validated` | row checked by `validate_row` / by `validate_model_inventory` |
| `memory_rows_visited` | memory row examined by select, scan, catalog or index walk |
| `ordered_ops` | B-tree insert/remove/seek in the reactive ordered set or memory index walk (each O(log n); bound stated per case) |
| `sqlite_rows_read` / `sqlite_rows_written` | adapter row read by `get` or `scan` / row or field written |
| `sqlite_fullscan_steps`, `sqlite_sort`, `sqlite_vm_steps` | summed after each adapter statement from `Statement::get_status(StatementStatus::{FullscanStep, Sort, VmStep})` (pinned rusqlite 661107f) |

Plans are deterministic: the adapter never runs `ANALYZE`, and no `sqlite_stat`
table exists. Before Phase-3 optimization, SI-W03 completes the currently unused
increment sites without changing planner behavior. First prove each named
counter is nonzero on a known exercised path, and that measured full metadata
traversals grow with M. Only then record authoritative SI-Y performance REDs.
Manual Clone implementations for SyncRecord and RowMetadata use exhaustive
field destructuring without `..`, and fully populated clone-equality tests, so
adding a field cannot silently omit it from the counted clone.

### Phase 2 (gated on Phase-2 review)

**R5. Full validation at open and the trust model.**

1. Uses R3.0's `ownership()` (introduced in Phase 1).
2. Every `CommandDatabase::open` (`owner.rs:127-161`), for `Exclusive` and
   `Unproven` adapters alike (RFD-0007: full content validation at
   command-capable open), validates once, after `Database` open:
   - `validate_model_inventory` over every declared table;
   - the existing catalog;
   - the existing full `load_sync`.

   Any violation fails open. This is stronger than today, where an untouched
   corrupt row passes open and fails every command. It costs O(N) rows once per
   open, disclosed. Plain `Database::open` is unchanged and remains the read and
   export path for stores that fail this check.
3. Trust model T1, **only when `ownership()` is `Exclusive`**: every change to
   the store between opens is made and row-validated by its owner, so whole-store
   checks occur at:
   - open (2);
   - schema migration and schema plans (unchanged);
   - reset;
   - conversion (R11).

   Per command, commit or page, only touched rows are validated.
4. `Unproven` adapters keep today's per-command behavior **in addition to** the
   strict open of (2):
   - `begin` snapshots and validates the full inventory into a private copy;
     the common scope overlay runs over that immutable copy, and no scope base
     read reaches the live store. The Unproven base never receives a
     ValidatedQuery proof or the R8 fast path;
   - full `SyncPlan` commits.

   This is the conservative path; no performance claim is made for it.

**R6. Delta-overlay staging (borrowed live base for ready Exclusive adapters).**

1. **State.** `StagedScope` holds:
   - `delta: BTreeMap<(String,String), Option<Row>>`;
   - `first_images: BTreeMap<(String,String), Option<Row>>`;
   - the journal;
   - model state;
   - the generator handle;
   - shared schema and projection.

   The borrowed live base requires both R5's validated Exclusive owner and the
   provided `SyncStorage::reads_ready(&self) -> bool` capability, whose default
   is false. It promises first-poll completion of every `get`, `scan`, `query`,
   `query_validated`, `project` and `catalog` future. This promise is independent
   of Exclusive ownership. Native SQLite and SnapshotStorage return true;
   facade Engine forwards it. WASM may claim readiness only if its reads are
   immediate, and Unproven ownership still prevents a live-store base.

   Only that conjunction makes `begin` copy and validate nothing. Every other
   owner, including a yielding Exclusive adapter, awaits the full private
   snapshot and per-begin validation of R5.4, then runs the common overlay over
   that immutable immediate-read copy. No scope base read reaches its live
   store and the private base never receives a ValidatedQuery proof. No staging
   performance claim applies to the fallback. Storage futures remain allowed
   to suspend under API-0002; no adapter gains a Sync requirement.

   If snapshot acquisition is dropped while Pending, RAII releases admission,
   no active scope is published, and rows, model and ledger remain unchanged.
   A readiness promise violated by Pending produces BackendFailure on that
   operation, with no retry or busy-poll. Every primitive completes its base
   reads before changing its delta/first image; if a later primitive in a batch
   fails, undo restores the whole batch's prior delta and first images. The
   failed operation leaves the scope usable. SI-O06 through SI-O08 pin this.
2. **Borrowing.** The scope methods (`owner.rs:276-380`, `proposed.rs:41-128`)
   and entity staging (`entity_owner.rs:115,249,404,474`) operate on a temporary
   `Database<Overlay<'_, S>>`. It borrows the base storage and the scope delta as
   disjoint fields; no borrow crosses a host await.
3. **Overlay `get`:** the delta, else the base (recording the first image).
   **Writes:** delta only, with existence errors computed through `get`.
4. **Overlay query or project** for table T with predicates P, order O and
   limit k:
   1. D is T's delta ids.
   2. R is the base ids of T whose single-hop ToOne key in P or O targets any
      delta key of the target table. This includes in-scope inserts that
      resolve a previously dangling reference, and deletes that leave one
      dangling. Use one base `IN` query per path; the dependency logic is
      factored unchanged from `reactive.rs:546-603`. D∪R is a set: an id
      affected through two paths counts once.
   3. **Window.** `limit: Some(0)` returns an empty result without a base
      query. `limit: None` runs the base query unbounded, so the read visits
      exactly its own results. Otherwise B is the base query with P, O and
      limit `k.saturating_add(|D∪R|)`, minus D∪R.
   4. C is P and O evaluated by the existing memory engine over the overlay
      images of D∪R and their delta or base targets.
   5. **Merge.** Adapter order is not trusted. The result re-sorts B∪C (at most
      `k + |D∪R|` rows) with the Rust API-0003 comparator, including the `id`
      tie-break and the existing Null and wrong-kind normalization, and
      truncates to k. B's top `k + |D∪R|` contains the Rust top k only if
      adapter order equals the Rust comparator for every value kind; SI-Q04
      pins that equality, including `-0.0` versus `0.0` ties.
   6. **Count and project (future scope API only).** This slice exposes row
      queries in a scope, with no scope count observation or projection API.
      SI-O02's count assertion runs after commit through existing reactive
      queries. The following formula constrains a future scope count/project
      API; it is not an implemented Phase-2 path. A count observation is the base count with P,
      minus the base matches among D∪R, plus the overlay matches among D∪R. The
      subtraction evaluates P on the base images of D∪R with their **base**
      target rows for relation paths. The addition evaluates P on the overlay
      images with their **overlay** (delta or base) target rows. A projection follows steps 1-5 with the
      projected fields.

   Exactness follows from `resolve_field` admitting single-hop ToOne paths only
   (`storage/typed.rs:202-215`). That bounds the path depth, not the number of
   affected referencing rows, which is |R|. Exactness also relies on base
   immutability during a scope (R3 same-owner Busy plus the R3.0 `Exclusive`
   promise). An Unproven owner uses the same overlay algorithm only over its
   fully validated private snapshot, with no live-store scope base reads and no
   R8 proof. SI-O02 pins that twin to the original full-snapshot oracle.
5. **Safety.** During a scope, every other same-owner write path is Busy (R3),
   and no other writer exists (R5.3). The overlay therefore equals the full
   snapshot observationally. `prepare` (`execution.rs:262-293`) is unchanged;
   its guards come from `first_images`.
6. **Other whole-store reads under Exclusive:**
   - `entity_owner.rs:81` does one `limit 1` query per table;
   - `merge.rs:107,157` compares the ledger revision and model captured at
     prepare, plus the exact images of the page's touched rows;
   - reconcile and inverse row-read bounds are implemented in Phase 3 R9,
     after the SPEC-0038 correction landed in a87d0a3. SI-Y08 pins closure,
     plan-write, guard, mapping-target and reference-rewrite rows; the deferral
     is not an exemption from the final bounds.
   Unproven merge prepare and stale checking retain the full snapshot
   comparison, so another writer's change to an untouched row still gives
   StalePull. Touched-only merge comparison is an Exclusive optimization.
7. **Phase 2 measures staging, not the commit.** Until R9, SQLite
   `apply_sync_plan` still runs the full catalog twice (`sqlite/src/sync.rs:40,80`),
   loads the stored ledger (`sync.rs:43`), and `validate_sync_plan` parses the
   prior and next ledgers. Phase 2 bounds are therefore measured from `begin`
   through handler completion. The commit path's bounds belong to Phase 3. Clone
   bounds are derived from named clone sites: one per fetch result row, per first
   image and per delta image.

**R7. Memory touched-entry atomicity without whole-store copies.**

1. **The guard.** `apply_batch`, `apply_typed_batch`, `apply_sync_plan` and
   `apply_sync_delta` (`snapshot/mod.rs:89-111`, `typed.rs:179-247`,
   `snapshot/sync.rs:41-80`) mutate in place under an `UndoGuard`. It holds
   `&mut SnapshotStorage` and a `Vec<Undo>` of
   `Row{table,id,before:Option<Row>,after:Option<Row>}` and
   `Table{name,existed}`.
2. **Recording and restoring.** Before each primitive, both the prior image and
   the attempted image are pushed. Primitives keep the map-first order and
   may fail partway through the following index updates, so restoring an entry
   is idempotent and covers both images:
   - remove the index entries of the current map value and of `after`, for
     every field;
   - set the map entry to `before`;
   - re-add `before`'s index entries.

   So a primitive interrupted at any point is also undone.
3. **Commit, error and unwinding.** `commit()` disarms the guard. An error, and
   any unwinding panic, run `Drop`, which replays the log in reverse. It uses no
   `unsafe` (workspace `unsafe_code = "forbid"`).
   Primitive order stays map-first. Pin the reachable panic windows for insert,
   replace and remove: after the map change; within index_remove; between a
   replacement's index_remove and index_insert; within index_insert; table
   creation; and between consecutive batch primitives. The old proposed window
   "index removal before map write" does not exist and is not fabricated by
   reordering production. Partial-index hooks are test-only and absent in normal
   builds. Every case verifies rows, all index entries and sync packets.
4. **Residuals, explicitly not equivalent to today's copy:**
   - a panic **during** replay aborts the process (double panic);
   - unlike the old swap, `Drop`-time restoration runs while the store is
     mutably borrowed, and is never observable by other code;
   - a process abort leaves no memory state, as before.
5. `apply_schema_plan` (`snapshot/lifecycle.rs:73+`) keeps its candidate copy.
   It is a whole-store operation, disclosed.

**R8. Ordered limit and reactive bounds.**

0. **Validated context only.** The adapter fast paths R8.1-R8.2 apply only when all three hold:
   `ownership()` is `Exclusive`, S is owned by a `CommandDatabase`, and that
   owner's R5 open validation succeeded. This is recorded as internal state,
   never a public flag. While it owns S by value, no raw unchecked write path
   exists and every write is row-validated, so non-optional columns hold present,
   correctly kinded values. An `Unproven` store may change after open by another
   writer, so it keeps `CASE` and the full sort even inside a validated
   `CommandDatabase`, as do plain `Database` and raw `TypedStorage`. At that raw seam the equivalence is false:
   TypedStorage accepts unchecked rows and supplied schemas (SPEC-0025), and a
   wrong-kind payload sorts as Null through `CASE` (`query.rs:184-196`) and
   memory `field()`. For example, a raw `Float(100)` in a non-optional Integer
   column would win a DESC limit-1 query without `CASE`, where today the valid
   row is returned.
1. **SQLite bounded path.** Eligibility additionally requires a finite limit k,
   no predicates, no relation/navigation source, and a direct non-optional order
   column. k=0 returns empty without querying storage. The existing id-order
   path remains unchanged. All ineligible shapes use today's query, including
   R6 overlay base queries that carry predicates; no O(k) claim covers them.
   - Fix table, field name and declared kind as bound parameters. Select the
     existing numeric index payload for Integer, Timestamp, Boolean and Float,
     or the text payload for String and Reference. No new index or stored byte
     change is introduced.
   - ASC reads ids in payload ASC, row_id ASC order with LIMIT k.
   - DESC repeatedly selects one payload with ORDER BY payload DESC LIMIT 1,
     adding payload < previous after the first value. For each value, read ids
     with payload = value ORDER BY row_id ASC LIMIT remaining. Never fetch an
     entire tie group and slice it afterwards. This preserves the required
     ascending id tie-break without a partial sort or a min/max optimization
     assumption.
   - Fetch the final ids through at most k existing exact-id gets, then perform
     the usual public-row validation. The walk reads no full rows.
   - For d distinct values visited, at most d+1 next-value queries and d
     equality-range queries occur; returned ids and full-row gets are each at
     most k. These are logical work bounds; each B-tree seek is O(log N).
     Physical Sort and FullscanStep are zero. VmStep counts VDBE opcodes,
     not internal B-tree comparisons; fixed k, d and schema fixtures must have
     equal bounded VmStep and row-read counts across N on the pinned engine.
     If equality fails, identify the statement and cause; do not loosen it
     because tree depth changed. System SQLite EXPLAIN is exploratory only.
   - The adapter seam is a provided TypedStorage::query_validated(proof) method
     whose default delegates to today's query. Its only argument is an opaque
     ValidatedQuery borrowing the schema and query. Private fields, a core-only
     constructor, public read-only getters and no Clone/Copy/Default prevent raw
     callers from asserting the validated context. Only the gated Database
     fetch and overlay base-window paths construct it, at exactly two core
     sites behind the same Exclusive plus successful strict-open gate. No public
     API returns it. Existing adapters gain
     no required method. No projection seam is added in this slice.
2. **Memory.** In the validated context, a finite filter-free query with
   `order: None` walks the first k ids ascending (the default order). Otherwise
   the same eligible direct non-optional column case walks
   `indexes[table][field]` in value order (reversed for Desc, ids ascending
   within a value) and stops at k. SQLite's existing id path is unchanged.
3. **Reactive representation.** Every reactive group uses an ordered
   `BTreeSet<(OrderKey, String)>` over its existing projected keys. This does not
   rely on storage validity or adapter order and therefore is not gated by R8.0.
   The key uses exactly today's comparator: Null normalization, value-direction
   reversal only, ascending-id tie-break, and canonical finite-float order.
   - Keep each id's current key and remove that stored key on update, never a
     key recomputed from the replacement row.
   - Existing ToOne affected-id logic recomputes and reinserts referencing
     rows' keys when a target changes.
   - visible takes k; only affected ids are removed/reinserted. Registration
     still loads the full match set, which the exact limit window requires.
     Per-group memory is O(matches).
   - SI-Q04 compares visible with the old full-sort oracle after every update,
     including raw/plain/Unproven setups. No second sort-per-update production
     path is retained merely to duplicate the same comparator.
4. For the adapter paths R8.1-R8.2, optional and relation-path orders keep
   their full sort (Null and wrong-kind normalization), disclosed. Reactive
   groups use R8.3 for these shapes too, with identical projected keys.

### Phase 3 (gated on Phase-3 review; after SPEC-0038 lands)

**R9. Delta sync commit with a conservative default.**

1. **`SyncDelta`.** Add an opaque owned proof in `storage/sync.rs`, constructed
   only inside core from an already-validated ledger transition. Its fields are
   private; it has read-only getters and no public constructor, Clone, Copy or
   Default. A public consuming `into_parts` returns owned `SyncDeltaParts` so
   adapters can move rows without copying; there is no conversion back into a
   proof. No public owner method returns a proof. Its payload contains:
   - `expected_model` and `next_model`;
   - `expected_packets: Vec<(String, Option<Row>)>`;
   - `upserts: Vec<Row>` and `deletes: Vec<String>`;
   - `guards` and `mutations`.
   The expected packet identities are exactly the union of changed packet ids
   and `state` and `entity:state`, each with its exact prior image (None when
   absent). Reject duplicates within each list, overlapping upserts/deletes,
   missing expectations and surplus packet expectations. Expected identities
   necessarily overlap the writes they protect. Construct the proof at exactly
   one core persist site, under validated_exclusive; add the static proof-site
   check to reject any other construction. The facade Engine forwards
   apply_sync_delta with a forwarding test. WASM SQLite remains Unproven and
   uses the conservative default/full-plan path.
2. **Default.** `SyncStorage::apply_sync_delta(delta)` is a **provided** method:
   load the full prior sync state, verify every expected packet, apply the delta
   and call `apply_sync_plan` with that full prior inventory as `expected_sync`.
   This retains full validation and CAS for custom/unknown adapters: a change
   to any packet between that load and `apply_sync_plan` is rejected. Malformed
   untouched state already present at the load is rejected by full validation;
   a well-formed untouched edit predating the load becomes part of its prior
   snapshot and is not claimed to be detected. No required method is added.
   External raw callers retain `apply_sync_plan`, not a proof constructor.
3. **Exclusive overrides.** Native SQLite and memory consume a core proof only
   under the Exclusive contract. In one transaction (SQLite) or one `UndoGuard`
   (memory), validate the bounded delta shape, compare model and expected packet
   images, validate guards, then apply mutations/model/packet changes. Preserve:
   - namespace and monotonic sequence/last_ms header invariants;
   - frozen bytes and envelope identity of every changed existing frozen record;
   - model and entity transition rules, metadata validity, capacity/history and
     record legality, checked by core before proof construction;
   - all row guard rules, including explicit read-only SPEC-0039 expectations.
   Scope the storage catalog to mutated application identities and upserted
   packets by primary key: registered table, no orphan fields, correct row
   version and packet schema. No whole-store catalog or ledger reload is hidden
   in this override. Strict open still runs the full catalog and orphan EXISTS
   scan; that is the base of R5's induction. An out-of-band forged untouched
   metadata packet is not detected by the Exclusive delta commit, by design,
   but is rejected at the next strict open. The default/Unproven path rejects
   it at commit. SI-Y03 characterizes all three, rather than claiming protection
   outside the Exclusive promise. Reset and entity reconfiguration retain their
   full inventory plans and checks and are outside SI-Y01/Y07 equal-at-N/M claims.
   A delta cannot change the entity configuration.
4. **Core.** Under Exclusive, persist derives upserts/deletes from touched ledger
   keys. `persisted_rows` is a `BTreeMap<String,Row>` updated only after successful
   storage apply. A planner must not deep-copy untouched entity metadata: use a
   touched-entry overlay or undo log and restore every staged ledger/model/data
   change on any planner or commit error. Bounded record validation may still
   depend on capacity plus history; unrelated metadata work must not grow with M.
   Apply this to command enqueue/finish, push, receive, reconcile, inverse,
   entity patch, pull and merge, not only packet serialization in persist.
   `ledger_entries_cloned` counts each nested record and metadata entry wherever
   copied, rather than counting calls to ledger.clone. No uncounted clone can
   satisfy SI-Y01. Unproven keeps its full `SyncPlan` and validation path.
   Use a LedgerTxn with Deref but no DerefMut: every mutation records the first
   image through an explicit method, and drop restores on error, panic or
   cancellation before persist. changed() compares touched entries only. Full
   record checks stay bounded by capacity/history; touched metadata validation
   relies on strict-open validity plus validated owner-only transitions.
   Converted planners mutate through that transaction in place rather than
   returning a deep-copied SyncLedger whose full diff is scanned afterwards.
   Exclusive skips the intrinsic full metadata walk, retaining bounded record
   checks and touched metadata checks. Corruption of untouched in-memory
   metadata outside the validated-owner promise is detected at the next strict
   open, not by a promised full walk on every Exclusive mutation. Unproven
   retains the complete intrinsic metadata validation at its existing points.
   A required prior-versus-staged read audit covers every converted planner,
   especially reconcile's closure walk, promote_metadata, inverse's reverse
   simulation and plan_push eligibility (computed before any record mutation).
   Pull/merge sessions keep touched overlays, not a borrowed guard across calls.
   Under validated Exclusive, an owner-private ledger_epoch replaces full-ledger
   session comparisons. Bump it only after successful storage commit, including
   reset, never on failed persist. Unproven retains full expected/persisted
   comparisons and the SI-O05b full-snapshot stale check.
   Reset's full-plan success advances the same epoch and invalidates all prior
   pull/merge sessions even though reset replaces delivery and persisted rows.
   Cache the tombstone total once at open and update it from touched deltas;
   progress reporting must not scan M on each commit. A test-only invariant
   recomputes the total after commits and compares it with the cache.
5. **Pull planner.** Read the page's application rows only and stage row and
   metadata deltas. A midway planner failure restores the owner's complete prior
   ledger and publishes nothing; subsequent operations remain usable.
6. **Format-2 bound.** Insertion at ordinal i upserts packets i..M and the tail;
   append writes only new packets, with O(log M) rank work. Only the required
   middle-insertion suffix visits are exempt from equal-at-M bounds; every
   visit is counted. The format-3 equal-at-M guarantee is not claimed for
   format-2 middle insertion.
7. **Bounded reconcile and inverse rows.** Under validated Exclusive, build the
   snapshot from closure undo identities, plan writes, explicit guards, mapping
   target identities (including occupancy), and every row whose declared
   Reference columns contain a mapped local id. Find those referencing rows
   with indexed fk IN queries over each declared Reference column. Every row
   the plan can change must be in the snapshot so apply_planned_with guards
   and writes its diff. SI-Y08 compares outside-closure reference rewrites with
   the full-snapshot Unproven twin. Unproven keeps full snapshot().

Implementation order is counter completion, R9 core transaction/session overlay,
R9 adapter overrides, R10, then R11. Pin SI-Y06 on unchanged planners before
converting them. All global-owner fixtures follow SPEC-0039 GW18 isolation.

**R10. Ledger format 3 for new entity ledgers.**

1. **Marker.** In format 3, `entity:state` data (`entity.rs:603-622`) carries
   `"ledger_format":3`. `header_from_json` (`entity.rs:624+`) accepts exactly the
   format-2 key set or the format-3 key set. Mixed formats reject.
2. **Metadata packet id:** `entity:metadata:` followed by lowercase hex of
   `u32_be(len(table_utf8)) ‖ table_utf8 ‖ record_id_utf8`, with data
   `{table,record_id,metadata}`.
   - Decode rejects:
     - uppercase or odd-length hex;
     - a length overrun;
     - invalid UTF-8;
     - data that does not re-encode to the id;
     - ordinal packets in a format-3 ledger.
   - The encoding is injective and needs no hash. Before hex decoding or
     allocating, require an even suffix length at most
     `2 * (4 + json::MAX_BYTES)`, using checked arithmetic. The metadata data
     packet already bounds the combined UTF-8 table/id bytes by MAX_BYTES.
     Require at least four decoded bytes, table length within the remaining
     bytes, existing check_name validity, nonempty id and valid UTF-8. The fixed
     four-byte big-endian prefix includes leading zero bytes for short names;
     those are canonical, not an alternate encoding.
   - to_rows orders packets by id. No format-3 traversal depends on old ordinal
     order. Canonical round trips, history trimming and local-reference checks
     retain their exact semantics.
3. **Selection.** Format 3 is used only when entity configuration creates an
   entity ledger where no `entity:state` exists, including a command-only store
   newly adopting entity sync. An existing format-2 `entity:state` keeps
   format 2 on every open, reset (`pull.rs:399`) and rebind. Nothing converts
   implicitly. Entity comparison state cannot disappear (the existing
   validate_entity_transition rule). Both reset modes, reconfigure and rebind
   retain the previous format marker even in an empty comparison-state shell.
   `validate_entity_transition` enforces this retention for both Normal and
   Reset plans; raw `apply_sync_plan` callers cannot implicitly change the marker.

**R11. Offline converter tool.**

1. **Binary.** `crates/hoyadb-storage-sqlite/src/bin/hoyadb-convert-ledger.rs`:
   `hoyadb-convert-ledger --store <canonical file> --to <v2|v3> (--key-file <path> | --unprotected) (--dry-run | --backup <new path>)`.
2. **Behavior:**
   1. Precheck no-follow metadata: require an existing nonempty regular file
      at the canonical store path. Open READ_ONLY, without CREATE or URI mode.
      Key as the first SQL, set busy_timeout(0), locking_mode=EXCLUSIVE, BEGIN,
      then read sqlite_schema to acquire a retained SHARED read lease. Validate
      the exact internal storage format/version, recorded MODEL_TABLE state,
      and the full ledger through load_sync, SyncLedger::from_rows and
      validate_sync_model. The tool has no application schema: it does not
      validate application rows against one and leaves their stored bytes
      unchanged. No schema initialization or index installation occurs.
      The read-only pager refuses a hot journal with READONLY_ROLLBACK rather
      than recovering it. A persisted WAL-mode source is unsupported: the
      read-only EXCLUSIVE-mode first read must refuse before opening a WAL or
      creating shared memory, including for an encrypted header and when no
      sidecars currently exist. Existing WAL/SHM bytes remain unchanged.
      Pin this behavior on the target engine with SI-X08; if it cannot be
      proved, keep conversion gated rather than broadening the no-write claim.
      First-read Busy maps to exit 3; protected NOTADB maps to redacted exit 5;
      READONLY_ROLLBACK and other first-read failures, including unsupported
      WAL admission, map to exit 2 (unsupported or invalid source).
      The operator-approved exception (2026-10-31) classifies a protected
      IOERR_LOCK refusal on that same retained read-only connection with
      `PRAGMA cipher_integrity_check`, after successful key setup under the
      existing compatibility-4/HMAC-enabled profile. One or more failure rows
      means redacted protected-open failure 5; zero rows means unsupported
      source 2, never permission to admit WAL or convert. SQL success alone is
      not an authentication result. A pragma/tool error fails closed with
      redacted protected-open failure 5. Unprotected sources skip this check.
      Hot-journal refusal retains exit 2 and lock contention retains exit 3.
      Wrong-key and corrupt protected main files need not be distinguished.
      This check reads the complete main file and may materialize one error
      string per failing page during statement preparation: both work and memory
      can grow with file size. The operator accepted that cost; there is no
      bounded first-page or early-stop claim, and no result text is printed.
   2. Dry-run prints `{from,to,metadata_entries,packets_rewritten}` and closes
      this read connection. Main-file bytes and existing sidecars are unchanged;
      `from` and `to` are numeric 2 or 3; `metadata_entries` is M and
      `packets_rewritten` is M+1: the replacement metadata packets plus the
      entity header, not the sum of physical SQL deletes and inserts.
      no missing path or new file is created. No converter path repairs a hot
      journal. A missing entity ledger, target already present, invalid source,
      missing/empty/nonregular store, recovery-required source or malformed
      arguments gives exit 2. A live owner gives StoreInUse, exit 3.
   3. For conversion, KEEP the validated RO connection and transaction open
      while opening the same canonical file READ_WRITE with no CREATE. Key it
      first, set busy_timeout(0), and acquire BEGIN IMMEDIATE (RESERVED).
      Only after this succeeds close RO. There is no unlocked handoff: RO
      prevents a commit, and the successful RESERVED lease excludes writers.
      A raw writer may still create a journal while RO holds SHARED; if it dies,
      RW cannot recover that journal while RO remains, so the attempt fails
      Busy without replay (exit 3). Never close RO and then retry RW admission.
   4. Within that same RW transaction, revalidate, then byte-copy the main file
      BEFORE any write or possible cache spill to a new backup created with
      create_new. Sync the backup file, reread and verify exact bytes, then
      fsync its parent directory before applying the plan. Both syncs must
      succeed before any source write; a test-only seam records their order.
      Copy through one descriptor of the store that stays open until the RW
      connection has closed: POSIX drops every lock a process holds on a file
      when any of its descriptors closes, so an open/read/close copy would
      release RESERVED before the source write. A separate-process probe pins
      this. Create the backup no more permissive than the store's own mode. A
      backup path that resolves to the store's `-journal`, `-wal` or `-shm`
      sidecar is a malformed argument (exit 2) and leaves no file.
      Apply one full Normal
      SyncPlan with model/application rows unchanged, then COMMIT. Factor the
      existing apply-plan body into a crate-private transaction helper; keep
      ordinary apply_sync_plan's transaction wrapper. Do not nest BEGIN.
      A later reader can make commit Busy: roll back, retain the verified backup
      and report exit 7, never success. The failed source stays logically exact;
      physical main-file byte identity after a rollback is not promised.
   5. Key-file read failure gives exit 4; wrong-key/protected-open failure is a
      static redacted exit 5; an existing backup path gives exit 6; other backup
      or conversion I/O/transaction failure gives exit 7. Pre-write refusals
      leave source main/sidecar bytes exact and create no file except an already
      completed requested backup. Never overwrite an existing backup. On a
      backup-preparation failure, remove only an incomplete backup created by
      this invocation: capture its file identity immediately after create_new
      and recheck device/inode before removal. If the path no longer matches,
      do not delete it. If identity checking or cleanup fails, report exit 7
      and disclose the leftover; do not claim a verified backup or conversion.
   6. Read exactly 32 key bytes once using a bounded read that detects an extra
      byte. Keep the owned key in zeroizing memory through both keyed opens,
      then wipe it; every early exit also wipes. Each connection's existing
      owned key/SQL copy is wiped as today. Never print keys, rows or plaintext
      recovery packets. No network access.
   The converter's read-to-reserved handoff is a tooling-only exception to
   R1's ordinary open-time EXCLUSIVE lease. It excludes competing owners and
   writers without asking a read-write pager to repair before backup.
   This is an offline operator tool. Its no-follow path precheck and SQLite's
   path-based opens are separate operations; an external actor replacing the
   file or a parent path between them is a residual, not prevented by the
   SQLite lease. Run only with the store path and its directory stable for the
   whole invocation. The tool makes no race-free pathname-identity claim.
3. **Library.** The re-encoding helper is
   `#[doc(hidden)] pub mod hoyadb_core::command::convert` (tooling-only). The
   facade, SDKs and `CommandDatabase` never call it. No automatic or SDK API
   exists. The SQLite binary delegates to a doc-hidden native-only tooling
   entry in the SQLite library, where private connections and transaction
   helpers remain accessible; it exposes no raw connection. Neither the facade,
   SDKs nor CommandDatabase calls either tooling entry. The core helper is
   `convert::reencode(&[Row], &ModelState, u8) -> LedgerConversion`, an opaque
   proof with private fields; the SQLite entry is `convert::run(ConvertOptions)`,
   and its `test-hooks`-only `convert::testing::run_with_observer` reports static
   lifecycle stages to fixtures without exposing data.
   Gate the SQLite tooling entry with a non-default `ledger-convert-tool`
   feature and native-target cfg. Declare the binary's required-features as
   `["ledger-convert-tool"]`; building it requires an explicit
   `--features ledger-convert-tool`. Ordinary facade, SDK and production builds
   do not compile this store-converting entry. SI-W02-style release graph checks
   prove no shipped dependency enables it. The core pure re-encoder remains
   doc-hidden and performs no filesystem operation.
4. Tests use synthetic stores only. A v2 → v3 → v2 round trip preserves exact
   ledger ids and canonical packet field bytes and exact canonical dumps of
   model/application rows. Sidecars are absent after close. This is logical
   byte identity, not SQLite main-file byte identity after two write transactions.
   The pre-conversion backup separately proves physical main-file byte identity.

## Exact public changes

| Item | Kind |
| --- | --- |
| `StorageError::StoreInUse` | new enum variant; exhaustive matches need an arm |
| `#[cfg(test)]` real deferred-FK COMMIT failure | test builds only; no public surface |
| feature `test-hooks` | native test builds only; non-default; forwards `rusqlite/functions` |
| feature `work-counters`, `storage::counters` | non-default; doc-hidden; dev self-enabled |
| `SyncStorage::ownership`, `StoreOwnership` | provided method and enum (Phase 1) |
| `SyncStorage::apply_sync_delta`, `SyncDelta` | provided method and struct (Phase 3) |
| `command::convert` | doc-hidden (Phase 3) |
| binary `hoyadb-convert-ledger`, SQLite tooling entry | native tooling only; non-default `ledger-convert-tool` feature; Phase 3 |

The wire ABI is unchanged.

## Integrity and compatibility

- **Persistent formats.** Phases 1 and 2 change no persisted byte. Format 3 is
  written only for newly created entity ledgers. Format 2 is never rewritten by
  open, reset or rebind. Older binaries cannot read format-3 entity state; their
  rollback path is R11 `--to v2`.
- **Unchanged checks:**
  - full validation and compare-and-swap for `Unproven` adapters;
  - SQLCipher authentication;
  - redaction;
  - row guards and stale-ticket and stale-pull semantics;
  - overlapping-scope rejection;
  - queue recovery;
  - merge hooks;
  - per-field stamps.
- **Behavior changes, disclosed:**
  - a second native open fails;
  - external readers are excluded while a store is owned;
  - command-capable open validates the whole inventory;
  - per-database FIFO on native, worker-wide on web.

## Exact Phase-1 contract amendments (root applies with Phase 1; not edited here)

**API-0002 L121-122.** Replace "The adapter owns its connection and can move
between threads with ownership; it promises no shared concurrent access." with:

> The adapter owns its connection and can move between threads with ownership.
> On native targets it also holds an exclusive SQLite file lock for its whole
> lifetime (`locking_mode=EXCLUSIVE`, acquired before any page read): another
> connection or process opening, reading or writing the same file fails, and a
> second hoyadb open returns `StoreInUse` (SPEC-0037 R1). The lock is released
> when the adapter is dropped or its process ends; no lock file is created. In
> this mode SQLite may keep a zeroed `-journal` sidecar between transactions.
> WASM targets make no exclusivity claim.

**API-0002 L86-91.** The memory sentence stays unchanged in Phase 1 (it is
amended with Phase 2 R7).

**API-0006 L113-115.** Replace "`ExecutionCoordinator::global()` uses one
process/worker singleton. A coordinator allocates checked monotonic owner and
request IDs, never reuses stale tickets, and queues requests FIFO." with:

> A command owner whose storage reports `StoreOwnership::Exclusive` on a native
> target uses its own coordinator, so serialization is per logical database
> (ADR-0037; SPEC-0037 R3). Every other owner — unknown custom adapters and all
> WASM builds — uses the process/worker singleton `ExecutionCoordinator::global()`.
> Owner and request IDs come from process-global checked monotonic allocators
> shared by all coordinators, so a ticket is never valid in another coordinator
> and is never reissued. Each coordinator queues its requests FIFO.

**API-0006 L248-250.** Replace "registers with the global coordinator and holds
an idle permit across Database's existing normalized schema lifecycle, since
opening may initialize or migrate rows. It cannot race another active handler's
global mutation exclusion." with:

> registers with its coordinator (SPEC-0037 R3) and holds an idle permit across
> Database's existing normalized schema lifecycle, since opening may initialize
> or migrate rows. It cannot race another active handler of the same
> coordinator.

## Later-phase candidate amendments (applied with their phases)

- **API-0002 memory sentence:** "touched-entry undo log; schema plans copy"
  (SPEC-0037 R7).
- **API-0007:** add "`apply_sync_delta` (provided; full-plan fallback) and
  `ownership` (provided; Unproven)", per SPEC-0037 R5/R9.
- **API-0009:** add the format-3 marker and metadata identity (R10). Format-2
  text is unchanged.
- **RISK-0003:** native file exclusivity is now enforced (R1). Browser,
  cross-tab, leader-election and cross-instance reactivity remain open.
- **`tools/javascript/README.md:39-40`:** "share that runtime; command FIFO
  is per database on native and worker-wide on the web".

## Recorded for Phase-3 review (not blocking Phase 1)

- **Converter dry-run must not write.** `open_inner` initializes an empty file
  (L122-131) and runs `CREATE INDEX IF NOT EXISTS` on every open (L161-163).
  The dry-run needs a read-only path that creates no file, schema or index, or
  it must reject stores that would require them.
- **The `ledger_format` marker survives entity clears.** Reset (`pull.rs:371-438`
  clears metadata and checkpoint) and reconfiguration must retain the marker. A
  format-3 ledger never silently reverts to format-2 encoding.
- **Bound the metadata identity length before allocating.** Format-3 id decode
  checks the hex length (even, and within a bound derived from `json::MAX_BYTES`)
  and the declared table length before allocating or decoding.

## Acceptance and TDD

Acceptance is TC-0041, phase by phase. Each case first fails on unchanged
source for its stated behavioral reason; a missing symbol does not count.
After each case group:
1. run the full workspace suite (at least 381 tests) and the Node suite (at
   least 84);
2. run both adapters' conformance suites;
3. seal a source-hashed evidence cut.

There are no timing assertions. There are no JDK/Android runs. The WASM
conformance rerun is a separate root decision.

## Quality attributes and rollback

- **Functional Suitability:**
  - the same logical database stays serialized while independent databases
    progress;
  - query, ordering and atomicity results are unchanged (oracle cases).
- **Performance Efficiency:** under Exclusive ownership, command, entity and
  page work depends on touched rows and changes (format 3), measured by
  physical counters. Open, reset and conversion visit total data by design.
  There is no numeric target.
- **Compatibility:**
  - additive provided trait methods with a conservative default;
  - one new error variant;
  - format 3 only for new ledgers;
  - format 2 preserved.
- **Interaction Capability:** a second owner gets a clear Busy/StoreInUse
  instead of silent unsafety.
- **Reliability:**
  - the lease is released by the OS on death;
  - the undo guard restores on errors and unwinding;
  - full validation at open catches closed-store edits.
- **Security:**
  - protected-open redaction is retained;
  - key bytes are never printed by the converter;
  - external readers are excluded while a store is owned.
- **Maintainability:**
  - one overlay path is reused by every staging site;
  - Unproven adapters keep the existing code;
  - counters and test hooks are test-only, guarded off in production.
- **Flexibility:** custom adapters opt in through `ownership()`.
- **Safety:**
  - no user store is converted, moved, deleted or reset;
  - the converter is offline, backed up and explicit.

**Rollback.** Revert task-owned source and documents together and keep every
RED/GREEN artifact.
- Phases 1 and 2 have no format residue.
- Phase 3 requires converting any format-3 store created meanwhile back with
  `hoyadb-convert-ledger --to v2` before running older binaries.
