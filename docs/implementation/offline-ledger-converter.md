# Offline ledger converter — 2026-10-31

Status: implemented under SPEC-0037 R11 and TC-0041 SI-X01–X08 / SI-XL01–XL06;
verification recorded below. This continues the
[R9/R10 follow-up](approved-engineering-completion.md). Implementer: the Fable 5.1
session that previously acted as read-only reviewer; the review notes in this
report are therefore the implementer's own, not an independent cross-review.

## Decision and baseline

The operator answered "Allow the full-file check" (DECISIONS.md, 2026-10-31):
a protected read-only WAL refusal (`IOERR_LOCK`) is classified on the same
retained connection with SQLCipher's `cipher_integrity_check`, whose main-file
reads and failure-row memory grow with file size. Failure rows give static exit
5, zero rows give unsupported-source exit 2, a pragma error fails closed as 5;
no outcome admits WAL or converts. Unprotected sources skip the check.

The task baseline (`/tmp/hoyadb-r11-20261031/`) holds 3,381 tracked/non-ignored
files on HEAD `035bd0f180ca73c017813f03c454d4ad85936927`, captured before edits.
Partial edits from three interrupted workers (core scaffold and tests, SQLite
feature/module/binary scaffolds, no tests) were inspected and completed rather
than discarded. Synthetic fixtures only; no user store, commit, publication or
new dependency.

## Implementation

- `crates/hoyadb-core/src/command/convert.rs` (doc-hidden, filesystem-free):
  `reencode(&[Row], &ModelState, u8) -> LedgerConversion`. Full
  `SyncLedger::from_rows` then `validate_sync_model` precede target checks;
  missing entity, same target and targets outside 2|3 are refused. Every packet
  other than `entity:state` and `entity:metadata:*` is carried byte for byte;
  the header gains or loses only `ledger_format`; metadata packets change id
  only; output is id-ordered and must decode back to the same ledger. The proof
  has private fields and borrowed accessors; its report is
  `{from,to,metadata_entries: M, packets_rewritten: M+1}`.
