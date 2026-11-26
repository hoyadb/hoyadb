# Claude Opus native and lifecycle checkpoint

This is the completed read-only checkpoint, not final Phase 7 acceptance.
The 83-test JavaScript workspace consists of 80 client tests plus 3 testing-package
tests; the reviewer ran the client subset. DEF-0015 external receipt wake was
independently reproduced during this work and remains outside this checkpoint's
notification-only helper assessment. Its correction still requires verification.

## Phase 7 checkpoint — PASS, no new findings

### DEF-0009 / DEF-0010 fixes (294ef85) — both correct

`budget.rs:34-99` replaces the table-filtered `prior` accumulator with `known: BTreeMap<(table,id), Option<Row>>` plus `unknown: Vec<(table,Row)>` gated on `allocated = Insert && proposed.is_none()`. I traced the soundness cases:

- Insert with `proposed_id` → `known`, so a later replace of that id sizes against exactly one staged image.
- Remove → `known.insert(.., None)` → `row.iter().cloned()` yields an empty candidate set, which is the real before-state.
- Core-allocated insert → stays in `unknown` and is still chained by table, because its id genuinely isn't knowable at preflight.

No under-estimate in any path, so the §54 guarantee ("no successful commit becomes LimitExceeded while serializing") still holds, and the quadratic term is now confined to mixed allocated-insert + keyed-write batches where it is irreducible. That residual is correct behavior, not a defect.

`commands.rs:153` now sizes with `count + 1 < pending.len()`. Traced both exits: full-drain ends with the last check at `i = len-1` using `false`, matching the emitted `count < pending.len()`; the break path's last successful check used `true`, also matching. Sized and emitted frames are now always byte-identical, so the livelock window is closed and `:163` degrades to an assertion.

Regressions are real and targeted — `g11_keyed_batch_budget_excludes_unrelated_rows` (+scope/entity variants), `g11_fitting_large_keyed_batch_does_not_accumulate_other_row_images`, `g11_unknown_allocated_before_image_stays_bounded_and_counter_unchanged`, `g11_byte_exact_terminal_prefix_makes_progress_without_dropping_events`, and `g11_undeliverable_single_snapshot_remains_until_cancellation` preserving §56-58.

### Native C / JNI foundation (6ab784d) — no demonstrated issue

- `src/lib.rs:121-147`: the `MutexGuard` is scoped to the inner block and dropped before the reply callback at `:145`, so no registry lock spans the collector — API-0011 §85 satisfied by construction, not by comment. Poisoned lock returns status 2 permanently (fail-closed).
- Validation order in `request()` is correct: reply-null → frame length/null → **key size/null** → only then the json dereference at `:116`. The SAFETY comment's claim at `:131` is accurate.
- The key `Vec` from `.to_vec()` is *moved* into `facade.open`, which wraps the same allocation in `Zeroizing` — no second un-wiped copy.
- `hoya_v1_validate_open` never touches `RUNTIME`, and `readonly_preflight_leaves_process_registry_uninitialized` asserts `RUNTIME.get().is_none()` before and after. That's a proof, not an assertion of intent.
- `hoya_jni.c`: double-reply, null-span, oversize and malloc failure all set `failed` and are caught at `:83`; `output.bytes` is freed on every exit path; `copy()` returns NULL with a pending exception on failure so `owned` is never double-freed; negative or out-of-range `validation_length` is rejected by `key_length != 0 && key_length != 32` at `:63`. The temporary key copy is wiped through a `volatile` pointer at `:77-81`.
- `exports.map` exposes only the eight intended symbols with `local: *`, so LTC/SQLCipher internals and the entropy adapter stay hidden.
- `build-provider.py:59-62` asserts `sources.count(excluded) == 1` and no duplicates before removing `libtomcrypt/prngs/rng_get_bytes.c`, and `:58` hashes every extracted source. Upstream drift fails loudly rather than silently linking both definitions — this closes the recipe concern from my earlier entropy review. Neither build script defines `HOYA_ENTROPY_TEST`.

One hardening nit, explicitly not a finding: `copy()` at `:50` frees a partially-filled buffer without wiping when `GetByteArrayRegion` throws, which for the key path would be an unwiped partial key. Unreachable in practice — the region always spans the full array length, so it cannot throw `ArrayIndexOutOfBounds`. Smallest correction if wanted: wipe before `free` in that branch.

### JavaScript lifecycle (02b64d1) + DEF-0012 helper — correct

- `#listenRelease` subscribes before admission, is idempotent via the `active` flag, registers in `#releaseSubscriptions`, and is retired from `run()`'s `finally`, `resumeOwned`'s `finally`, `reset()` and `close()`. The release callback re-checks owner *and* generation before firing.
- `resumeOwned` now re-validates owner/generation after `operation()` resolves and returns `null` on replacement — that suppresses stale publication without re-running the transport or mapper, and the durable effect already committed.
- `sync()`'s `catch` guards on `live()` before pushing to `syncErrors`, which is the DEF-0011 stale-error fix.
- The DEF-0012 helper (`#deliverSyncMutation`, uncommitted, `client.js:254-261`) guards on `generation === this.generation && this.callbacks.size`. I checked the one case that would make that guard wrong — a consumer with only `observeStatus`/`observeProgress`/`observeOutcome` and no data query. `subscribe()` at `:295` puts **every** subscription into `this.callbacks`, with `syncSubscriptions` only tracking tokens for unsubscribe routing, so sync-only subscribers are covered. No lost transition. Skipping `wakeSync()`/`scheduleCount()` is right here: `preparePush` runs inside the pump, and these ops change sync record state rather than row data.

### What I actually ran (all before the quiet-window request)

- `cargo test -p hoyadb-bindings --offline --locked` → 28 passed, 0 failed (7 entities + 21 general).
- `node --test test/*.test.mjs` in `bindings/javascript/client` → **80 passed, 0 failed**. Note the brief cited 83 Node; the package's own test glob yields 80, so the extra 3 presumably live outside `client/test/`. Minor provenance discrepancy, not a failure.
- I did **not** rerun the native nine this pass, nor any browser/OPFS/WASM lane. Those remain the runners' results.

### Provenance (reviewed files, sha256)

```
4cfa3f57…9a47  bindings/native/src/lib.rs
e620c700…0265  bindings/native/android/hoya_jni.c
c8b6059f…0c75  bindings/native/android/hoya_entropy.c
41230e75…19a1  bindings/javascript/client/src/client.js   (uncommitted DEF-0012 state)
8efe83e4…5e15  crates/hoyadb-bindings/src/budget.rs
d054b96d…c215  crates/hoyadb-bindings/src/commands.rs
```

### Scope

This is a checkpoint, not Phase 7 acceptance. Kotlin/RN/Swift/benchmark implementation and DEF-0013/0014 were excluded as instructed. No edits, no commits, nothing running now — clear to start your timed samples.
