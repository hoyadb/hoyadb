# Native memory point cohort 0 pilot — contaminated, not a baseline

A root-authorized pilot of the unchanged native warm protocol for memory
cohort 0 (point, N100, Q0): five independent workers, each with five warmups
and 30 measured samples. It ran on 2026-10-25 from 20:33:54Z to 20:34:44Z with
`--run-status contaminated --admission stdin`. The runner wrote no summary.
This directory contains no accepted summary, baseline or budget. These data
must never be pooled with a full baseline or with earlier experiments.

## Why contaminated

Quiet attestation was false. External processes the lane did not own were
present and were left untouched:

- Codex/ChatGPT application processes, including pid 80165, which was at 100%
  CPU before GO and sampled at 0.0% during the pilot;
- a Metro bundler and node workers for an unrelated user project (idle at samples);
- a booted user iPhone 17 Pro simulator;
- a Google Chrome renderer at up to about 33% CPU;
- WindowServer, sysmond and Paseo renderer activity.

The one-minute load average was about 4.2–4.5. `pmset` reported no thermal or
performance warning, `CPU_Speed_Limit` 100, AC power and a fully charged
battery. No hoyadb build or test was seen while workers ran. Before GO, the
Swift/RN/native lane reported itself held.

## Chronology and integrity

The parent issued exactly five one-use permits (`capture/parent.control`).
Each answered the currently requested `AWAIT_ADMISSION` coordinates, and each
followed the previous worker's `BETWEEN_WORKERS`, which the runner emits after
the `os.wait4` reap and after the validated raw report has been persisted.
Before each `ADMIT`, the driver (`driver/admit-one.sh`) checked that no child
process existed and that the held and await counts were exact. It recorded the
environment before admission and after each worker (`driver/observe.sh`,
`capture/parent.observations.txt`), outside every sample clock. The manifest's
admission events are admit/between_workers five times. No permit was
pre-issued, and no CANCEL or SIGTERM was needed; the 10-minute per-worker cap
was never approached.

The source snapshot before, after and on recheck was
`8c4a89c2a24f10ed5415aec8cdf1689554cf699bbff2b7c865e2f04f104ac57a` (98
files). The artifact `hoyadb-benchmark` was
`d50bf0e9b76bfae263022d812e113b6943a1d7f704c6a3c80c2e56614981ee93`, and the
runner also rechecked it before every admission. The build exit code was 0.
Every worker exited 0 with raw status ok. Each initial and final independent
row proof had 112 records, and the two were equal. The owned temporary
directory was removed. All owned processes were reaped before handback.

## Cost

The total command wall time was 50 s. That figure includes the no-op release
build check and manual parent admission handling. Each worker's launch-to-exit
time is the per-worker cost: 0.80, 0.14, 0.21, 0.15 and 0.16 s.

## Observed distribution (contaminated pilot; ns; 30 measured samples each)

| Run | Median | p95 (nearest rank) | Min | Max |
| --- | --- | --- | --- | --- |
| 1 | 56894 | 59970 | 55641 | 62940 |
| 2 | 48034 | 54654 | 47494 | 57778 |
| 3 | 56742.5 | 59651 | 55539 | 74306 |
| 4 | 50987.5 | 57044 | 47894 | 68972 |
| 5 | 54518 | 72636 | 52826 | 73816 |

These are observations only, not repeatability acceptance or a budget input.
Peak RSS reported by `wait4` was 11.2–14.0 MB per worker.

## Files and their classes

Byte-identical benchmark records: the files listed under
`byte_identical_to_original` in `provenance/derivation.json`. Every config,
raw report, sample, proof count, `manifest.json` and `build.txt` in
`capture/` is a byte-identical copy of the original capture at
`target/phase-seven-benchmarks/pilot-native-memory-c00-20261025T203349Z`. So
are the parent console, the exact control lines and the wall timestamps, the
driver and observer scripts, and the pre-GO hash record.

Sanitized environment views:
`capture/parent.observations.txt` and
`driver/81-pilot-environment-before-go.txt` are derived, line-preserving
views, not byte-identical copies. In process lines they keep PID, %CPU and
elapsed time and replace external command lines with marked
`[sanitized: …]` categories. Listener names and addresses and the simulator
identifier are also replaced. Timestamps, load averages, pmset, power, CPU and
memory lines, and the admission/worker chronology are unchanged.
`provenance/derivation.json` maps each original SHA256 to its derived SHA256
and states the deterministic transformation and the hash of the
sanitizer script. The sanitizer (its match patterns name unrelated projects)
and the unsanitized originals stay local, outside version control. This README is a revised authored text; its earlier
version's SHA256 is recorded in the provenance file.

No secret-scanning guarantee is made. The derivation removes the unrelated
command lines rather than certifying their absence elsewhere.

`files-sha256.json` lists the SHA256 of every file here except itself.

## Not covered

The browser package footprint and browser source cuts recorded earlier remain
historical. A declaration-only client repack (DEF-0026) is upcoming, and any
new browser evidence must first align with the exact new package identity. No
further measurement may start without an explicit root GO and the full protocol
and evidence requirements.
