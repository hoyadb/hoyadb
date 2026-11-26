# Phase 6 lifecycle follow-up review

Claude Opus 5, existing agent `e6f3b33e-7787-43bb-904f-646b096c9154`,
reviewed the narrow late-open ownership fix after contract commit `c3dfcf8`.
The final response below preserves its execution and scope limits.

## Lifecycle delta review — PASS

Inspected only the `#opening`/`#closing` delta in `command-client.js:94-130, 372-409` plus the five new cases in `command-client-security.test.mjs:57-150`.

### State machine traced, no reachable defect

- **Reservation is synchronous.** `#open` sets `this.#opening = attempt` before invoking the caller's transport, and the `check(!this.owner && !this.#opening && !this.#closing,'Busy')` guard precedes it. Synchronous reentry — including a transport that calls back inline — sees the same attempt. A synchronous throw from `crypto.getRandomValues` or `call` is caught and rejected, and the `finally` releases the reservation, so a failed start never leaves the client permanently Busy.
- **Late handle cannot be published.** `#open` re-checks `attempt.cancelled` after `await received` and converts any outcome to `failure('Closed')`. The `finally` deliberately does *not* clear `#opening` when cancelled, handing the attempt to `#dispose`.
- **Order-independent, not order-lucky.** I checked the case where `#dispose`'s continuation runs before `#open`'s: `#dispose` clears `#opening`, then `#open`'s `finally` sees `this.#opening !== attempt` and no-ops. Both interleavings dispose the handle exactly once and still reject the opener. The handoff does not depend on which continuation was queued first.
- **Active-owner invalidation stayed synchronous.** `close()` calls `#dispose(attempt)` directly rather than awaiting first, so in the no-`attempt` branch `stopSync()`, `generation++`, `callbacks.clear()` and `job.cancel()` all still execute inside the synchronous prefix of the `close()` call, exactly as before the change. The next extracted-batch callback is still suppressed.
- **Failed disposal retains, does not orphan.** The `catch` in `#dispose` only clears `#opening` for a *failed open* (no handle exists to close). A failed `call('close')` propagates with `#opening` intact, so `#open` stays blocked with `Busy` and a later `close()` retries using the already-resolved `attempt.received`. Only the `close` op is resent — the key-bearing `open_secure` frame is never rebuilt or replayed.
- **Concurrent closes coalesce** via the synchronous `if (this.#closing) return this.#closing;` / `this.#closing = closing` pair, with `finally` clearing only its own promise (`this.#closing === closing`).
- **No key on the attempt object** — `{cancelled, received}` only; `key_hex` exists solely inside the `call` frame.

### Verification I actually ran

`node --test command-client.test.mjs command-client-security.test.mjs` → 23/23, and `node --experimental-vm-modules --test worker-errors.test.mjs harness-errors.test.mjs` → 15/15. That is the 38 Node tests, all passing, observed here rather than taken from the report. Test 7 genuinely distinguishes the shared retry (`closeCalls === 2`: one failure plus one shared retry, not three), and test 8 pins both open frames' path/key_hex/namespace, confirming no stale configuration replay.

### Limitations

- I did not run or inspect the browser worker suspended-reply or the command/entity browser regressions — still in flight under the browser agent; I make no claim about them.
- Scope was the lifecycle delta as instructed. I did not re-verify the Rust/engine layer in this pass, so my earlier crypto PASS stands on that earlier reading, not on a fresh check.
- The deliberate retain-on-failed-close means a client whose worker dies mid-disposal stays `Busy` with no reset path. That is the contract's fail-closed choice and the handle is unreachable anyway — noting it as designed behavior, not a defect.
