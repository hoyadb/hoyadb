# Benchmark lane functional evidence — not a baseline

Functional and calibration evidence for the TC-0037 benchmark lane on a
non-quiet host (other lanes active; load average above 100 during the cost
diagnostic). Every run here is `contaminated` or measurement-disabled. Nothing
in this directory is a latency, resource or budget baseline and nothing may be
pooled with a future accepted run.

## Contents

- `browser-verification-results.json`: actual installed-package browser
  verification with measurements disabled and a synthetic endpoint clock on
  browser closure `f8e1ac64…`: point and three one-observer reaction modes on
  both engines, all six declared cold classes (memory create; protected create,
  reopen N100/1000/10000, upgrade N1000 with post-open schema check) and both
  resource classes (fresh-memory-per-cycle; same-owned-store-reopen). All 17
  steps passed with SQLCipher4.19.0, pool `[24,0]` before/after, equal canary
  hashes, exact owned-name removal and absent closed journal/WAL.
  `measureUserAgentSpecificMemory` exists in the Paseo Electron host but rejects,
  so that metric is recorded unknown.
- `browser-admission/`: admitted contaminated integration (A08) on the same
  closure. An unpermitted page start was rejected without creating a worker;
  two cold memory-create workers and one memory resource run were each admitted
  by an explicit parent `ADMIT`, held after persisted raw and validated cleanup,
  and verified by the independent collector proof (`control.jsonl`, raw and
  accepted files, the parent control lines and the final manifest).
- `browser-prepare/`: final `--prepare-only` installer manifest on browser
  closure `027eeb44…`, including the actual package footprint (client tarball
  1481367 bytes, testing tarball 9465 bytes, installed files with fixed-mtime
  gzip sizes). The only change after the browser sessions is the manifest
  sample-count accounting for untimed resource runs.
- `native-calibration/`: real-runner manifests for the DEF-0020 cohort0/32
  label regression and the A08 native admission integration (contaminated, no
  summaries).
- `native-cost-diagnostic/`: bounded 10k memory reset diagnostic. Setup took
  33.3s and fixture validation 3.2s; the first reset sample (initial bootstrap,
  reset, reseed and restoring bootstrap) did not finish before the 1200s cap and
  was killed and reaped. No projection was possible. Completed interrupted-run
  workers (ccf8ebe) show memory reset workers of about 2.2–3.4s at N100,
  78–89s at N1000 and an incomplete 1487s at N10000, while the N10000 non-noop
  pull worker took about 34s. That superlinear growth in the bootstrap path is
  an unconfirmed hypothesis for the core owner, not a recorded defect.
- `lane-closure.json`: path, size and SHA256 of every lane file at HEAD
  `2603a747`, and the closure digest.

Raw command logs and intermediate RED/GREEN outputs remain under
`target/phase-seven-browser-bench/opus/`.
