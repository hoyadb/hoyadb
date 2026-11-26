# R9 pull / passive merge conversion — read-only review (Claude Fable 5.1)

Date: 2026-10-31. Packet: `/private/tmp/hoyadb-followup-20261031/r9-pull-final-work`
(`packet.json` sha256 `ba8229ef…aea749`). No repository file edited; no build or test run by me;
not a whole-workspace, platform or Phase 3 completion verdict.

## Binding

- `packet.json.source_inputs`: 167 inputs, all match the frozen tree; `verification-before`
  and `-after` hashes are equal (`480603c6…`).
- `pull.rs` `b75ffa93…`, `merge.rs` `a8369937…`, `phase3_cases.rs` `633c3603…` equal current
  main at review time. `owner.rs` and `ledger_txn.rs` in the checkpoint differ from main
  (sibling core lane), as the README states.
- `owned.diff` touches only `pull.rs`, `merge.rs` and the one `phase3_cases.rs` pin snippet
  (`phase3-pin-before/after.txt` match the diff hunk).
- Results: core pull 12, snapshot counter 1, SI-Y06 pins 6, memory 37 (13+5+9+1+8+1), SQLite 29
  (14+5+1+9); 0 failed, 0 ignored; rustfmt on the three files exit 0; advisory clippy: five
  warnings, none in pull.rs/merge.rs (dead `Planned`/`apply_planned*`, `trim`,
  `record_keys`/`metadata_keys`, one `entity_owner.rs:638` needless borrow). Not `-D warnings`.
- RED: `r9-pull-work/incremental-red-final.log` 4 compiled failures with the quoted counters
  (23→2003, 500→2300, 38→3008, metadata visits 10 vs 0); `snapshot-counter-red.log` 0 vs 1.

## Verified by code reading (frozen tree)

1. **Bounded pull under Exclusive** (`pull.rs:357-368`). `touched_snapshot` reads only the
   page's distinct identities via `storage.get`, validates each row and strips `ROW_VERSION`
   (`merge.rs:226-250`). `PullPlanner::new` seeds a metadata overlay for the distinct page
   identities only (`pull.rs:496-503`); `advance`/`accept` read and write that overlay
   (`pull.rs:529`, `585-597`); duplicates resolve against the previously accepted overlay value.
   `finish` copies only `rows` (page-bounded) and the changed metadata (`pull.rs:603-618`).
   No `SyncLedger` clone remains on the pull path.
2. **Single synchronous ledger transaction.** `PullPlan::stage` runs inside `transact`'s
   closure (`sync.rs apply_transaction`, `pull.rs:377-380`, `merge.rs:316-317`): header edits
   through `edit_entity_header`, metadata through `put_metadata` (first image captured,
   `ledger_txn.rs:192-207`), then `trim_in`, `row_plan`, and the owner commit. No `LedgerTxn`
   crosses an await or a callback: `PullPlan` holds owned data only; `merge_reply`'s closure
   touches the planner, not a transaction. `LedgerTxn::drop` restores records, metadata, the
   sync header and the entity header (`ledger_txn.rs:367-402`), so a `stage`/`row_plan`/commit
   error leaves the owner ledger exact.
3. **Semantic equivalence with the retired path.** Old `finish` set checkpoint/ready/pages/
   changes on a cloned ledger and `accept` inserted metadata; new `stage` applies the same four
   header edits with the same checked arithmetic and puts metadata only where the final overlay
   value differs from the live entry. Absent-before metadata that resolves to a non-default or
   row-changing result is still persisted (as before); an unchanged read is not. `row_plan`
   diffs only identities present in `before`/`rows`, so a touched-only snapshot yields exactly
   the page's mutations and guards; untouched rows are neither guarded nor written (Exclusive
   promise, SPEC-0037 R6.6).
4. **Exclusive merge staleness by epoch** (`merge.rs:139-141`, `183-191`). I enumerated every
   production write to `self.ledger` in the checkpoint: `transact` (`owner.rs:562`, bumps
   `ledger_epoch` on any committed change; early-returns only when `!txn.changed()` and no
   mutations and same model), `reset` (`pull.rs:453-460`, bumps and clears `merge`),
   `reconfigure_entities` via `persist`→`transact`, `configure_entities` via `transact`, and
   `prepare_push`'s unpersisted direct `LedgerTxn` (`sync.rs:107`) which is dropped, never
   committed. No path mutates the ledger without an epoch bump. Model and schema are still
   compared separately, and the touched snapshot is re-read and compared before commit
   (`validate_merge_state`). `merge_reply` validates before `advance(&self.ledger)` reads live
   records for recovery protection, so protection sees the preparation-epoch records.
   `protected()` walks live records only, no metadata (`pull.rs`), matching SI-Y09's 0 visits.
5. **Unproven retention.** `MergeExpected::Unproven` keeps the boxed full expected ledger and
   persisted inventory; staleness still compares both plus the full snapshot. Persisted copy
   is counted with an explicit `map_or(0, len)` before the clone (`merge.rs:143-146`).
6. **Counter accuracy.** `touched_snapshot` counts one `rows_cloned` per present row returned;
   `clone_snapshot` counts its copies (`reconcile.rs:16-22`); `row_plan` counts prior/after
   images; `SnapshotStorage::get` is not instrumented for this counter (README claim consistent
   with the test's 1 vs 0 result).
7. **SI-Y06 pin.** `LedgerTxn::new(&mut ledger, false)` + `stage` + `serialization_check()`
   equals the old `planned.ledger.to_rows()` on the staged clone for a non-exclusive txn
   (`ledger_txn.rs:112-118`); the asserted record-level error is unchanged.
8. **Fixtures.** The two private pull fixtures' no-op `transact` replaces the removed persist
   bridge; `transact` early-returns on a no-op, so fixture state is unchanged, as claimed.

## Findings

No actionable defect in `pull.rs` or `merge.rs`.

- **INFO 1 (pre-existing order, acknowledged).** `apply_pull` and `merge_prepare` acquire the
  (touched or full) snapshot before `PullPlanner::new` validates the page (`page.to_json()`,
  checkpoint rule). Under Exclusive an invalid page still costs one `get` per distinct
  identity before rejection; bounded by page size, no state change. Not a regression.
- **INFO 2.** The clippy advisory's dead-code items (`Planned`, `apply_planned*`, `trim`,
  `record_keys`/`metadata_keys`) and the `entity_owner.rs:638` needless borrow must be cleared
  by the owning lane before the workspace `-D warnings` gate; they are not pull/merge defects.
- **INFO 3.** Old `merge_packet_copies` characterization expecting the retired full inventory
  copy is, per README, owned by core/parent. I did not see its replacement in this packet.

## Limits

- Not rerun by me; conclusions rest on the retained logs and reading of the frozen tree.
- The checkpoint's `owner.rs`/`ledger_txn.rs` are not main's; the integrated rerun root is
  preparing is required before any claim about main.
- Host Rust only; no Android/Apple/browser artifact claim; R10/R11 remain separate gates.
