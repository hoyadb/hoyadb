# Phase 6 native encrypted-engine acceptance

This independent test lane covers TC-0035 S02–S07 through the production native
SQLite adapter. Its source is
`crates/hoyadb-storage-sqlite/tests/security_engine_cases.rs`; it changes no
adapter implementation, dependency manifest or shared conformance source.
ADR-0034 (`e318c4a`) and the reviewed API-0010 precede these tests. Only fixed
synthetic externally supplied keys and fresh owned temporary directories are used.

## TDD and direct security checks

The first supplied-key open case compiled against the adapter's functional stub
and failed at the intended protected-open assertion. The observed output is
`target/phase-six-security-engine/s02-stub-red.txt`. This was not a compile or
missing-symbol failure. The root subsequently activated the approved SQLCipher
engine and supplied-key implementation; the independent test source did not
weaken its open assertion.

- **S02:** verify actual protected-open capability and approved cipher version,
  exact keyed row recovery, raw main-file canary/key/header absence, and failure
  of a separate unkeyed read-only connection to read the schema. That reader is
  a separate connection using the linked SQLCipher engine without a key, not a
  separately installed stock SQLite executable.
- **S02/S07:** persist/reopen entity recovery containing trusted identity,
  complete frozen retry bytes, comparison state and a partial bootstrap
  checkpoint. Acknowledged writes do not resend and exact data survives reopen.
- **S03:** snapshot every owned file's bytes; missing/default/wrong keys and an
  explicit unprotected attempt preserve an encrypted store. Protected opens of
  a plaintext private store and an encrypted foreign schema reject without
  stamping, conversion or reset. The original key continues to read its data.
- **S04:** corrupt one byte in the authenticated first page of an owned copy.
  Protected open fails before an owner can publish data/progress and leaves the
  damaged file unchanged. The untouched original still recovers rows, checkpoint
  and acknowledged recovery. This is authenticated corruption, not a claim that
  the damaged copy can be repaired.
- **S05:** separate writer processes hold a real rollback-journal transaction
  before commit or a real WAL update after commit. Each reaches an explicit
  durable marker before the parent kills it. Nonempty journal/WAL pages exist
  and raw files omit complete domain/key canaries before interruption. Correct-
  key reopen returns precisely the old/new value for the selected barrier.
- **S05:** execute an actual 33006-operand query, above the TEMP-set threshold,
  including order/filter work; the result is exact and the owned directory file
  inventory is unchanged. This external test does not claim that directory
  inspection observes every OS temporary file. The adapter-owned protected-profile
  unit separately reads `temp_store=2` from the actual protected connection.
- **S07:** distinct writer processes stage or finish a real public command while
  prior entity receipt/bootstrap recovery exists. Kill at named pre/postcommit
  barriers; reopen with the supplied original key and a different proposed
  namespace. Rows, counters, original namespace/checkpoint, acknowledged entity
  suppression and byte-identical command retry all match the committed state.

The process driver captures stderr and checks complete synthetic key/domain
canaries are absent. Every spawned child has a kill/wait cleanup guard. These
checks establish process-termination recovery, not power-loss durability.

## Encrypted behavioral parity

An owned forwarding fixture supplies the same encrypted factory to **60 unchanged
shared functions**: ten typed, fifteen model, sixteen reactive, one command owner,
five dispatch/reconciliation/capacity, five entity and eight pull/reset cases.
Only storage construction and cleanup differ. No query, resolver, queue,
transaction or recovery algorithm is duplicated in this fixture.

The native test binary reports **53 named tests**: 41 individually registered
shared tests, four groups calling the remaining 19 generic shared functions,
seven direct security cases, and one re-executed child-driver entry. The driver
entry returns immediately in the ordinary parent run; the substantive process
assertions run through its four explicit child modes from the security cases.
The configured-500 capacity workload and existing assertions are unchanged.

## Verification record

Focused wrong-key/format preservation, journal/WAL interruption, command/entity
process recovery and large TEMP operands have passed. Owned-file rustfmt and
diff-whitespace checks passed. Test-target Clippy with warnings denied passed.
The complete encrypted matrix passed **53 tests, zero failures** in **212.03 s**
in the unoptimized debug test profile. That includes the unchanged configured-500
workload; these observed synthetic costs are not a latency or throughput SLO.
The test harness printed its ordinary over-60-second notices while the capacity
group ran and other owner groups waited on their shared-coordinator test lock.
Every group subsequently passed; no deadline or assertion was relaxed.

Exact commands and local ignored output:

| Command | Output under `target/phase-six-security-engine/` |
| --- | --- |
| `cargo test -p hoyadb-storage-sqlite --test security_engine_cases --offline --locked s03_ -- --nocapture` | s03-first.txt |
| `cargo test -p hoyadb-storage-sqlite --test security_engine_cases --offline --locked s05_real_ -- --nocapture` | s05-sidecars-first.txt |
| `cargo test -p hoyadb-storage-sqlite --test security_engine_cases --offline --locked s07_command -- --nocapture` | s07-process-first.txt |
| `cargo test -p hoyadb-storage-sqlite --test security_engine_cases --offline --locked s05_large_ -- --nocapture` | s05-temp-first.txt |
| `cargo test -p hoyadb-storage-sqlite --test security_engine_cases --offline --locked` | full-first.txt |
| `cargo clippy -p hoyadb-storage-sqlite --test security_engine_cases --offline --locked -- -D warnings` | clippy-first.txt |
| Owned-file `rustfmt --check` and diff-whitespace check | fmt.txt |

The parent owns dependency/source provenance, actual Swift/browser protection,
backup controls, full-workspace/container gates and final independent review.
This lane makes no new platform, key lifecycle/rotation, FIPS, compromised-app,
backup-suppression, universal performance or release claim. No user stores were
read, migrated or modified; no Git commit was made by this lane.
