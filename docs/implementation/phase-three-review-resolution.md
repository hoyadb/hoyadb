# Phase 3 review corrections and three Claude verification rounds

Status: finalized by operator acceptance on 2026-10-23. Implementation is
verified; rounds 1 and 2 passed. Round 3 was interrupted by Claude's session
limit with no verdict. The operator accepted the current result ("Let's say
it's good for now, finalize your phase") and then directed a stop before
Phase 4. The scheduled continuation was canceled; no third pass is claimed.
Operator requested three review/resolution rounds, explicitly limited to real
issues rather than a quota of findings. No findings is a valid review result.
The original independent Claude Opus 5 review covered code since Phase 0.
This follow-up preserves Phase 0–3 behavior while correcting reproduced faults.

## Scope and disposition

Reviewed prep SPEC-0028, DEF-0006/0007/0008 and TC-0031 was committed as
`671daa2` before tests/correction. Claude prep review clarified candidate-vs-row
failure, registration behavior, memory-only temporary sets and threshold parity.
Implementation and permanent regressions are committed as `094d59a`.

| Finding | Resolution |
| --- | --- |
| DEF-0006, SQL expression-depth failure near 1000 IDs | Native flat IN, with bounded bound-parameter execution through connection-local TEMP sets for oversized inputs. temp_store=MEMORY; same typed encoding/affinities as stored cells; main schema unchanged. |
| DEF-0007, repeated explicit retry failures silent | Each failed explicit refresh queues a typed error. Ordinary writes against an already dirty group suppress duplicate errors; count deadlines remain intact. |
| DEF-0008, row decoding poisons valid counts | Candidate-key validity is separate from per-row-listener delivery state. Counts continue from valid projected keys; failed row listeners retry exact current state independently. |
| Native error-arm coverage | An entry-scoped one-shot fixture causes actual Swift kind=2/code=7 callbacks, reentrant committed reads, repeated explicit errors, exact recovery, cancellation and close. |
| Real SQLite read-fault coverage | Two fixtures corrupt consulted physical payload only after successful COMMIT, exercising the real UTF-8/value decoder, restoration and exact recovery with unchanged metadata/counter. |

The original claim of permanent irrecoverability was corrected: an observation
recovers if writes reduce the visible window below the failing SQL threshold.
Dropping final observers clears cached failure state but cannot make an oversized
fresh query succeed. The threshold was native-build/query-shape dependent;
new lower-bound regressions require successful large queries instead of
preserving an accidental ceiling.

Snapshot delivery intentionally reads complete visible rows when row events
must emit; its cost grows with the window. Memory writes clone the store/indexes
for atomicity; migration/adoption work materializes lifecycle rows and plans.
These are disclosed costs, not newly promised constant-work/streaming behavior.
The current adapters finish without suspension; cancellation of in-flight
public mutations has no rollback guarantee. Genuinely pending adapters are not
validated by this work. No future-feature optimization or redesign was added.

## RED and GREEN evidence

TC-0031 shared Q01/Q02/Q03 functions run on memory, native SQLite and real browser
SQLite/OPFS. They cover 996-to-1105 live growth, 1105 reverse dependencies and
many-to-many navigation, a separate 4105-row reactive TEMP window and two-set
intersection, and 33005 operands across all six primitive kinds,
Null, Unicode, duplicate values, multiple predicates and order/limit. Small
flat and oversized TEMP paths return identical selections, including Float
42.0 and 1.25, finite extrema and i64 extrema. Temp execution also covers actual
query-plan execution, subsequent query isolation and unchanged main DDL/reopen.

Before correction, four SQLite shared cases failed for the expected expression
depth, missing repeat error and count-poisoning reasons. Memory passed both
large cases and failed both recovery cases. Logs: ignored
`target/review-sqlite-red.txt` and `target/review-memory-red.txt`. Two actual
SQLite physical-fault tests independently failed on count poisoning and missing
retry notification, then passed after correction. The actual Swift Q05 case
compiled/linked and failed at absent typed error callbacks, then reached and
reproduced silent repeated refresh before passing against the corrected core.

The [actual browser RED](../../spikes/storage-browser/results/review-resolution-failure-red.json)
shows a committed insert followed by correct row errors and incorrect count
error. [Standalone GREEN](../../spikes/storage-browser/results/review-resolution-green.json)
passes 17 reports, including all five new shared functions, repeated projected
errors, independent row/count recovery and retained deadlines. An actual
33002-operand worker probe returns exact keys/rows with identical SAH pool
file names before/after and successful namespace cleanup.
The full harness passes [36 first-page reports](../../spikes/storage-browser/results/review-resolution-full-stage1.json)
and [12 after actual reload](../../spikes/storage-browser/results/review-resolution-full-stage2.json),
retaining rollback, persistence, read-before-write, identity continuity, live
owner rejection and acknowledged VFS shutdown/cleanup.

The five shared cases run as separate worker requests, each retaining the full
harness's original 20-second deadline. The final first-page samples were
3589, 2357, 2575, 30 and 23 ms (rounded). Earlier aggregated fixture attempts
exceeded the deadline; the corrected fixture isolates setup costs without
reducing the required cardinalities or exact snapshot assertions. The earlier
[oversized fixture timeout](../../spikes/storage-browser/results/review-resolution-oversized-fixture-timeout.json)
and [interrupted run](../../spikes/storage-browser/results/review-resolution-interrupted.json)
remain diagnostic records, not passing evidence. The interrupted run and failed
full fixture retain their synthetic namespaces; successful final runs clean
only their own namespaces. A stale cached JS module initially called the new
indexed worker operation without its index; an explicit cache reload corrected
the harness asset mismatch before the recorded final runs.

The [source/build manifest](../../spikes/storage-browser/results/review-resolution-build-manifest.json)
records the executed source, generated JS/WASM and result digests. Historical
Phase 3 results remain untouched. Actual runtime is Paseo 0.12.0 / Electron
44.2.0 / Chrome 152.0.7977.76 on secure loopback port 8772, without security flag
changes. Test fixtures stay synthetic; no origin-wide deletion.

All **171 workspace tests** pass, preserving all previous 158. Workspace fmt,
Clippy with warnings denied, and core/memory WASM builds pass. The native
[RED/GREEN record](../../spikes/cross-platform/native/REACTIVE_RED.md) records
four actual Swift cases, eight native Rust tests, eight old Swift scenarios
and separate persistence writer/reader processes. Wrapper faults are explicitly
synthetic; the native SQLite tests separately exercise actual SQL decoding.
No new runtime dependency or lockfile change.

## Numbered Claude rounds

[Round 1 full output](claude-resolution-round-1.md) and
[round 2 full output](claude-resolution-round-2.md) are retained.
Only post-fix code reviews count toward the operator's three requested rounds;
initial review/addendum and prep reviews do not count. Each round assesses real
supported-contract failures and may return no actionable finding. Any new
correction requires a permanent regression and re-verification.

| Round | Snapshot | Result |
| --- | --- | --- |
| 1 | 094d59a | PASS; defects resolved; document structural AND limit and strengthen owner TEMP coverage |
| 2 | 7e531fe plus bounded-case working diff | PASS; no actionable findings; 171 tests and independent SQLite probes |
| 3 | 2ffa1bd | Interrupted by provider session limit; no verdict; operator accepted closure with two completed reviews |

The pinned Rust 1.98.1 container passed from final frozen `git archive 2ffa1bd`
mounted read-only, with fresh ephemeral Cargo caches/targets: all 171 tests,
both examples, fmt/Clippy and core/memory/SQLite/browser WASM builds. It exited
zero with `PHASE3_CONTAINER_GREEN`. The image digest matches `.gitlab-ci.yml`.
Commands are unchanged from `phase-three-ci-commands.sh`; terminal
progress whitespace is normalized in [container output](review-resolution-ci-output.txt).
[Host test output](review-resolution-host-tests.txt) is retained as well. Trace
check reports zero violations and the RTM is current. The frozen-container run
also proves the final source archive builds without workspace artifacts.
The [third Claude attempt](claude-resolution-round-3-interrupted.md) stopped at
the provider session limit and is not counted as a pass. The operator accepted
closure with this limitation and canceled further review work. No unresolved
demonstrated supported-contract finding remains from the completed reviews.
Phase 4 has not started: only roadmap/governance reads occurred before the stop.
No push, merge or publication.
Only the six preexisting user CI runner-tag lines remain outside task commits.
