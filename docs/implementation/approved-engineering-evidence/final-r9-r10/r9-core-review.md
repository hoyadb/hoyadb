# R9 core transaction/planner conversion — final read-only review (Claude Fable 5.1)

Date: 2026-10-31. Scope: `phase3-core-r9.md`, `phase3-core-r9.diff` (23 files), `phase3-core-r9-files.json`,
`phase3-core-r9-checks.json`, `phase3-core-r9-source-{before,after}.json` (179 inputs), the final logs,
`phase3-ledger-txn-audit.md`, `core-independent-review/review.md`, and the main sources named below.
No repository file edited; no build/test run by me. Not a whole-workspace, platform, R10/R11 or
Phase 3 completion verdict.

## Binding

- `source-before.json == source-after.json` (179 inputs). At the start of this review every input
  matched main byte-for-byte. At the end, four main files had moved: `command/entity.rs`,
  `command/merge.rs`, `command/pull.rs` (native counter supplement, reviewed separately) and
  `model/reactive.rs` (DEF-0061 lane). The semantic review below is of the frozen checkpoint; the
  integrated rerun root is preparing must re-bind.
- `files.json` current hashes matched main for all 23 files at review time.
- Final logs: core 193 unit + 3 supervisor integration + 6 compile-fail doctests pass; the one
  nested `FAILED` (`quality07_json_parser_is_bounded` child) is the supervisor's negative control
  and the parent exits 0. Adapters 15+15 counter cases, 1+1 mapping twins, 6+6 SI-Y03 cases; memory
  cancellation 2; Clippy four packages `--all-targets -D warnings` exit 0; fmt exit 0; proof-site
  checker `passed: true`, `errors: []` over 40 sources.

## Verified by code reading (main at freeze)

1. **Transaction guard** (`command/ledger_txn.rs`). Deref only, no DerefMut (74-79). First images
   captured once per key/identity before the first mutation (124-140); repeated touches keep the
   original Option image; `prior_record/prior_metadata` fall back to the untouched entry (216-226).
   `changed()` compares small headers plus touched keys only (270-275). `commit` detaches the borrow
   (276-278); `Drop` restores records, metadata, sync header and entity header, and removes a newly
   adopted shell (281-319). `validate()` under Exclusive = header intrinsic + full record graph +
   touched metadata entries (96-111); Unproven keeps full `SyncLedger::validate`.
2. **Sole proof constructor and commit boundary** (`command/owner.rs`). `transact` (537-568) runs
   the planner synchronously, early-returns on a semantic no-op without epoch/publication, then
   hands the guard to `LedgerCommit::persist`. `persist` (723-854): revision bump from the prior
   header iff mutations or net metadata change; tombstone cache from first-image vs final; checked
   epoch; reconfiguration and Unproven take `full_plan`; Exclusive runs `txn.validate`,
   `validate_sync_model_bounded`, `validate_ledger_transition`, `packet_changes`, permit check,
   then the single `SyncDelta::validated_exclusive` call (813) inside `if self.db.db.validated_exclusive
   { if reconfigure {..} else {..} }`. Packets, ranks, model, epoch, cache are installed only after
   `apply_sync_delta` returns Ok (823-845); `txn.commit()` precedes `delivery.changed` (846-847).
   A panic or error anywhere before 823 drops the guard and restores the ledger; model, packets,
   ranks, cache and epoch are untouched (pinned by `si_y01_late_planner_error_…`, owner.rs:1749).
3. **No await inside a transaction.** `LedgerTxn::new` production sites: `transact` (owner.rs:547)
   and the unpersisted `prepare_push` wake (sync.rs:107-109, synchronous, dropped, never committed).
   Every planner takes `&mut LedgerTxn<'_>` from a synchronous `FnOnce`; no `async fn` accepts a
   transaction (grep). `apply_transaction` (sync.rs:290-318) awaits only after `transact` returns
   and publishes only when the epoch advanced.
4. **Transition validation** (owner.rs:953-1074). Namespace/sequence/last_ms non-regression;
   every changed retained record with a prior frozen envelope must keep `frozen` and `envelope()`
   (968-975); touched metadata columns checked against the normalized storage schema with the
   original `BackendFailure` message; adoption, disappearance, reconfiguration-on-pristine-shell,
   bootstrap non-regression, generation and exact revision rules match the storage validator's
   `changed = mutations || metadata change`.
5. **Format-2 rank/suffix** (owner.rs:883-951). Value-only edits below the first membership shift
   use the cached rank; the shift start is the prior successor's rank or `ranks.len()` for append;
   the staged suffix is re-encoded with `start + offset`; departed identities get `None`; tail
   ordinals `[new_len, old_len)` are deleted; `retain` drops no-op packets. I traced insert-middle,
   delete-middle, append, and insert+delete across the shift; all correct. Pinned at M=10/1000,
   position 0/mid/end by `si_y05_…` (owner.rs:1788) including rank-cache equality and drop restore.
