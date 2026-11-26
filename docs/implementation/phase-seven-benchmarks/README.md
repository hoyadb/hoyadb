# Phase 7 native benchmark preparation and historical pilot

The native Rust-facade worker and independent Python collector are implemented.
This checkpoint is not the full TC-0037 baseline or a numerical budget.
[The evidence index](checkpoint.json) preserves actual RED logs, current
functional outputs and the complete historical point-query pilot.

The worker executes the real memory/protected adapters through the shared
facade. Dataset A, complete typed before/after images, command outcomes, entity
and pull changes, queue retry state and reset effects are checked. Initial and
final actual public rows cross a bounded proof protocol to an independent
Python literal oracle; only counts and hashes are retained. The runner rejects
changed sources/artifacts, incorrect proofs, debug profiles, mixed identities,
failed/contaminated samples, missing runs and overwrite attempts. Retained RSS
uses a separate untimed 30-cycle worker; it does not claim allocator leak proof.

Root observed seven Rust library tests plus the debug-profile guard pass,
12 Python runner tests and nine summary tests pass. The 136.31-second debug
library run occurred while other engineering work was active and is **not a
latency measurement**. Other browser-controller tests belong to the browser
lane; an oversized-body socket race found during broader discovery is recorded
in that lane and is not silently included in these native totals.

The earlier quiet pilot used the optimized ordinary artifact
`8f6cd610433df3918ca1fa1a0ca9d2a94a20f6ab3bf59d89796f0023bc930ab5`
(6,941,560 bytes), point queries at N100 and Q0, five independent processes per
class, five warmups and 30 measured samples per process. Both classes passed
all independent proofs; source/artifact hashes stayed stable and owned stores
were removed after children exited. Task-owned agents paused heavy work;
external background activity remained unknown. Full environment and per-run
samples remain in [the exact pilot capture](pilot-point-n100/manifest.json).

| Rust-facade class | Measured samples | Median of run medians (ns) | Run-p95 range (ns) |
| --- | ---: | ---: | ---: |
| Memory | 150 | 37405.5 | 38255–73481 |
| Protected native SQLCipher | 150 | 156437 | 191847–305177 |

These values describe this pilot machine and endpoint only. They are not mobile
or browser SDK latency, a full baseline, a regression limit or a product promise.
The pilot records the original workload SHA256 beginning `11e120b0`; the later
`d8251bd` change only documents the existing extrema fixture precisely. Historical
captures are not relabelled with that new source hash or pooled into later runs.

Repeat functional checks from the repository root:

```sh
cargo test --manifest-path benchmarks/native/Cargo.toml --offline --locked
python3 -m unittest discover -s benchmarks -p test_runner.py
python3 -m unittest discover -s benchmarks -p test_summary.py
cargo fmt --manifest-path benchmarks/native/Cargo.toml --check
cargo clippy --manifest-path benchmarks/native/Cargo.toml --all-targets --offline --locked -- -D warnings
```

A future accepted full matrix requires a coordinated quiet window, unchanged
current source/artifact hashes, all declared warm/cold/resource workloads,
repeatability review and explicit measured-budget review under TC-0037.
The runner defaults to `--run-status contaminated`; it does not quietly promote
engineering runs to accepted performance evidence. No package was published.
