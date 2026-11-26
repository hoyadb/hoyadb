# Final ordinary browser/OPFS acceptance checkpoint

Executed 2026-10-31 using the unchanged existing `tools/javascript/scripts/application-browser.mjs` and unchanged `examples/application.mjs`. This runner is invoked by `docs/public/verify.py` and recorded under its historical path in `docs/implementation/javascript-application-binding.md`. No runtime, test, source or generated-artifact edit; no rebuild, repack, dependency install or user-store access.

## Result

The exact runner passed on its second attempt, exit0 in1.75s, using installed Headless Chrome156.0.8078.79 and Node22.18.0. The original raw result is `attempt-2/browser.json`, raw process output `attempt-2/application-browser.log`, and command/cwd/cleanup record `attempt-2/execution.json`.

It executes the complete public Todo application in an actual dedicated browser worker: plain row queries, observation, async command handlers, throwing-handler rollback, fake backend sync, entity patches/inserts, and command-created ID mapping. It then checks missing-key rejection and a separate caller-key encrypted OPFS create/write/close/reopen fixture. The runtime returned encrypted=true, cipher=SQLCipher, version4.19.0, backup=unsupported.

The first unchanged attempt failed its existing five-second Chrome startup assertion, before any page/test execution. Its `execution.json` and `application-browser.log` remain intact. A separate launch with identical flags reached the DevTools endpoint after6.32s; `chrome-diagnostic.{json,log}` retains that observation. No timeout or assertion was changed. The unchanged retry passed; neither the initial failure nor diagnostic is counted as a product regression or passing acceptance result.

## Source/artifact and ownership closure

Both attempts used exact copies in task-owned work trees. All112 copied files (110 final dist members plus runner and tutorial) are unchanged before/after and still equal main. Every dist member matches the independently retained final ordinary web build `final-web-refreshed/final-generated.json`. The tested WASM SHA256 is `9a37bbce1b49df4109394786b3826338e027f33d4abbf38ac543c8a2422ef09c`, already bound by that build's 5,359-input closure. Its package manifest declares testControls=false. Root's187-entry final runtime freeze still matches main; the post-DEF0063 ledger helper source is included.

`tools-before.json` and `tools-after.json` bind Node, Chrome launcher, Chrome Info.plist and its actual framework binary; their hashes were rechecked after execution. Browser.getVersion supplies the actual connected browser revision. Each exact runner creates its own OS temporary profile and ephemeral loopback server, then closes Chrome/server and removes the profile in finally. The external capture saw both task profiles and records no remaining profile. The separate diagnostic profile was also removed. The original final dist/WASM/tarball artifacts were not overwritten.

## Limits

This is one installed Chrome macOS headless runtime checkpoint, not cross-browser/device certification. It proves the application tutorial and encrypted OPFS close/reopen boundary, not the entire PhaseSeven browser matrix, reload persistence, tamper/security matrix, concurrent-worker retry, or RN/mobile runtime.

The broader `tests/packaging/javascript/browser.mjs` and `worker-retry.mjs` require a separately current test-controls artifact even for cleanup. The documented packed `--serve` path also repacks artifacts. Those were not run, and stale `pkg-test` files were not used. Existing final ordinary artifacts and evidence remain intact.