6. **Prior vs staged.** `push_in` (queue.rs:143-219): clock/jitter then `validate`, capped set and
   eligible set computed from the prior view with effective capped deadlines before any
   `record_mut`; dispatch mutates in order; `next_due_ms` from staged records. `receive_in` and
   `redeliver_in` mutate only the target and return no-ops without change. `finish_entity_scope`
   (entity_owner.rs:576-728): each write reads staged metadata/records and header; `put_metadata`
   and `insert_record` first-image once; duplicate key → Protocol; publication after epoch change.
   `reconcile_in` (reconcile.rs:138-450): all shape/mapping/typed/closure/guard checks and the
   inverse proof precede the first mutation; the C4 path mutates only after `rebase_in` returns
   `Some` (fallback.rs:187-202, every E/G refusal before `record_mut`/`put_metadata`); metadata
   moves are explicit remove+put; record undo/reference rewrites go through `insert_record` only
   when changed; promotion uses the post-mapping, pre-clear accepted record; `settle` renormalizes
   queued arguments against the staged ledger then size-checks; `finish_in` derives outcomes from
   `prior_record`, trims, serialization-checks. `inverse_in` (655-737) simulates rows and metadata
   before touching the transaction; cancel-conflict commits Cancelled bookkeeping and returns
   `RollbackConflict` after commit; rollback-conflict tries C4 then policy Discard. `discard::plan_in`
   checks the whole closure before mutation. Order of existing errors retained (SI-Y06 pins).
7. **Bounded mapped-reference snapshot** (sync.rs:323-421). Identities = closure undo + metadata
   undo + plan writes + mapping local/remote + explicit guards, plus, when a mapping relocates,
   every row whose declared `Reference` column holds a mapped local (one indexed `where_in` per
   Reference column per table). All schema tables present. Every lookup `reconcile_in`,
   `guard_outside_closure`, `simulate_inverse`, `apply_entity_correction`, `promote_metadata` and
   `rewrite_row` performs against `rows` is for an identity in that set, so Exclusive and Unproven
   produce the same plan; `row_plan` derives guards only for identities present in `before`/`rows`
   (SI-Y08 twin asserts the outside-closure child guard and the absent-target guard).
8. **Stale sessions and reset.** Exclusive merge compares epoch + model + schema + re-read touched
   rows; every ledger write path bumps the epoch or is a dropped no-op (enumerated in my pull/merge
   review). `reset` (pull.rs) installs ledger/packets/epoch/cache and clears sessions only after
   the full plan succeeds.

## Findings

No confirmed semantic defect in the frozen R9 core. Items below are instrumentation or hardening.

1. **LOW — uncounted row copies inside whole-record clones (`reconcile.rs:423`, `:1001`).**
   `ledger.records[&k].clone()` runs for every record in the ledger (and, in `promote_metadata`,
   once per accepted metadata-undo entry). `SyncRecord::clone` counts `ledger_entries_cloned`
   and `RowMetadata::clone` counts, but the `CommittedChange`/`Row` images in `undo` are not
   counted in `rows_cloned` (`Row` has a derived `Clone`; only the `clone_row` helper counts).
   Not an N/M bound violation (record count is capacity-bound), but it is exactly the class of
   omission the native lane is auditing; the equal-at-M/N claims for reconcile should state that
   per-record undo image copies are uninstrumented or instrument them. No algorithm change.
2. **INFO — early no-op return ignores explicit guards (`owner.rs:555`).** `transact` returns
   before storage when the ledger is unchanged, `mutations` is empty and the model is unchanged,
   even if `guards` is non-empty. Today unreachable: `reconcile_in`'s idempotent path requires
   empty guards and every other guarded path changes the ledger. Consider adding
   `&& guards.is_empty()` so a future planner cannot skip a requested CAS silently.
3. **INFO — IN-list size on SQLite.** The bounded snapshot issues `where_in("id", ids)` and
   `where_in(reference_field, mapped)`; the SQLite adapter renders a literal `IN (…)` list
   (`hoyadb-storage-sqlite/src/query.rs:138-163`). Closure undo identities are capacity-bounded,
   so this is far below SQLite's variable limit in practice, but the previous `scan` path had no
   such ceiling. Document the bound or chunk if capacity can be configured very high. Hypothesis;
   not exercised by the retained logs.
4. **INFO — binding movement.** `entity.rs`, `pull.rs`, `merge.rs` and `reactive.rs` moved on
   main during this review (expected supplements). The 248-case checkpoint certifies the frozen
   bytes only; the integrated rerun must re-freeze.

## Limits

- Logs were read, not re-executed; conclusions on behaviour rest on source reading of the frozen
  tree plus the retained RED/GREEN evidence described in `phase3-core-r9.md`.
- Format-2 middle insertion cost, Unproven full validation cost and raw full plans are outside
  the equal-at-N/M claims, as the packet states.
- Root's full immutable workspace run, platform artifacts, R10 format 3 and R11 remain separate gates.

## Correction (appended 2026-10-31, after root's recheck request)

Re-verified against the retained bytes (`records.rs` `7d22b881…`, `hoyadb-storage-sqlite/src/query.rs`
`06364609…`, both equal to the 179-input snapshot):

- **Finding 1 withdrawn.** `SyncRecord::clone` (`records.rs:112-118`) counts
  `rows_cloned += Σ(before.is_some() + after.is_some())` over `undo` before `undo.clone()` at
  line 132, and `si_w03_sync_record_clone_is_exact_and_counts_nested_metadata` (line 2287)
  asserts both nested Row images (`rows_cloned == 2`). The whole-record clones at
  `reconcile.rs:423` and `:1001` are therefore fully instrumented. My statement that `Row` has
  only a derived `Clone` was true but irrelevant: the count is taken at the record level.
- **Finding 3 withdrawn.** `query.rs:124-161` switches to constant-shape
  `temp.__hoya_query_operands` subqueries when total operands exceed 4096; the literal bind list
  is used only below that. No IN-list ceiling applies to the bounded resolution snapshot.
- **Finding 2 unchanged** as INFO: the guard-only no-op early return is unreachable with
  current planners; no hardening requested.

Net: no actionable finding against the frozen R9 core. Verdict stands as "no confirmed
semantic defect"; the remaining items are INFO (binding movement, pending integrated gate).
