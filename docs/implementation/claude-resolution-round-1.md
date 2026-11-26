Review output from Claude Opus 5 on snapshot `094d59a`.

Parent verification note: result digests are recorded under the manifest's `results` field; the review's artifact-hygiene note about their absence was superseded by independently verifying those digests. The structural AND limit was documented, and owner-driven TEMP coverage was strengthened before round 2.

Round 1 complete. Repo untouched (`git status`: pre-existing CI edit + the three uncommitted evidence JSONs you mentioned). Scratch crates in `/tmp` removed.

# Verdict: PASS

DEF-0006/0007/0008 are resolved. All three prep blockers landed. I found **one real demonstrated residual** (low severity, outside DEF-0006's scope) and **one concrete test gap** covering behavior that is correct but unpinned. Nothing blocks.

# Prep corrections verified

- **B1** — `API-0005:129` now reads "A failed **candidate** refresh discards undelivered pending counts." Self-contradiction gone; `publish` only clears `pending_count` on the candidate path (`reactive.rs:659-663`), never on row-only failure.
- **B2** — specified and implemented. `observe` is unchanged, so a row-only-failed group is not dirty: count registration succeeds, row registration fails in its own `rows()` before `NEXT.fetch_update` and before `groups.push` (`reactive.rs:412` precedes `:415`/`:421`), so no token, listener, or cache. Verified behaviorally — see S6 below.
- **B3** — `PRAGMA temp_store=MEMORY` set in `open()` (`sqlite/src/lib.rs:111-115`), and the browser evidence asserts `poolAfter == poolBefore` plus per-case `poolChecks` equality. SPEC-0028:32 states it.
- Follow-ups also landed: kind/text_value/int_value encoding reuse (SPEC-0028:33), duplicate-error negative in TC-0031:32, scoped no-ceiling claim (SPEC-0028:33-34), `DEF-0006 parents: [REQ-0042, SPEC-0027, SPEC-0025]`.

# DEF verification

**DEF-0006.** `In` now lowers to one flat `IN (…)` node plus at most one `field.null` term, so the OR chain is gone (`query.rs:130-168`). Null is partitioned out of the value path and deduped — logically identical to the old per-value expansion, including empty-In → `"0"` and In-with-only-Null. `Plan::execute` is wired into all three read paths (`query_rows`, `projected_rows`, `query_plan`). Clear-before-use *and* clear-after, with `set_id` isolation so an abandoned set cannot match. My probes:

```
A observe at 5000 = ok          # reactive window above the 4096 TEMP threshold
A initial: ["Rows(5000)", "Count(5000)"]
A after insert: ["Rows(5001)", "Count(5001)"]
A fetch head_n=Integer(99999) second=Integer(4999)   # Desc order correct
B where_in(5001) = Ok(5001); same order as fetch = true
C k=4095/4096/4097/4098 -> Ok(4095)/Ok(4096)/Ok(4097)/Ok(4098)   # no off-by-one at the seam
D main sqlite_schema objects=6  operand_objects=0 ; reopen = Ok(1)
```

Threshold seam clean, TEMP confined to `temp`, main DDL untouched, reopen fine. `typed_cases review_large_operands` additionally pins DDL-identical-before/after, `query_plan` with 33005 operands, a real decoder error mid-TEMP-query not contaminating the next query, and reopen.

**DEF-0007/0008.** The split is implemented the right way round: `group.entries`/`dirty` and count `pending_count` are committed *before* `rows()` (`reactive.rs:597-604`), `rows()` failure marks only `columns.is_some()` listeners via `row_pending`, and `publish` selects on `group.dirty || any(row_pending)` (`:640-648`) so explicit refresh actually reaches a row-pending listener. `explicit_retry = dirty_only` gives per-refresh requeue and write-driven suppression. Probed the interleavings the suite doesn't enumerate:

```
S3 row-fail      : ["1:ERR", "2:Count(1)"]      # count survives row failure
S3 clean write   : ["1:Rows(2)", "2:Count(2)"]  # current rows, no duplicate error
S4 refresh-fail  : ["1:ERR"]                    # per-explicit-retry error, count silent
S4 write-no-dup  : ["2:Count(4)"]               # failing write adds no duplicate error
S4 refresh-ok    : ["1:Rows(4)"]
S5 other group   : ["3:Rows(1)"] ; idle refresh : []   # independent group isolated
S6 count reg     : Ok(4)                        # count registers while row pending
S6 row reg       : Err(rejected), observers 3 -> 4     # only the count reg added one
S6 refresh-ok    : ["1:Rows(5)"]
```

Also confirmed: a row-only retry via refresh makes **zero** projection calls (reuses valid keys — `affected` with empty changes never calls `project`), so recovery is cheap and cannot re-fail on the candidate path; and dropping the row listener leaves no stranded retry state (count listener then recovers normally, `groups=1 observers=1`).

`reactive_fault.rs` is the real-SQL-decoder fixture I asked for: corruption via a side channel *after* a successful `apply_typed_batch`, no manufactured error, with counters proving the production decoder actually failed. The two faults are well chosen — invalid UTF-8 in unconsulted `notes` exercises row-only failure (`real_projection_errors == 0` asserted), and a BLOB in the consulted `placed_at` order key exercises candidate failure. `assert_retained_state` pins no data loss, `next_id == 2`, and `__hoya_meta.version == 1`.

Native Q05 asserts `kind == 2 && error == 7`, caller-thread affinity, reentrant unlocked reads returning the committed row during the error callback, per-retry errors, exact recovery, count deadline held to 116, cancellation suppressing a queued error, close-from-error-callback, and stale fixture controls rejecting. Injection flag is per-`Entry`, not process-global.

# Findings

## P3 — demonstrated: AND-chained predicates still hit the expression-depth limit

**`crates/hoyadb-storage-sqlite/src/query.rs:206`** (`filters.join(" AND ")`) — the flat-IN change fixed the operand axis; the predicate axis is still an unbounded left-leaning chain.

```
1000 distinct Ne predicates -> Err(BackendFailure { "Expression tree is too large (maximum depth 1000)" })
5000  -> same ; 40000 -> same
```

Repro: `let mut q = db.query("o"); for i in 0..1000 { q = q.where_("n", Operator::Ne, Value::Integer(i)); } q.fetch(&db)`. Reachable through public `QueryBuilder::where_` (`model/query.rs:23`), which bounds nothing; the memory engine accepts the same query, so it is also an engine divergence.

Qualifications, stated plainly: this is **outside DEF-0006's scope** (that defect is In expansion), and SPEC-0028:33-34's no-ceiling claim is correctly scoped to "introduced by this lowering", so **nothing currently claimed is violated**. A 1000-predicate single query is not ordinary application use, unlike a 1000-row result set. My recommendation matches your stated stance: **document the bound rather than engineer for it** — one sentence in API-0003/SPEC-0025 that predicate count per query is bounded by the engine's expression depth. Balancing the AND tree in chunks would also work but is not worth the diff here.

## P3 — concrete test gap: the TEMP path is never driven through `ReactiveDatabase`

`tests/conformance/reactivity.rs review_q01` uses 996→1105 rows and 1105 reverse-FK/m2m operands — all **below** the 4096 threshold, so the reactive suite only ever exercises flat IN. The TEMP path is pinned solely by direct `project`/`query`/`query_plan` calls (`projection.rs review_q01`, `typed_cases review_large_operands`) and the browser `review_large_sets`.

What that leaves unpinned is real and supported: one `update_group` runs `resolve` → `keys` → `rows` as *sequential* `Plan::execute` calls, each doing its own create/clear/populate/clear. That sequencing, and the `id IN affected` ∧ `id IN targets` two-set intersection, are only exercised above 4096. I verified both work (probe A/B above, 5000-row window with correct Desc ordering and a 5001-operand public `where_in` matching `fetch` exactly) — so **no bug**, but a regression here would currently escape the whole suite.

Smallest fix: raise one reactive scenario above the threshold. The test already builds 1105 rows in a batch, so bumping that one scenario's population is a near-free change.

## Evidence hygiene (one line, not a finding)

`results/review-resolution-build-manifest.json` pins `git_head = 094d59a` with 52/52 source and 4/4 generated sha256 matching the tree and `rust_sources_newer_than_wasm: []` — stronger than last round since it now records the actual HEAD. It no longer carries the `observed_checks` per-result digests the Phase 3 manifest had, so the result JSONs themselves are not digest-pinned. Worth restoring when you commit them.

# Checks observed

| Check | Result |
|---|---|
| `cargo test --workspace --offline --locked` | **169 passed, 17 suites, exit 0** (158 prior + 11 new) |
| `cargo fmt --all --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | no issues |
| Browser evidence JSONs | stage1 **31/31** pass (`needsReload=true`), stage2 **12/12** pass after real reload, green **12/12**; RED file shows genuine behavioral failure |
| Manifest integrity | 52/52 source + 4/4 generated hashes match HEAD `094d59a` |
| Own probes | reactive >4096 window, 5001-operand `where_in`, threshold seam 4095–4098, main-schema/reopen, six `row_pending` interleavings, AND-chain ceiling |

# Residual scope

- **Not re-run:** browser and Swift suites, pinned container. Assessed by reading the diffs (`reactive-tests.js`, `reactive_bridge.rs`, `reactive.swift`, `phase3.h`, native `FixtureStorage`) against the recorded JSON and RED/GREEN notes.
- **Not executed:** the WASM `sqlite-wasm-rs` expression-depth/temp-store behavior. Source default is the same 1000/MEMORY-capable, and your worker evidence shows the oversized path passing with a flat SAH pool — but the boundary value itself remains unmeasured in-browser.
- **Not re-reviewed:** Phase 0–2 sources outside the touched files; I reused the prior audit and read only the `671daa2..094d59a` diff plus the files it touches.
- **Out of scope by your instruction and unchanged:** O(window) snapshot delivery, memory full-store clone, lifecycle full scans, and the ready-future/cancellation restriction (now stated in API-0005 rather than left implicit — adequate).

Ready for ROUND 2 when you have the next snapshot.
