# Phase 7 core scaling investigation — read-only, source-derived

Provenance: HEAD 416fffbc (docs-only over 2603a747). Traced core/storage/bindings
files carry uncommitted formatting overlays; `git diff -w` on pull.rs, sync.rs,
snapshot/sync.rs, snapshot/typed.rs, records.rs, sqlite sync.rs shows whitespace /
line-wrap only. Exact working-tree hashes: `sources.sha256`. No build, test,
benchmark or instrumented run executed. All counts below are from reading code,
not measurement. Contaminated wall times are not used for any forecast.

## Call chain (memory engine, reset sample, N, Q0)

benchmarks/native/src/lib.rs `reset_sample` (1285): configure once; if epoch 0
`bootstrap()`; timed `reset`; untimed reseed N/10 authors (batches of 100);
`bootstrap()` again. `bootstrap` (1269): P = N/100 pages, each `begin_pull` +
`apply_pull` with 100 full `create` changes, complete only on last page.
TC-0037/PLAN.md reset row requires "configured populated rows+comparison state",
so bootstrap before reset and restore after are legitimately required. Page size
100 is a harness choice (core cap 1000/page); not changed or recommended here.

Per page: bindings entities.rs `apply_pull` -> core pull.rs `apply_pull` (343):
1. `self.snapshot()` (sync.rs 172): scan+clone every app row, validate_row each. O(S)
2. `PullPlanner::new` (pull.rs 467/469): `before.clone()` O(S), `ledger.clone()` O(M)
3. `advance`/`accept`: 100 changes, O(100 log) — linear, fine
4. `finish` (578/591): `ledger.clone()` O(M), `rows.clone()` O(S)
5. `apply_planned` (sync.rs 193): diff all rows O(S); `planned.ledger != self.ledger` O(M)
6. `persist` (owner.rs 503): metadata compare O(M); `to_rows()` = validate (2 JSON
   encodes/entry, entity.rs 160/189) + 1 packet encode/entry; clones of
   persisted_rows and next_rows O(M)
7. memory `apply_sync_plan` (snapshot/sync.rs 41):
   - `validate_sync_plan`: `from_rows(next)` + `from_rows(expected)` each with
     parse + canonical re-encode + validate + `validate_sync_model` (re-validate)
   - `model_catalog()` (43) -> `sync_rows()` (lifecycle.rs 22): clone + parse + validate, result DISCARDED
   - `sync_rows()` again (46): identical input, identical work
   - `self.clone()` (67): full store incl. all indexes and M sync rows
   - `apply_typed_batch` on candidate (typed.rs 185): `self.clone()` AGAIN
   - delete + reinsert all M sync rows with index (74)
   - `candidate.model_catalog()` (77): parse+validate next ledger again + O(S) scan

S = app rows (~1.1N), M = entity metadata entries (grows 100/page to N).
Per page ~= 3 core S clones + 2 full-store clones + 3 O(S) scans/compares, and per
metadata entry ~= 5 full parse passes + ~28 canonical JSON encodes + >=7 clones/compares.

## Scaling (structural, deterministic from code)

Total per bootstrap ~ sum over P pages of (S + M_p):
sum M_p = 100*P(P+1)/2 -> N100: 100, N1000: 5,500, N10000: 505,000 entry-passes.
sum S (bootstrap 1) = P*1.1N -> 110 / 11,000 / 1,100,000 row-passes.
Each 10x in N gives ~92-100x work: Theta(N^2/100). First reset sample = 2
bootstraps + 1 linear reset + reseed. Confirmed quadratic structure; which term
dominates wall time is UNMEASURED.

## Classification

Contract-mandated (not removable in-scope): API-0007 full prior sync-row CAS and
"entire validated next recovery row set" per commit; API-0009 contiguous
zero-based `entity:metadata:<ordinal>` packets (ordinals shift on insert, so any
incremental scheme is a contract change); row guards; atomic candidate. The
O(M)-per-commit and O(S)-snapshot-per-page shape follows from these. Not a bug;
an algorithmic property of the current contract.

Provably redundant within contract (same input, result discarded or subsumed):
- R1 memory nested full-store clone: outer candidate (snapshot/sync.rs 67) is
  discarded on any error before `*self = candidate`; inner clone in
  `apply_typed_batch` (typed.rs 185) adds no atomicity there.
- R2 memory duplicate `sync_rows()`: `model_catalog()` parses/validates sync rows
  and drops them (lifecycle.rs 22); line 46 repeats on unchanged `&self`.
- R2' SQLite analogue: `catalog()` calls `load()` (lifecycle.rs 51), then
  sync.rs 33 calls `load()` again in the same transaction.
Removing R1+R2 cuts roughly one store clone and ~1/5 of storage JSON passes per
commit. Constant factor only; quadratic remains.

Not proposed (validation removal / stale reuse / contract change -> root/ADR):
final candidate catalog re-check, core to_rows pre-validation, cached parsed
ledger, delta SyncPlan, identity-keyed metadata packets, SQLite skip-unchanged writes.

Secondary, not reset-path: setup seeding uses memory `apply_typed_batch` full
clone per 100-row batch (Theta(N^2/100)); bindings budget.rs 82/101 re-encodes
the growing success array per write (Theta(batch^2) per call, linear in N);
fixture verify pages via `score Gt` + full sort per page (typed.rs 133-160).
