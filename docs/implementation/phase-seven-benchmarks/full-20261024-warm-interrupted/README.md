# Interrupted native warm matrix — not a completed baseline

The original runner exited1 with manifest status `failed`. Its145 completed raw
workers (memory cohorts00–28,5 runs each) are retained unchanged. The next worker,
memory29/run1 (10,000-row reset), was explicitly interrupted with SIGTERM while
CPU-active after more than24minutes. The parent recorded exit-15, incomplete
worker protocol and failed raw status, reaped the child and removed its owned
temporary directory. No protected-native warm worker had started.

The decision frees the host for SDK correctness/packaging work after the reset
restoration cost greatly exceeded the earlier point-query pilot. It is an
operational interruption, not evidence that the reset call itself took that
long: setup/restoration is outside its timer. The worker keeps samples in memory
until final proof/report; its partial sample vector was not emitted and is
unavailable. No duration or percentile is inferred or fabricated for it.

No sample count or matrix size was reduced. All original configurations, raw
files, build output, console and manifest are retained. `interruption.json`
records exact owned commands/PIDs, control timestamps,145 completed raw hashes
verified unchanged, cleanup and the post-exit source/artifact equality check.
The raw `run_status:ok` field was configured before launch; it does not override
the failed experiment status or constitute complete baseline acceptance.

Source closure:6deedce1eea57e0562965cf8afcc2aa784557d0eb33a2b07136c20fcb1ed65a3.
Artifact:d50bf0e9b76bfae263022d812e113b6943a1d7f704c6a3c80c2e56614981ee93.
Workload:e1963c2f3a94c6efc45345a4ae0855805c3f62582e29d794840f1a8e5103e82d.

No accepted summary was written. Full warm acceptance, same-class repeats,
reactive endpoint correction (DEF-0020), browser/language metrics and numerical
budget review remain open. A future clean affected cohort needs all5 complete
independent runs. Paused/failed and clean samples must not be silently pooled.
The already completed cold/resource experiment is a separate evidence class.
