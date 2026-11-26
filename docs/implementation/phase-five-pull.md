# Phase 5 pull/bootstrap/reset implementation evidence

This report covers the production pull/bootstrap/reset slice of
[API-0009](../design/contracts/API-0009-entity-pull-reset.md) and
[TC-0034](../design/tests/cases/TC-0034-entity-sync-conformance.md), checked on
2026-10-24. It is a subset report; foreign hosts and the complete Phase 5 gate
are reported separately. Contract preparation was committed before test code
(`7a85ec4`, with the typed foreign page representation in `770855c`).
Typed correction contract/test preparation followed in `9fec544`/`2a5d648`;
post-reset reconfiguration preparation followed in `5da83b5`.

## Implemented behavior

- `CommandDatabase` owns checked, process-unique pull tickets. A ticket captures
  the opaque checkpoint and durable generation/revision. Transport holds no
  admission permit. Busy retains the original ticket; StalePull retires it.
- `PullPage::to_json/from_json` provide the shared strict tagged-cell page
  representation, preserving i64 and finite Float without foreign number
  conversion. A canonical page is at most 1 MiB and contains at most 1000 changes.
- Page application resolves current rows before one guarded SyncPlan commit.
  Live recovery protects rows and comparison metadata, including dependency
  closure and unresolved producer insertion collisions. Inbound changes can
  address any declared table and create no outbound diffs.
- Checkpoint, comparison state, readiness and checked page/change counts commit
  together. Readiness is sticky. Progress notifications are owned, redacted,
  and queued only after success.
- Explicit reset has a separate storage transition. Both adapters check complete
  actual row inventory inside the candidate/transaction; reset cannot omit a
  concurrent unexpected application row. Recovery-preserving reset refuses live
  work. Discard reset clears application rows and recovery while retaining schema,
  configuration and identity counters. Subscription/ticket invalidation follows
  successful commit only.
- Typed reconciliation corrections carry backend-selected values and exact
  per-field/tombstone clocks. They override superseded optimistic clocks while
  preserving unrelated fields, validate before commit, and distinguish optimistic
  deletion recovery from forbidden confirmed-tombstone resurrection. Identity-
  preserving mappings retain a valid correction identity; actual retired IDs
  reject. Entity-origin legacy Put/Remove rejects instead of inventing clocks.
- Reconfiguration of the durable binding requires an empty post-reset shell,
  unchanged counters/model/header, no recovery records and no application rows.
  The adapters prove actual emptiness inside the same transaction/candidate;
  a pristine metadata cache alone cannot conceal an unconfigured local row.
- Nonpristine comparison/checkpoint state blocks migration. A pristine shell
  permits migration only if its retained binding still validates against the
  target schema. Schema-aware storage validation also checks live entity payload
  types and reference paths.

## Behavioral RED and verification

Meaningful failing assertions were observed before the corresponding fixes:

| Regression | Captured RED log under `target/` |
| --- | --- |
| Configured pull admission rejected | `phase-five-pull-ticket-red.txt` |
| Empty page could not commit progress/checkpoint | `phase-five-page-red.txt` |
| Busy/StalePull semantics absent | `phase-five-pull-stale-red.txt` |
| Lossless bounded page codec absent | `phase-five-page-codec-red.txt` |
| Reset accepted missing previously loaded ledger | `phase-five-reset-red.txt` |
| Normal transition advanced reset generation | `phase-five-transition-red.txt` |
| Reset omitted an actual row | `phase-five-reset-inventory-red.txt` |
| Incoming declared nonconfigured table rejected | `phase-five-pull-unconfigured-red.txt` |
| Comparison tombstone failed to block migration | `phase-five-lifecycle-red.txt` |
| Initial entity binding accepted generation two | `phase-five-initial-binding-red.txt` |
| Migration removed a still-configured table | `phase-five-migration-target-red.txt` |
| Normal transition erased bootstrap progress | `phase-five-progress-transition-red.txt` |
| Live queued entity payload accepted undeclared field | `phase-five-entity-storage-schema-red.txt` |
| Entity correction accepted an unclocked legacy write | `phase-five-correction-untyped-red.txt` |
| Typed correction could not apply selected clocks | `phase-five-correction-clocks-red.txt` |
| Reconfigure accepted generation one | `phase-five-reconfigure-generation-red.txt` |
| Reconfigure changed the identity counter | `phase-five-reconfigure-counter-red.txt` |
| Reconfigure omitted an actual application row | `phase-five-reconfigure-inventory-red.txt` |

