# DEF-0029 browser start preflight — RED/GREEN evidence

This is synthetic controller evidence only. Only stdlib tests and temporary
directories ran: no browser, SDK, worker, build or measurement. It covers
[DEF-0029](../../../design/defects/DEF-0029-browser-start-plan-failure-state.md)
at HEAD `e63ceb2`. Earlier source cuts and captured evidence are unchanged and
remain valid for their own runs. This cut supersedes them for future runs only.

## Change

`benchmarks/browser_server.py`: `consume(coordinates, plan)` now does its work
in the reviewed order, under the existing collector lock:

1. a non-mutating exact permit match. A mismatch is the existing non-blocking
   rejection, and the permit is kept.
2. the existing source/artifact drift recheck. On drift it clears the permit,
   restores the pre-admission state and blocks.
3. the start route's non-mutating plan. A failure is recorded as a non-blocking
   `rejected` event, with no start and the permit unconsumed.
4. only then the consumption and start-generation commit, with the DEF-0023
   waiter notification.

`start`, `start_cold` and `start_resources` pass their existing plan to it. No
worker clock, workload, count, cleanup rule, SDK or captured evidence changed.

## Tests (`benchmarks/test_browser_server.py`)

- P01, for the warm, cold and resources routes: the expected output file appears
  after admission. The start rejects, and permit, state, start generation, runs,
  workers, active run, claim and blocked list are all unchanged. No `start`
  event is logged and the file bytes are preserved.
- P02, for each route: after that rejection, REVOKE then RECOVER, then a
  different valid identity starts exactly once. A second start rejects. The
  conflicting file is byte-identical, and there is one `start` event. Resource
  runs are declared as run 1 only, so their different identity is the other
  engine.
- P03, for each route: a mismatched start keeps its permit unchanged. Drift
  clears the permit and blocks, with no run, worker or start, both with and
  without a conflicting output file. Drift wins in the combined case.

## Results (normalized logs in `logs/`)

- RED (`90-def29-red.txt`): P01 failed on all three routes with permit null,
  state active and starts 1. P02 errored on all three with `no unconsumed permit
  to revoke`, which is the defect itself. P03 passed, as a guard for the existing
  mismatch and drift semantics.
- After the fix, P02's resource case first errored because the test asked for
  run 2, which resource runs do not allow. The test's identity choice was
  corrected to the other engine; no assertion was removed. This did not affect
  the RED: P02 failed earlier there, at `revoke`.
- GREEN (`91b-def29-green.txt`): 3 tests passed.
- Full benchmark Python discovery: 93 OK normally (`92-def29-python.txt`) and
  93 OK under `-O` (`92-def29-python-O.txt`). The named A01–A08, N01–N08,
  DEF-0023 and DEF-0029 cases were also run as a group: 32 passed.

The logs replace the repository root, OS temporary paths and other home paths
with placeholders. `log-provenance.json` maps each original log's SHA256 to its
derived SHA256. The originals stay local, outside version control.
`source.json` records the changed source hashes and the previous cut.
`files-sha256.json` indexes every file here except itself.
