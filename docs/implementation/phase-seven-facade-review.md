# Claude Opus shared-facade review on 02621eb

Read-only checkpoint before DEF-0009/DEF-0010 regression fixes. This is not the
final Phase 7 cross-review. Subsequent reproduction and verification are recorded
in [reply-budget resolution](phase-seven-reply-budget-resolution.json).

## Phase 7 shared-facade checkpoint — 1 confirmed finding, 1 narrow note

### Finding 1 — `budget::batch` over-rejects same-table batches (medium, availability; no data loss)

`crates/hoyadb-bindings/src/budget.rs:81-98`

The conservative before-image array collects **every prior after-image for the same table**, filtered by `name == table` only (`:86`), not by row id. For a batch of B writes to one table, change *k* accumulates ~*k* candidate images, so the sized frame grows O(B²) while the real reply is O(B).

I confirmed this rather than estimating it. A probe reconstructing `data.rs:112-122`'s `row_wire` shape and running core's own `Json::canonical()` (the same limiter `budget::success` uses):

```
batch=100 sizing_accepts=true  real_reply_fits=true real_bytes=35271
batch=128 sizing_accepts=false real_reply_fits=true real_bytes=45239
batch=256 sizing_accepts=false real_reply_fits=true real_bytes=90807
```

**Trigger:** 128 `replace` writes to one table, 3 string fields each. **Impact:** `LimitExceeded` before mutation, while the actual reply is 45 KB — about 4% of the 1 MiB budget and ~3% of the 100,000-node cap. Callers hit a ceiling roughly 30× tighter than API-0011 §52-59 documents.

The direction is safe (rejection precedes mutation, no false accept), and the table-level filter is *necessary* for inserts whose id is core-allocated and unknown at preflight. But it over-applies to keyed writes.

**Shortest correction:** at `:83-88`, include a prior after-image only when its id is the placeholder (core-allocated insert) or equals the current write's id. Keeps the bound sound, collapses the quadratic term to linear for keyed batches.

**Test gap:** `tests/general.rs:268` (`g11_batch_reply_overflow_...`) covers a single 600 KB before-image, not the accumulation path.

### Finding 2 — `prefix()` can livelock on a byte-exact frame (low, narrow)

`crates/hoyadb-bindings/src/commands.rs:144-165`

The loop sizes each candidate with `has_more = true` (`:153`), but the emitted frame uses `count < pending.len()` (`:162`). When everything fits, that flips to `false`, one byte longer than `true`. The re-check at `:163` catches it, so nothing overflows and nothing is dropped — but `pending.drain(..count)` never runs, so an identical retry fails identically and the queue cannot drain.

**Trigger:** a full drain whose frame lands exactly at `MAX_BYTES` with `"has_more":true`. Narrow enough that it needs a crafted payload; I did not construct one. **Correction:** on failure at `:163`, pop one value and re-frame before returning.

### Checked clean

- **Passive merge stale/busy/retirement.** `merge.rs:148-160` guards on model, schema, full ledger equality, `persisted_rows` and a re-taken storage snapshot — strictly stronger than `validate_pull_request`, since entity generation/revision/checkpoint live inside the ledger. Every route that clears `self.pull` also clears `self.merge` (`pull.rs:305,337,434`, `entity_owner.rs:88-89`), and `apply_pull` returns `Busy` while a session is live (`pull.rs:354`), so no session can commit a stale page under a replaced ticket. `merge_prepare` rejects tables that already carry a Rust hook, and `merge_reply` takes metadata from the core default only — no host LWW or clock editing, matching SPEC-0033 §5.
- **Entity-normalized reply sizing placeholders are exact, not guesses.** `budget.rs:23,43` uses `entity:<32 f>:<16 f>` = 56 bytes; core generates exactly `format!("entity:{}:{counter:016x}")` with a 32-hex namespace (`entity_owner.rs:261,423`), and ordinary ids are `local:{u64}` ≤ 26 bytes. `commands.rs:300`'s `"0".repeat(66)` matches the 66-byte command key. No under-estimate, so no post-commit `LimitExceeded`.
- **Owned-prefix liveness otherwise correct.** A single oversized item returns `LimitExceeded` with `count == 0` and stays pending (`:155-157`) — API-0011 §56-58, no drop or truncation.
- **Protected open before effects.** `lib.rs:251` runs `checked_open` (pure frame + key-length validation, memory 0 / native 32 / opfs 32 at `:212,224,233`) before anything else, and re-validates at `:316-317` before touching a path. Keys arrive as `Zeroizing<Vec<u8>>` (`:163`) and never enter the JSON frame.
- **Engine forwarding.** `engine.rs:15-21` polls once and maps `Pending` to a static payload-free failure. It surfaces as `BackendFailure` rather than `UnexpectedPending` because `StorageError` has no such variant — forced by the type, and the facade's own `UnexpectedPending` at `lib.rs:100` covers the contract-level case. Not a defect.

### Qualifications

Phase 7 is **not** certified and this is not a final review. I inspected the core seams, `crates/hoyadb-bindings` and the budget/codec/engine paths; `bindings/web`, mobile, JS and benchmark lanes were out of scope and I make no claim about them. I ran no repo test suite this pass — the 372/21-pass and Clippy/fmt/trace-green results are the runners', not observed here. My one probe was a standalone `/tmp` crate against `hoyadb-core` only; both it and the earlier `target/` scratch are removed, and I edited nothing in the repo.