The early combined `phase-five-pull-red.txt` additionally contains a poisoned
shared test lock after the first intended failure. Ticket admission was rerun
alone and failed on its behavioral assertion, as captured separately above.

Observed targeted verification:

| Command | Result and evidence under `target/` |
| --- | --- |
| `cargo test -p hoyadb-core --offline --locked command::pull::tests -- --nocapture` | 8 passed; `phase-five-pull-core-green.txt` |
| `cargo test -p hoyadb-core --offline --locked command::reconcile::tests -- --nocapture` | 14 passed, including preserved Phase 4 cases; `phase-five-correction-core-green.txt` |
| `cargo test -p hoyadb-core --lib --offline --locked` | All 78 core unit tests passed; `phase-five-correction-core-all.txt` |
| `cargo clippy -p hoyadb-core -p hoyadb-storage-memory -p hoyadb-storage-sqlite --all-targets --offline --locked -- -D warnings` | Passed; `phase-five-pull-clippy-final.txt` |
| `cargo test -p hoyadb-storage-memory --test pull_cases --offline --locked` | 8 passed; `phase-five-pull-memory-final.txt` |
| `cargo test -p hoyadb-storage-sqlite --test pull_cases --test entity_process --offline --locked` | 9 adapter tests and 1 process test passed; `phase-five-pull-sqlite-final.txt` |
| `cargo build -p hoyadb-core -p hoyadb-storage-memory --target wasm32-unknown-unknown --offline --locked` | Passed; `phase-five-pull-wasm-final.txt` |
| `rustfmt --check` on this slice's owned production/test files | Passed; `phase-five-pull-format-final.txt` |

The SQLite fault case installs a deferred foreign-key violation fired by sync
packet insertion, so failure occurs at COMMIT. It verifies data/progress and
pending observers remain intact, then retries the same original ticket
successfully. Failed reset retains data and the old ticket. The file then
reopens and continues the next bootstrap page. Physical SQLite tables remain
exactly `__hoya_fields`, `__hoya_meta`, `__hoya_rows`, and `__hoya_tables`.

The process test starts four distinct OS processes (`write`, `read`, `reset`,
`read-reset`). An acknowledged entity does not resend after restart; its pending
outcome/reconciliation resumes. Pull comparison/checkpoint/readiness survives
restart. Reset retains the namespace and next local-ID counter, so the next
created entity receives counter two rather than reusing counter one.

The initial Clippy capture found a parent-owned collapsible-if lint. The final
all-target run passed after that fix; the earlier failed output remains in
`target/phase-five-pull-clippy.txt` for provenance. Full-workspace and foreign-host
checks remain the parent integration gate. Separate physical writers retain the
existing exact-CAS/reopen convention; no cross-owner cache refresh or concurrent-
writer support is introduced.

The durable typed-correction case also observes the correction/outcome only after
commit, rejects an unclocked correction without events, makes an outstanding pull
StalePull after the correction's revision, preserves a selected clock of 50
against a later incoming clock of 40, and accepts a later incoming clock of 75.
That proves selected backend clock 50 actually replaced superseded optimistic
clock 100 in comparison state. No secondary outgoing write is created.

## Final protected-no-op regression

A later independent native audit reproduced a checkpoint-loss path: confirmed
row `old10`, queued optimistic Delete at time 1000, and incoming Update at time
20. The resolver kept the pending deletion unchanged and pull incorrectly
consumed the update's checkpoint; cancelling Delete then restored `old10` after
the checkpoint had advanced past the backend update. Its captured reproduction
is `target/phase-five-pull-audit.txt`.

API-0009/TC-0034 clarification `e4e1cf2` was reviewed and committed before the
regression/fix. The existing shared `pull_recovery_protection` case failed for
the required reason on both adapters (`phase-five-hidden-noop-memory-red.txt`,
`phase-five-hidden-noop-sqlite-red.txt`) and then passed unchanged after the
minimal guard. It also covers a newer optimistic field clock suppressing an
older backend value, checkpoint/progress/event preservation, cancellation and
fresh refetch, and an authoritative tombstone with live net-zero command undo
remaining a harmless nonblocking no-op.

Protected identity overlap now returns PullConflict even when optimistic
filtering makes resolution appear unchanged. Only unchanged row/comparison state
under an existing authoritative tombstone bypasses that overlap guard. No rebase,
rollback simulation, broader sync state machine or dependency was added.

Post-fix checks use `target/phase-five-hidden-noop-*` logs. Earlier full-core and
build results above describe their earlier snapshots; the parent final
integration report supplies the full-workspace and host gates on the frozen
post-fix source.

