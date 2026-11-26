# Claude Opus 5 resolution review: round 2 of 3

Agent: `61919d0e-d590-49b6-bb36-43fc2761d6c3`.
Independent review; production snapshot `094d59a`, subsequent documentation
and shared test changes through `7e531fe` plus the bounded-case working diff.
Temporary reviewer probes were removed before delivery.

+Showing 1 of 95 activities (limited to 1)

No reachable parent agent to message — report below.

## ROUND 2: PASS — no actionable findings

No production defect reproduced. Production code unchanged since `094d59a` (verified: `aa74685` test + `fd6f1ab` docs on top, no working-tree edits under `crates/`).

**Scratch probes removed.** `crates/hoyadb-storage-sqlite/tests/zz_round2_probe.rs` and `zz_round2_reactive_probe.rs` deleted. `git status` shows only the six `.gitlab-ci.yml` tag lines, the test-only edits (`tests/conformance/reactivity.rs`, `spikes/storage-browser/{tests.js,reactive-tests.js,worker.js,src/model_evidence.rs}`) and untracked evidence files. Nothing of mine retained.

### Gates run (all green)
- `cargo test --workspace --offline --locked` — 171 passed, 17 suites (169 → 171 = memory + sqlite instances of the new shared case)
- `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean
- `cargo build -p hoyadb-core -p hoyadb-storage-memory --target wasm32-unknown-unknown` clean

### Probes written, run against real SQLite, then deleted
1. **Inline-vs-TEMP parity per payload kind/affinity** — String (`""`, `"123"`, non-ASCII), Integer (`i64::MIN/MAX/0`), Timestamp (negative, extrema), Boolean, Reference, Float (`2.0`, `1.5`, `-0.0`, `1e300`, `MIN_POSITIVE`, 2^53+1). Sharpest risk in the fix: `temp.__hoya_query_operands.int_value INTEGER` gives NUMERIC affinity, collapsing integral REALs to INTEGER — identical to `__hoya_fields.int_value`, and `decode_value`'s `(kind 4, Integer)` arm covers readback. Same window both paths, every kind.
2. **Multi-set isolation** — two sets on one *cached* `Field` intersect (not union); relation-edge set + local set + `ORDER BY owner.score DESC` + `LIMIT 3` identical inline vs TEMP; `query_plan` leaves no residue for the next read.
3. **Inline flat-IN at exactly 4096 and 4097** over a relation edge — the DEF-0006 boundary holds on the pinned native build.
4. **Null operand alternative** on an optional column at 5000 operands.
5. **Reactive relation-keyed window** — `order_by("customer.country", Desc).limit(2)`; delivered rows == `fetch()` after a related-row-only reorder, after deleting a related row (dangling ref, NULL order key), and after re-pointing to refill. `affected()` FK-dependency path, `visible()` Null/Desc/id-ASC tiebreak, and SQL `CASE WHEN kind THEN value END` + NULLS placement all agree.

### Invariants traced (no defect)
- `query.rs` — dense `set_id`; all-Null oversized predicate short-circuits `Plan::execute`; only set indices + payload column literals reach SQL; `present_target` deliberately absent from the order field, matching `projection.rs` returning Null for a dangling order key.
- `Plan::execute` — no nesting exists; ids fully materialized inside the closure, `cell()`/`get()` run after cleanup; cleanup error surfaced only when the operation succeeded; `temp_store=MEMORY` set in `open()` for every connection.
- **DEF-0008** — `group.entries`/`dirty` commit before `rows()`; every path that can return `Err` from `update_group` precedes that commit, so a failing publish leaves state intact. A `rows()` failure cannot reach count listeners.
- **DEF-0007** — `!listener.row_pending` suppresses write-driven repeats; `refresh_notifications` passes `explicit_retry=true`; group-level `!group.dirty || dirty_only` requeues on every explicit refresh. 16ms deadline retained.
- `observe()` — no subscription ID burned on failure (`fetch_update` runs after `rows()`), no group pushed, refuses a dirty equivalent group; `unsubscribe` drops emptied groups.
- `fetch()`/`resolve()` parity across all three navigations.
- Phase 0–2: `transaction` rolls back via `Transaction` drop; `apply_schema_plan` re-checks catalog state inside the transaction and re-validates at each checkpoint; memory `candidates()` only over-approximates, `DropTable` clears indexes; `apply_batch`'s post-commit `public_row` is pre-guarded and `state.version == schema.version` in every `open_schema` path, so "committed but unjournaled" is unreachable.

### Your test-only edits — sound
`review_q01_owner_temp_window_intersection` does cross the threshold: 4105 membership operands + 1-id affected restriction = two intersecting TEMP sets, then a 4105-id TEMP materialization. `counts(...).is_empty()` is deterministic — at clock 16 the equality branch fires before the deadline branch. Case names match exactly and in order across `reactive-tests.js`, `worker.js`, `model_evidence.rs`; `review_conformance` bounds the index and returns `Err`.

### Two observations, NOT findings
1. Dropping `review_q01_large_result_sets_and_navigation` to 1105 means the ManyToMany-navigation membership set no longer crosses 4096 anywhere. Oversized two-set intersection is now covered only by the owner case (`where_in("id")`). Same `Plan` path and SQL shape, so coverage is equivalent — only predicate provenance differs. Flagged so it's a recorded choice, not an accident.
2. `worker.js` hardcodes `message.index < 5`. If the Rust `cases` array grows, the harness silently stops exercising the new case instead of failing. Driving the bound off the Rust list length self-corrects.

### Scope/limits
Host Rust only. Did not run browser WASM/OPFS evidence or the Swift bridge — the in-flight 20s/per-case-RPC split is yours to land, and the 60s aggregate was treated as fixture cost, not a product SLO. No cross-adapter differential harness (memory-vs-sqlite query matrix) — needs a dev-dependency edit, outside read-only scope; shared conformance modules compiled into both adapters already serve that role for existing assertions. Documented limits (full-row snapshots O(window), memory store clone, lifecycle materialization, unsupported pending-future cancellation, unbounded AND/statement complexity) treated as specified behavior.
