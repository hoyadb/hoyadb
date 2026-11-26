# First full native cold and resource execution

The reviewed native cold and untimed-resource matrices passed on the ordinary
optimized Rust-facade artifact. This supplements the historical point pilot;
it does not complete the warm baseline, SDK measurements or budget review.
[The evidence index](cold-resources-checkpoint.json) retains exact raw results,
configuration, source/artifact hashes, environment and cleanup manifests.

All three task-owned engineering lanes stopped heavy execution and their owned
emulators/simulators before this window. External background activity remains
unknown. No sample was dropped for being slow. Source/artifact hashes matched
before and after both runs; task-owned temporary stores were removed only after
all children exited. No OS cache was flushed: fresh processes are not cold disk.

Cold execution took82.43 seconds for60 independent processes, plus three real
protected fixture preparations at100/1,000/10,000 rows. Six operation classes each
have10 observed samples and separate inner-open and outer-process endpoints.
The table gives medians in nanoseconds, not an accepted performance limit.

| Class | Rows | Inner open median (ns) | Launch to observed owner median (ns) |
| --- | ---: | ---: | ---: |
| Memory create | 0 | 530966.5 | 11854187.5 |
| Protected native create | 0 | 7844613.5 | 21299986 |
| Protected native reopen | 100 | 3725808.5 | 15211127.5 |
| Protected native reopen | 1000 | 20169227.5 | 33584943.5 |
| Protected native reopen | 10000 | 254013265.5 | 268459272.5 |
| Protected native authored upgrade | 1000 | 284743759.5 | 299639456.5 |

The slowest memory process launch was422,164,802ns and remains in the raw data;
its inner open took732,811ns. This illustrates why the endpoint distributions
are kept separate and why repeatability must precede a numerical budget.

The resource run took17.67 seconds, with30 open/observe/close cycles and61 RSS
checkpoints per engine. Independent complete row proofs matched at the first
and last cycles, and intermediate fixture checks ran outside any latency timer.
Darwin `ps` RSS units were converted from1024-byte units; peak resident bytes
come from `wait4` on the exact owned child.

| Engine | Baseline RSS bytes | Final post-close RSS bytes | Retained delta bytes | Peak bytes |
| --- | ---: | ---: | ---: | ---: |
| Memory | 2269184 | 19099648 | 16830464 | 19841024 |
| Protected native | 2318336 | 23388160 | 21069824 | 24244224 |

These process deltas include runtime/allocator retention; they do not establish
an allocator leak. Browser heap/WASM, mobile SDK overhead and package sizes are
separate metrics and are not inferred here. Full warm workloads, independent
repeat/control runs, reviewed Must/Target values and a failing regression gate
remain required under TC-0037.