- `crates/hoyadb-storage-sqlite/src/convert.rs` (native, non-default
  `ledger-convert-tool`): `run(ConvertOptions)`. Order: argument/target check;
  bounded 32-byte key read (33-byte probe, zeroizing) → 4; no-follow regular
  nonempty file precheck → 2; READ_ONLY/no-CREATE open, key first,
  `temp_store=MEMORY`, `busy_timeout(0)`, `locking_mode=EXCLUSIVE`, `BEGIN`,
  first `sqlite_schema` read (Busy → 3, `READONLY_ROLLBACK` → 2, keyed
  `NOTADB` → 5, keyed `IOERR_LOCK` → integrity classification, else 2); strict
  private format, catalog with recorded model, `load` of the complete ledger;
  `reencode`. Dry run closes and reports. Backup mode: existing backup → 6;
  READ_WRITE/no-CREATE second connection, key first, `BEGIN IMMEDIATE`
  (Busy → 3) while the read lease is still held, then the read connection is
  closed; revalidation inside the reserved transaction; `create_new` backup with
  device/inode captured, exact main-file copy, file sync, reread verification,
  parent-directory fsync, all before any write (failures remove only this
  invocation's backup after identity recheck, otherwise disclose → 7); the
  genuine proof is applied through the shared `rewrite_sync_rows` helper after
  re-deriving the marker-only property, model CAS and exact packet comparison;
  COMMIT failure rolls back and keeps the verified backup → 7. Errors are static
  strings; no key, path, row or packet is printed.
- The ordinary validator is untouched: `validate_sync_plan` still refuses
  marker changes for every `SyncPlan`; the converter never constructs one. Two
  behaviour-preserving extractions share code: `validate_private_format` in
  `lib.rs` (strict-open DDL/version check) and `rewrite_sync_rows` in `sync.rs`
  (full-plan recovery rewrite). `StorageError` gains no variant; Busy maps to
  exit 3 inside the tool.
- `src/bin/hoyadb-convert-ledger.rs` with `required-features =
  ["ledger-convert-tool"]`; `convert::testing::run_with_observer` exists only
  with `test-hooks` and reports static lifecycle stages.
- `tests/tooling/test_convert_boundary.py`: no runtime, facade, SDK or binding
  source names the entries; no manifest enables the feature on the SQLite
  dependency; the binary is absent without the feature and declared with it.

## RED/GREEN evidence (`/tmp/hoyadb-r11-20261031/evidence/`)

- `core-convert-red.log`: the inherited fixture itself failed
  `validate_sync_model` (missing `recordId` reference path on the entity
  update record); corrected in the test, not in production.
- `core-convert-red-fixed-fixture.log`: with the corrected fixture and the
  inert stub, 6 of 8 cases fail (2 refusal controls pass). `core-convert-green.log`:
  8 pass.
- `convert-process-first.log` → `convert-process-green.log`: the SQLite suite
  went 12/20 → 14/20 → 18/20 → 20/20 while three fixture defects were fixed
  (metadata count 6 = 4 pulled + 2 local writes; barrier files excluded from
  inventories; hot journals need a synced header, produced by a mid-transaction
  cache spill). No production change was needed between these runs. A
  retrospective inert-scaffold RED for the SQLite lane is recorded in
  `convert-process-stub-red.log`: with `run_inner` replaced by the original
  `Err(InvalidSource)` scaffold on the final tests, 17 of 20 cases fail and only
  the three cases whose expected outcome is exit 2 (or the child no-op) pass.
  The implementation was restored byte-identically afterwards (hash in the
  manifest).
- Final focused results on the final source (`final-tests.log`): core
  `command::convert` 8/8; `convert_process` 20/20 covering SI-X01/X02/X03/X05/
  X06/X07/X08 and SI-XL01–XL06 through the real binary and the staged library
  entry (reserved writer, donor hot journal published during the handoff, late
  reader at commit, production opens at every barrier, swapped backup path).
- Hot-journal insight: SQLite zeroes the journal magic until the journal is
  synced, so a writer killed before COMMIT or blocked at COMMIT leaves an
  ignorable journal. The fixture forces a cache spill; the "published during
  handoff" case copies a genuine spilled journal beside the source because no
  real writer can sync one while the converter holds SHARED.

## Gates on the final source

| Check | Result |
| --- | --- |
| `cargo test -p hoyadb-storage-sqlite -p hoyadb-core` (feature off) | pass (`ordinary-core-sqlite-tests.log`; the nested `quality07` child failure is the supervisor control) |
| `cargo clippy -p hoyadb-core -p hoyadb-storage-memory -p hoyadb-storage-sqlite -p hoyadb-bindings --all-targets -- -D warnings` | pass |
| `cargo clippy -p hoyadb-storage-sqlite --all-targets --features ledger-convert-tool -- -D warnings` | pass after two test-lint fixes (`gates-retry3.log`) |
| `cargo fmt --all --check` | pass |
| `RUSTFLAGS='-D warnings' cargo build -p hoyadb-core -p hoyadb-storage-memory --target wasm32-unknown-unknown` | pass |
| `cargo build -p hoyadb-storage-sqlite --lib --target wasm32-unknown-unknown` | pass with the WASM-capable LLVM 22 clang/ar (`gates-retry.log`) |
| `cargo build -p hoyadb-storage-sqlite --features ledger-convert-tool --bin hoyadb-convert-ledger` | pass, no warnings (`bin-build.log`) |
| `cargo build --bin hoyadb-convert-ledger` without the feature | refused: "requires the features: `ledger-convert-tool`" (tooling test) |
| `python3 -m unittest discover -s tests/tooling` | 56 pass; `sync_proof.py` passed |
| `trace_check.py`, `rtm_gen.py --check` | 0 violations, RTM up to date |
| `cargo test --workspace --offline --locked` | exit 0; 890 passed, 0 failed, 0 ignored across 90 outer Cargo blocks (882 before this task plus the 8 new core cases); the one nested `FAILED` line is the supervised `quality07` child whose parent passes (`final-tests.log`) |

Source hashes before/after for every changed and new file, and the tool binary
hash, are in `evidence/r11-source-manifest.json`. All commands used
`CARGO_TARGET_DIR=/tmp/hoyadb-r11-20261031/target`, two jobs, incremental off.

## Limits and remaining decisions

- Host macOS x86_64 only. No Android/Apple/WASM artifact was rebuilt; the core
  and SQLite crates changed, so earlier platform artifacts no longer match
  current source hashes and need fresh builds before any new platform claim.
- The path precheck and SQLite's open are separate operations; the tool assumes
  a stable path for the whole invocation (SPEC-0037 R11).
- Wrong-key and corrupt protected files both report exit 5; the integrity check
  is O(file) with proportional failure-row memory, as approved.
- The converter is an operator tool; no SDK, facade or owner path calls it.
  Factory double-failure cleanup, release/license/security/registry choices,
  React Native consumer assurance, GitLab execution and wider certification
  remain open decisions untouched by this work.

## Follow-up — 2026-11-10 (review corrections)

The report above is unchanged history. A later review found three converter
defects, corrected in `crates/hoyadb-storage-sqlite/src/convert.rs`:

- The backup copy used an open/read/close of the store. POSIX releases every
  lock a process holds on a file when any of its descriptors closes, so the
  RESERVED lease ended at the copy and another process could take the write
  lease before the source write. The copy now reads through one descriptor
  opened before the connections and closed after them.
  `staged::si_xl05_no_other_process_takes_the_write_lease_after_the_backup_copy`
  probes from a separate process; the earlier in-process probe shares SQLite's
  lock table and could not see this.
- The backup is created with the store's permission bits (further limited by
  the umask) instead of the default mode
  (`si_x06_backup_is_never_more_permissive_than_the_source_store`).
- A backup path that resolves to the store's `-journal`, `-wal` or `-shm`
  sidecar is refused with exit 2 and removed; previously SQLite reused and then
  deleted a `-journal` backup while the tool reported success
  (`si_x06_backup_path_naming_a_store_sidecar_is_refused_without_writing`).