Observed post-fix verification passed: core pull 8, memory shared 8, SQLite shared
9 and independent-process 1; all-target Clippy for core/memory/SQLite; WASM build
for core/memory; and rustfmt plus diff-whitespace checks. Evidence is respectively
`phase-five-hidden-noop-core-green.txt`, `phase-five-hidden-noop-memory-final.txt`,
`phase-five-hidden-noop-sqlite-final.txt`, `phase-five-hidden-noop-clippy.txt`,
`phase-five-hidden-noop-wasm.txt`, and `phase-five-hidden-noop-format.txt` under
`target/`.

## Bounded comparison-state footprint measurement

A validation-only probe used the frozen production owner and both production
adapters on fresh stores containing 10, 100, or 1,000 rows with exactly two
fields (`label` String and `rank` Integer). Each store received one data page and
then an empty checkpoint-only page. No queue/history records or subscriptions
were present. All six runs completed and their exact synthetic SQLite directory
was removed. Production/test source and dependencies were unchanged.

| Rows | Metadata packets | Metadata `data` JSON UTF-8 bytes | All sync packets | All `data` JSON UTF-8 bytes | Sum of canonical packet-object bytes |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 10 | 10 | 1,910 | 12 | 2,236 | 3,237 |
| 100 | 100 | 19,100 | 102 | 19,428 | 28,349 |
| 1,000 | 1,000 | 191,000 | 1,002 | 191,330 | 279,451 |

Both adapters produced identical byte counts. For this fixed field/ID/time
shape, comparison packets cost exactly 191 data-JSON bytes per row. The final
column sums each canonical `{"id":...,"format":...,"data":<String>}` object,
including escaping the inner data String and excluding container-array brackets
and commas. These are actual decoded packet Strings serialized through the
production Json codec, not SQLite raw-cell ASCII, file sizes, disk-write bytes
or memory-consumption estimates. The 1,000-change input page was 258,948 UTF-8
bytes, within the committed page bound.

| Adapter | Rows | Data-page apply (ms) | Empty checkpoint-page apply (ms) |
| --- | ---: | ---: | ---: |
| Memory | 10 | 16.328 | 12.925 |
| SQLite | 10 | 14.588 | 19.950 |
| Memory | 100 | 70.406 | 110.999 |
| SQLite | 100 | 100.562 | 156.238 |
| Memory | 1,000 | 691.300 | 1,111.554 |
| SQLite | 1,000 | 962.882 | 1,459.550 |

The empty page had zero application mutations but its successful SyncPlan still
carried all 12/102/1,002 ledger packets, with every comparison packet unchanged.
The inspected SQLite adapter deletes all sync cells/rows and reinserts every
next packet; the snapshot adapter replaces the whole logical sync table in its
candidate. Thus packet storage and the number of packets rewritten per commit
are O(N) for a fixed number/size of field stamps; this does not claim total CPU
time is O(N). The timed owner call also scans current application rows and
validates old/new state, so the empty-page figure does not isolate SQL rewrite
cost. It does expose the cost of retaining full-state persistence even
when a page changes only checkpoint/readiness/progress.

This was one unoptimized debug run with no warmup, repetitions, percentiles or
SLO claim: Rust 1.98.1, macOS x86_64, 16 available parallel workers. OS load averages
were `{ 11.57 27.36 18.44 }` before and `{ 10.55 26.63 18.28 }` after, during
concurrent integration/host work. The probe's transparent shared inspection
wrapper adds mutex forwarding and linear packet counting inside the timer;
all owner/resolver/adapter algorithms remain production code. Configuration,
input-byte accounting, post-commit inspection, compilation and cleanup were
outside the timer. The rows represent one deterministic two-field workload,
not general performance or scaling guarantees.

Exact source and separate locked graph are retained under
`target/phase-five-footprint-probe/`; the source SHA-256 is
`073d6338a81d509f0c74a94675c29d0f9b4019cc46244686e1b30e1e16bf9e23`.
Native dependency versions match the workspace lock. Reproduction command,
source hashes and cleanup assertion are in `target/phase-five-footprint-manifest.json`;
raw results and cargo output are `target/phase-five-footprint-observed.jsonl`
and `target/phase-five-footprint-run.txt`. The probe README defines every byte
counter and timer boundary.

The durable evidence also includes the [exact probe source](phase-five-footprint-probe.rs),
[observed logical-byte samples](phase-five-footprint-observed.jsonl) and
[measurement manifest](phase-five-footprint-manifest.json). Reproduce with the
probe package described above using the existing workspace path dependencies;
this is validation tooling, not a new runtime dependency or a benchmark suite.
