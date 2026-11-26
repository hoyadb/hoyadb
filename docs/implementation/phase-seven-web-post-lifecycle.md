# JavaScript SDK matching runtime verification

The matching local packages pass after the lifecycle/release changes in02b64d1
and DEF-0012 managed sync delivery correction. The exact source/artifact matrix,
all case results, commands, archive hashes, raw-log hashes and cleanup record are
in [the evidence JSON](phase-seven-web-post-lifecycle.json).

Verification passed:

- 83/83 Node tests,49 source JavaScript syntax checks and3 fixture HTTP tests.
- External packed npm consumer: strict TypeScript,15 real WASM memory groups,
  all16 TC-0016/17 walkthrough cases and the exact guide example (exit0).
- Ordinary protected browser:35/35 groups across a real page reload, including
  both new prepare/receipt managed delivery cases, unchanged exact G11 bounds,
  wrong-key rejection, offline behavior, migrations and durable frozen retries.
- Instrumented security:9/9; transient competing-worker initialization retry:1/1.

The runtime interval preserved **195 source hashes and8 generated artifact
hashes**. The optimized WASM artifacts are unchanged from the preceding release
matrix; these corrections change JavaScript only. Controlled Channel regressions
pin stale callbacks/errors and cross-wrapper release retry. Actual native/RN
broker evidence remains in those target reports. No mobile or performance result
is inferred from this browser execution.

All three owned tabs were closed. The exact packed-consumer server processes,
temporary root and port8878 were removed/released. Harness assertions verified
exact owned logical-file removal, unchanged neighbor canary bytes and pool
assignment baseline. Numeric pool values were not emitted, so no numeric value
is invented in this report. The ordinary and instrumented artifacts remain
separately identified. No whole-origin or shared-pool deletion occurred.

The earlier [web runtime](phase-seven-web.md),
[82-test lifecycle](phase-seven-web-lifecycle.md) and
[managed sync checkpoint](phase-seven-web-managed-sync.md) retain their original
chronology. Full [Node output](phase-seven-web-post-lifecycle-node.txt),
[external consumer log](phase-seven-web-post-lifecycle-package.txt) and
[verification output](phase-seven-web-post-lifecycle-verification.txt) are kept.
Browser benchmark implementation and measurement remain separate pending work.
