# DEF-0062 SQLite terminal-correction fixture repair — read-only review (Claude Fable 5.1)

Date: 2026-10-31. Frozen source `/tmp/hoyadb-followup-20261031/sqlite-terminal-fault/final-work`
(`final-source.json`, 183 inputs). No repository edit; no build/test run by me.

## Binding

- 183 inputs match `final-work`; all 183 match current main (the repaired test is already the
  main bytes, `07cd9ea4…`). `red-source.json` differs from final only in
  `crates/hoyadb-storage-sqlite/tests/command_sync_cases.rs`. No production file changed.
- `red.log`: diagnostic copy with the old `BEFORE INSERT ON __hoya_fields` trigger prints the
  armed result `Ok(())` and fails at line 56 (exit 101). `final.log`: both entries pass
  (bundled `c09_c12_c16_c19_dispatch_delivery` and new `terminal_correction_failure_is_atomic`),
  default parallel scheduling, 25.81 s. Scoped Clippy finished clean after root's message
  (`clippy.log` ends `Finished`, no diagnostics); fmt/diff checks exit 0.

## Why the old fixture stopped firing (confirmed from source)

The Exclusive delta path (`hoyadb-storage-sqlite/src/sync.rs:88-105`) runs `typed_batch` (the
mapping relocates `parents/local → parents/server:fault`), replaces the model row, registers the
sync table, then processes `parts.deletes` via `remove` (`lib.rs:344-357`: `__hoya_fields` then
`__hoya_rows`), then `write_packet` for upserts. With `history: 0` the resolved terminal command
is trimmed, so the only sync change is the deletion of `command:<key>`; the `state` header is
unchanged and `packet_changes` retains nothing, so no `INSERT … __hoya_fields` for `__hoya_sync`
occurs. The old INSERT trigger therefore never fired; the pre-R9 full plan rewrote every packet
and masked this. The new trigger targets the actual write.

## Repaired fixture (`command_sync_cases.rs:34`)

`BEFORE DELETE ON __hoya_rows WHEN OLD.table_name='__hoya_sync' AND OLD.id LIKE 'command:%' AND
EXISTS(parents/server:fault) AND hoya_test_fault(...) → RAISE(ABORT)`.

- Fires exactly at the terminal command deletion, after the application mapping and model
  writes within the same transaction; the `EXISTS` predicate is statement-side evidence that
  the mapped row was already written when the fault hit, so the subsequent rollback assertions
  (`fetch == [row]`, no events, pending reconciliation retained) prove atomicity rather than a
  pre-write refusal. `RAISE(ABORT)` surfaces as the exact `BackendFailure` message; the adapter's
  transaction helper rolls back (reviewed earlier for rusqlite 661107f).
- Strengthened, not weakened: exact error text instead of `BackendFailure { .. }`; exact
  `QueueStatus` equality and unchanged `ledger_work().epoch` after the failed commit; after the
  unarmed retry, mapped rows asserted via the public query, empty records on raw reload, and a
  strict `CommandDatabase::open` reopen with empty queue and mapped rows. All original
  assertions retained (rollback, no events, pending reconciliation, retry outcomes, empty history).
- Specificity: `LIKE 'command:%'` excludes header and `entity:record:` deletions; the fixture
  installs the trigger while the store is closed (R2.3) and arms it only on the live owner;
  unarmed retry leaves the trigger inert.
- Isolation: new entry uses `hoya-sync-terminal-<pid>.sqlite`, bundled entry
  `hoya-sync-dispatch-<pid>.sqlite`; `test_hooks::arm` is keyed by (path, name), so parallel runs
  do not collide; files are task-owned and removed.

## Findings

None. Not a weakened test and no unsupported claim; the repair tests the real R9 write path.

Observation (INFO): `terminal_failure` now runs twice per binary (bundled and focused). Harmless
duplication; the focused entry is the one with the diagnostic message, keep it.

## Limits

Not rerun by me. Root's integrated workspace gate on the post-DEF-0062 snapshot remains pending.
