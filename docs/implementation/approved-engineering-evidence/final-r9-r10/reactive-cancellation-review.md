# DEF-0061 cancelled publication recovery — read-only review (Claude Fable 5.1)

Date: 2026-10-31. Frozen source `/tmp/hoyadb-followup-20261031/reactive-cancellation/final-work`
(`final-source.json`, 180 inputs). Reviewed runtime `model/reactive.rs` `a9f6323e…`, test
`reactive_cancellation_cases.rs` `20e8fdbc…`, API-0005 `166f7560…`. No repository edit; no
build/test run by me.

## Binding

- All 180 frozen inputs match `final-work` and matched main at review time (`source-check.json`
  `main_movement: []`, my recheck: no drift).
- `fix.diff` = `Publication` guard + loop rewrite in `reactive.rs`, the new memory test binary,
  and the API-0005 paragraph. No command/*.rs change; shared `yielding.rs` and
  `ledger_cancellation_cases.rs` unchanged (same hashes as the SI-Y01 packet).
- RED `red.log`: both tests fail at line 149 `assert_live` with groups 0 vs 3 after a genuine
  delta commit (`delta_commits == 1`) and a genuine Pending second-group projection/row read.
  `queue-red.log`: third test fails 1 vs 3 against the first draft's queue clearing. Final:
  3 new + 2 ledger cancellation + 1 smoke PASS; 16 reactive conformance PASS; core 2, SQLite 2,
  Clippy exit 0, fmt/diff/trace 0 (`results.json`).

## Verdict: NO DEFECT found. Two non-blocking observations.

### Guard, borrows and Drop boundary (`reactive.rs:135-156`, `792-836`)

- `Publication { owner: &mut ReactiveDatabase, groups, next }` is created after `mem::take`; the
  loop borrows `publication.groups[next]` and `publication.owner` as disjoint fields, so the
  compiler-checked split holds; no unsafe, no clone of groups or caches.
- `next` advances only after `update_group` returns (Ok or Err) or a group is skipped; there is
  no await between the awaited call's completion and `next += 1`, so a drop can only leave the
  group at `next` unfinished and groups `> next` unvisited.
- `Drop`: groups `[next..]` get `dirty = true`, every listener `pending_count = None`,
  `count_valid = false`; `delivered_count` (value, time) is untouched so the 16 ms deadline is
  preserved; then `owner.groups = take(groups)`. Completed groups `[..next]` are untouched. On
  normal completion `next == len` and Drop is a pure restore. A panic inside `update_group` now
  also restores registrations (improvement over the old code).
- Mid-group state: `update_group` mutates `entries`/`ordered` (`replace_entries` or
  remove/insert), sets `dirty = false` and `pending_count = Some(len)` before its final await
  (`self.rows(...)` at 766), and pushes to `self.queue` only after that await (768-788). Hence a
  drop during projection leaves entries untouched; a drop during materialization leaves updated
  entries with a stale/partial view, which the Drop's `dirty = true` forces to a full recompute
  on the next publish/refresh (`recovering` path, 705-708, `notify` includes `recovering`). The
  discarded `pending_count` prevents an incomplete count from draining (test 3, 116 ms empty).
- Previously queued events are retained: nothing in Drop touches `owner.queue`, and the
  interrupted group had not yet pushed. The first-draft queue clearing was correctly removed; the
  third test pins it.
- No new normal-path work: the only added operations on the success path are the index
  increments and the vector move already present before.

### Tests

Non-vacuous: `cancel_publication` asserts `is_pending()` on every poll, requires one genuine
`apply_sync_delta` commit and exactly two `project` calls (first group complete, second
suspended), and for `Suspension::Rows` one more poll that advances `reads` by one without a third
projection. Recovery asserts exact rows via `fetch`, exact counts, deadline behaviour at 115/116/
117, repeated refresh cancellation, unsubscribe of a completed group, and a later committed patch.
The yielding adapter forwards genuine deltas (reviewed in the SI-Y01 packet).

### API-0005 wording

Matches the code: registrations retained; completed groups and earlier queued snapshots kept;
incomplete/unvisited groups discard pending counts and recover via refresh or a later write;
delivery time and 16 ms deadline preserved; no rollback or completion promise; no general
cancellation-safety claim.

## Observations (no action required)

1. A cancelled `refresh_notifications` (dirty_only) marks every unvisited group dirty, including
   clean groups the loop would have skipped, so the next publish recomputes them fully. Correct
   and cancellation-path only; slightly more work than strictly necessary.
2. `stats.candidate_updates` is incremented for an interrupted group whose update is then
   discarded. Statistics only.

## Limits

Not rerun by me; root's integrated 183-input snapshot and broader checks remain the gate.
