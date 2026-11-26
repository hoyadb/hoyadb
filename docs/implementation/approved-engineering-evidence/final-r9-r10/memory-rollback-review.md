# Memory UndoGuard fix (DEF-0059) and SQLite write-counter correction — read-only review

Reviewer: Claude Fable 5.1. Date: 2026-10-31. No repository file edited; no API baseline
accepted; no Phase 3 or platform completion claim.

## Binding

- `undo.rs` `e00ccf7d…`, `delta_tests.rs` `8e339548…`, `mod.rs` `ddf6fc8f…` match the
  packet; `undo.rs` and `mod.rs` are byte-identical to
  `memory-delta-final-work/crates/hoyadb-core/src/storage/snapshot/`.
- `memory-delta-final-source.json`: 178 inputs, all match `memory-delta-final-work`.
- `mod.rs.diff` is only `#[cfg(test)] mod delta_tests;`. `mutant.diff` touches only the
  memory `apply_sync_delta` guard (ManuallyDrop) and no production file.
- The invalid shared-target attempt is recorded in `packet.json.invalid_attempt` and
  superseded by the unique-target run; I treated only the fresh logs as evidence.

## Verdict: NO DEFECTS in the frozen fix. Observations below are non-blocking.

### Fix correctness (frozen `undo.rs`)

- **Shape capture before mutation.** `absent_index_fields` (lines 40-58) reads only the
  table index map and the touched row's own field names (`id` plus `row.fields.keys()`),
  returning `None` when the table index is absent or the sorted set of field names whose
  maps are absent. It allocates before any map write: `insert` computes it at line 90
  before `rows.insert` (line 99); `replace` at line 123 before `mem::replace` (line 128),
  with `log.reserve(1)` at line 127 so the push after the write cannot allocate; `remove`
  at line 160 before `rows.remove` (line 162), likewise reserved. No value sets, no
  unrelated rows, no table-wide field scan: O(fields of the touched row).
- **Restoration order.** `Drop` (lines 197-259) pops entries in reverse. For each `Row`
  entry it removes whatever the primitive left (`rows.remove` + `remove_entries` of the
  current and attempted images), re-inserts `before` and its entries, then removes exactly
  the maps that were absent before that entry (whole table index for `None`, listed field
  maps for `Some`). Because later entries are undone first, a map listed as absent by an
  earlier entry is empty again when that entry runs, so `index.remove(&field)` deletes
  only shells it introduced. Pre-existing empty maps are never listed and therefore
  survive (test 3). Multi-write same table/row/field: a second write to a field whose map
  the first write created sees the map present and records nothing; the first write's
  entry removes it last. Correct for insert→insert, replace→replace and replace→remove
  sequences I traced.
- **Interaction with `Undo::Table`.** `clear_table` moves rows and index out; `Row`
  entries pushed after it are undone first, then the table entry restores or removes the
  table and index. Unchanged by this fix.
- **Generic callers.** `apply_sync_plan` (full path), `typed_batch`, and schema paths use
  the same primitives; the added field only tightens rollback, never changes the success
  path. `commit()` still clears the log at the `before-commit` fault point.
- **Why the old code failed.** `remove_entries` (mod.rs 194-222) prunes empty value sets
  but never field maps or the table index, so a panic after `insert_entries` left empty
  field/table shells. The fix addresses exactly that.

### Tests (`delta_tests.rs`)

- Genuine proof: `Capture` forwards every adapter call and only intercepts
  `apply_sync_delta` once when armed, storing the owner-produced `SyncDelta` and failing
  the owner's finish with a storage error; the owner is closed normally before the proof
  is applied under `catch_unwind`. No proof construction or mutation; a fresh owner on the
  restored store yields a new proof for the retry. Asserts the owner never falls back to
  `apply_sync_plan` while armed.
- Fault matrix derived from the real proof inventory (mutations, upserts classified by
  existing row, deletes, model replace, optional table creation, before-commit), with
  skip counts per fault point; `fault was not reached` guards over-counting.
- Exact `tables` and `indexes` equality against the pre-apply store, then fresh-proof
  retry equals the clean expected store, then strict reopen equality.
- Four matrices: initial sync registration, existing replace/remove/insert, fully
  pre-existing empty field maps, id-only partial prior shape. RED 2/1 and 2/2 before the
  fix, 4 pass after; ManuallyDrop mutant fails all four on raw-row equality.

### Observations (no action required for this packet)

1. `absent_index_fields` includes the *before* row's field names in `replace`/`remove`.
   On a consistent store those maps already exist and nothing is listed; on an
   inconsistent store (a stored field without a map) rollback would recreate and then
   remove that map, restoring the prior unindexed shape exactly. Correct, just noting the
   reasoning since it is non-obvious.
2. `clear_table` on a store with an index but no table (inconsistent) drops that index on
   rollback. Pre-existing behavior, unchanged here.
3. Core WIP dead-code warnings and final Clippy/planner/platform gates remain pending, as
   the packet states.

## SQLite write-counter correction (`sqlite-r9/counter-review`)

- `sync.rs` `31ba4850…` matches the packet; `fix.diff` wraps only the
  `INSERT INTO __hoya_tables … ON CONFLICT(name) DO NOTHING` result in `super::written(...)`
  and adds `si_y07_delta_write_counter_includes_first_registration`.
- `counter-green-source.json` (177 inputs) binds `engine-delta-current-work` plus this
  fixed `sync.rs`; the only difference from that base is the fixed file, as declared. This
  resolves the earlier manifest/test-module mismatch without rewriting the older packet.
- The regression compares `Connection::total_changes()` delta against
  `sqlite_rows_written` across the genuine adapter apply boundary for a first registration
  and an already-registered control. RED 16 vs 17 on first registration before the fix;
  GREEN 6 unit groups and 32 integration cases after. Apply-interval counters unchanged
  (13 read / 30 written / 1731 VM steps / 0 fullscan / 0 sort; metadata visits now 2).
- Finding 2 of my earlier SQLite review is closed. No new issue.
