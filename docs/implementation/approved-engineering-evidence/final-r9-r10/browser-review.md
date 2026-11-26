# Final browser/OPFS acceptance checkpoint — read-only review (Claude Fable 5.1)

Date: 2026-10-31. Packet `/tmp/hoyadb-followup-20261031/final-browser/packet.json` `e3033316…5b9ee`
(matches). No repository edit; nothing rerun by me. Scope per `REVIEW.md`: one installed headless
Chrome on macOS running the unchanged `application-browser.mjs`; not cross-browser, reload,
tamper/security, worker-retry or mobile certification.

## Source / artifact / result linkage (verified)

- `recorded_files` (17) all match on disk; `inputs-before == inputs-after` (112: 110
  `tools/javascript/dist/**` members plus `examples/application.mjs` and
  `tools/javascript/scripts/application-browser.mjs`); every one of the 112 equals current main
  bytes. All 110 dist members are byte-identical to `final-web-refreshed/final-generated/dist/**`
  (the inventory bound in my web review), and `build_packet.sha256` equals that packet. The WASM
  under both work trees is `9a37bbce…`, the bindgen output of the 5,359-input web build;
  `run.py` asserts the staged WASM equals the checkout copy.
- `runtime-freeze.json` `8549dc12…` (root's final 187 map, post-DEF-0063) has no drift against main.
- `tools-before == tools-after` (Node `v22.18.0` binary, Chrome launcher/framework hashes);
  `attempt-2/browser.json` records `Browser.getVersion` → `Chrome/156.0.8078.79`, revision
  `@a21ce95c…`, status `passed`, four checks, protection `{encrypted: true, cipher: SQLCipher,
  version: 4.19.0, backup: unsupported}`; `attempt-2/application-browser.log` ends with
  `PASS real Chrome application tutorial + protected OPFS reopen`; `attempt-2/execution.json` exit
  0 in 1.75 s, one task profile seen, none remaining.
- The runner (`application-browser.mjs:8-42`) creates its own `mkdtemp` profile and loopback
  server and removes the profile in `finally`; the `missing key` check requires
  `InvalidRequest` before storage open (line 35) and the protected reopen requires the row and
  `protection.encrypted` (line 38). No assertion, timeout or source differs from main.

## First attempt and diagnostic

`execution.json` (root) records attempt 1 exit 1 after 5.52 s with
`AssertionError [ERR_ASSERTION]: installed Chrome starts` — the runner's existing 100 × 50 ms
`DevToolsActivePort` poll (line 21), before any page load. `chrome-diagnostic.json` shows the same
flags reaching the DevTools endpoint after 6.32 s; its profile was removed (`exit_after_cleanup: 0`).
The unchanged retry passed. Correctly classified: an environment startup latency, not a product
result, and not counted as a pass. The 5 s ceiling is pre-existing and was not touched.

## Findings

None actionable. Scope notes:

1. **INFO.** The startup assertion has now flaked once on this host (5.52 s vs 6.32 s to
   endpoint). Not a defect in the product; if it recurs, the fix belongs to the runner's poll
   budget, with its own change record, not to this evidence.
2. **INFO.** `docs/implementation/javascript-application-binding.md:26` still cites the historical
   `bindings/javascript/scripts/…` path; `docs/public/verify.py:86` uses the live path. The
   report already notes this.

## Limits

Not rerun by me. Broader `tests/packaging/javascript/browser.mjs`, `worker-retry.mjs` and the
packed `--serve` path were not executed (they need a fresh test-controls artifact), as stated.
