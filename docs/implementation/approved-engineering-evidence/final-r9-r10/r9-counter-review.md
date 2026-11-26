# R9 candidate-copy counter supplement — read-only review (Claude Fable 5.1)

Date: 2026-10-31. Packet `/private/tmp/hoyadb-followup-20261031/r9-candidate-green-work/packet.json`
sha256 `f0571389…7ac063` (matches root's hash). No repository edit; no build/test run by me.

## Binding

- 179 `source_inputs` all match the frozen tree; `verification-before == verification-after`.
- `entity.rs` `8d64e951…`, `pull.rs` `56b0cba7…`, `merge.rs` `bd74db90…` equal current main at
  review time; `import.json` before-hashes equal the ba8229 packet's reviewed bytes.
- `owned.diff` = three `count_work!(rows_cloned, …)` sites plus comments in production and three
  new unit tests; nothing else. Tree differs from `r9-pull-final-work` only by the core lane's
  already-reviewed R9 conversion plus these three files.
- RED (`r9-candidate-work/red.log`): 3 FAILED with exact counts 0 vs 2, 0 vs 3, 2 vs 4; existing
  `touched_snapshot` counter PASS. GREEN: core 196, metrics 6, memory 5+1+8+1, SQLite 5+1+9
  (226 unique), 0 ignored; `clippy.log` no warnings, exit 0; owned fmt and main diff check exit 0.

## Accounting scopes (verified against source)

1. `resolve_entity` (`entity.rs:325`): counts the present current row copied into the initial
   `EntityResolution.row` (`current.cloned()`), including the tombstone/delete paths that then
   discard it. Actual clone, correctly counted; absent → 0.
2. `resolve_entity` non-delete branch (`entity.rs:342`): second `current.cloned()` for the working
   row; absent allocates a fresh row → 0. Correct.
3. `PullPlanner::advance` host offer (`pull.rs:560`): the retained `OwnedMergeInput.current`
   clone, present → 1. Correct; the resolver's two copies precede it, so 3 total for a present row.
4. `merge_reply` (`merge.rs:270-275`): counts `input.current` and `input.default.row` after the
   internal `merge_input` clone of the pending offer. The public `merge_input` call stays
   uncounted by design and the test's 0-count control pins that boundary. Expected 4/3/1 in the
   test equal 2/1/1 internal copies plus the two present-row stale-check snapshots
   (`clone_snapshot(before)` and the touched re-read) — consistent with `validate_merge_state`.

No algorithm, clone or result change. Validation refusal before any copy is pinned (0).
All prior M/N equal-work groups still pass with the added counts (metrics 6 PASS), so the bounds
are retained; the counts are page-bounded.

## Findings

None actionable. Observation: per my core review, the remaining uncounted core-owned Row copies
are the whole-record clones in `reconcile.rs:423` and `:1001` (undo images inside `SyncRecord`
clones); outside this packet's three files, record-count bounded, flagged there as LOW.

## Limits

Not rerun by me; main moved after this packet only in `model/reactive.rs` (DEF-0061 lane).
Host Rust only; no platform artifact claim.
