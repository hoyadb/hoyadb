JS policy and compatibility-checker review complete. Snapshot (sha256 prefixes): `client.js d5381a5a`, `application.js dcde726a`, `index.d.ts 98be4a24`, `application.d.ts 6184c53e`, `conflict-policy.test.mjs d9cdcb0f`, `conflict-policy-wasm.test.mjs e446b0ad`, `guard-sdk be54f979`, `guard-gw16b 915faf8d`, `stale-sync 5e22f0fd`, `compatibility.py 149f11a4`, `test_compatibility.py ccc46f46`, `swift-accepted-changes.json 59502fdf`. Observed myself: 40 host tests pass across conflict-policy, guard-sdk, guard-gw16b, stale-sync, lifecycle; 20 compatibility checker tests pass; `compatibility.py sources` fails only because `tools/release/current-api.json` does not exist yet (expected per root).

## Verified correct

- **Unpublished configuration.** `attempt.ready` (`client.js:71-75`) chains `set_conflict_policy` on the private handle; `#open` publishes only after `ready`. `close()` marks the attempt cancelled, `#dispose` awaits `ready` then `received`, closes once (`:524-539`). A `check(!cancelled)` before and after the setter means a close that lands before the native open never sends the policy call. Covered by CP13 tests 1-3.
- **Failed cleanup retention.** `#open` catch calls `close()`; a failed native close leaves `#opening` set because `close()` set `attempt.cancelled` before `#open`'s finally runs (`:74`), so reopen is `Busy` and a later `close()` retries the same owner. Test 'failed configuration close retains the same private owner' pins owner `'1'` twice.
- **Resolving/legacy.** `reconcile`, `reconcileGuarded`, `rollback` route through `#resolve` and return `null`; `reconcile_guarded`/`reconcile`/`rollback` wire ops no longer sent by JS. Fixture routing updates are consistent.
- **Decoder.** `resolution()` (`:558-571`) enforces exact key sets, string arrays, `unsigned(...,64)` (no leading zeros, ≤ u64 max), deep-frozen proxy via `immutableContext`; any failure → `InvalidPayload` before callbacks.
- **Callback containment.** `Promise.resolve(cb()).catch(failed)` plus outer try; hostile thenable settles once; failure reported via `#report('callback','CallbackFailure',null,generation)` only when `live()` at rejection time, so old-epoch late rejections are suppressed. Never touches `syncErrors`. `Completed` never notifies.
- **Routing.** `openApplicationDatabase` spreads options into the host and the `Database` constructor whitelists `conflict`/`onResolution` (`client.js:35`), so application explicit calls, application unit calls and background `pushPass` → `raw.reconcile` all pass through one `#resolve`. `ApplicationOpenOptions extends Omit<OpenOptions,…>` keeps `conflict`/`onResolution` typed. RN `index.d.ts` re-exports `@hoyadb/client/host` types, so the new types reach RN.
- **Checker.** History digest pinned and matches (`5d6091ba…`). Accepted Swift list is exactly the SPEC-0036 enum addition; `check_swift_result` requires each accepted line verbatim and fails on any other `API breakage:` or unrecognized text; zero exit cannot pass alone. Export checks fail on tool failure, malformed lines, missing or unexpected symbols. Candidate generation cannot overwrite or write into retained history.

## Findings

**1. LOW — `conflictPolicy()` passes an invalid `default` straight to the facade.** `client.js:553-557` only checks object shape and key names; `{commands:{}}` or `{default:'maybe'}` reach `set_conflict_policy` and fail with `InvalidRequest` from the facade. Fail-closed, so acceptable, but for the constructor path this happens inside the open IIFE after schema copy and before native open, which is fine; document that `default` is required at the SDK layer too (index.d.ts already types it as required). No code change needed.

**2. LOW — close during a hanging `set_conflict_policy` blocks `close()` indefinitely.** `#dispose` awaits `attempt.ready` with no cancellation (`:527`). Consistent with the DEF-0044 rule that disposal waits for admitted host calls, and the facade setter is a local synchronous core call, so unreachable in practice. Note only.

**3. LOW / test gap — CP13 "reconfiguration during an admitted handler returns Busy without changing policy" has no JS case.** Core enforces it; a host test with a `bad('Busy')` reply asserting the prior constructor policy is reapplied on reopen would pin host behavior cheaply. Also the WASM suite could assert it directly once the fresh build lands.

**4. LOW — `-swift-version 6` in `check_swift` (`compatibility.py:150`).** `Package.swift` declares `swift-tools-version: 6.0` with no explicit language mode, so Swift 6 mode is the package default; consistent. If the package ever pins mode 5 the digester comparison would produce spurious diagnostics, failing closed. Note only.

**5. INFO — `SOURCE_PATTERNS` includes `packages/react-native/src/index.js`.** Any implementation refactor forces a manifest review. Intentional per README; keep but expect churn.

**6. INFO — `current-api.json` absent, so `compatibility.py sources` fails today.** Root states the RN manifest is intentionally pending the final declaration review; the tests use a fixture manifest, so the gate is not vacuous. Must exist before the gate is cited as passing.

## Not reviewed

`conflict-policy-wasm.test.mjs` results (fresh WASM pending; file read only, cases align with CP01/02/04/07/09/13), RN package source changes, Swift sources. Final freeze re-review needed for `client.js`, the two tests, and the `current-api.json` manifest once written.
