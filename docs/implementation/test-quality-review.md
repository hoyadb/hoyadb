# Code and test quality review — 2026-10-29

Status: implemented and locally verified, including the Claude Opus 5.5
challenge and follow-up corrections. This is not release certification or a
claim that every project/platform gap has been closed.

Origin: the operator requested a code/test/documentation and public-name audit,
coverage improvements, mutation testing and an independent Opus 5.5 challenge.
Evidence applies to revision `035bd0f180ca73c017813f03c454d4ad85936927` plus the
captured dirty source snapshots. Existing user changes were preserved. No
commit, push or GitLab pipeline was performed.

## Findings and dispositions

| Priority | Confirmed finding | Disposition |
| --- | --- | --- |
| P1 | A rejected default browser bootstrap stayed cached permanently. | Fixed under DEF-0041. The regression failed before the fix; concurrent initialization, retry and explicit-worker isolation pass. |
| P1 | A successful browser runtime also stayed cached after its worker crashed. | Fixed under DEF-0043 after the operator selected fresh-worker recovery. Later default opens share a fresh attempt; old handles stay Closed, with no replay or owner migration. The regression failed before the fix. Explicit close also retires that cached attempt. |
| P1 | Coverage collection had no enforced scoped floors or comparison identity. | Recorded measured macOS/dev and Linux/release floors. The runner compares compiler/LLVM, tool, command/features, target, profile and relevant environment; missing/incomplete/incompatible identities reject. Missing floors explicitly fail as not established. |
| P1 | The initial JSON gate always timed out on `digits` changing `self.pos += 1` to `*= 1`. | Added exact public parsing assertions in a supervised child. The same mutant now fails the parent's termination assertion; all earlier 40 catches remain caught. Final result: 41 caught, 1 unviable, zero missed/timeouts. No allowlist, source exclusion or production parser rewrite. |
| P1 | The encrypted conformance wrapper omitted `ownership`, `reads_ready` and `query_validated`, exercising fallback behavior. | Forwarded all three under DEF-0042. The regression failed on Unproven versus Exclusive; all 55 encrypted cases then passed. A fixture counter verifies validated-override entry and exact rows, not SQLite's internal bounded-walk cost. Existing conservative/false-capability suites remain. |
| P1 | Existing DEF-0040: API-0011 declares a Kotlin per-database `onBackgroundError` sink absent from the implementation checked. | Remains open under the existing implementation/platform verification holds. No Android parity or undocumented replacement parameter is claimed. |
| P2 | The proposed full scheduled mutation job would repeat the expensive entire suite for 2,921 candidates. | Removed that proposed job. CI enforces the measured decoder selection. Full discovery/default workspace testing remain available for deliberately budgeted investigations; no full-run result is claimed. |
| P2 | Rejected mutation summaries were empty; failed coverage collection could expose old counts. | Structured rejection artifacts now preserve source hashes, measurement metadata, raw counts, tool status and reason. Old coverage JSON is deleted before collection. Negative CLI/shell regressions reproduced these gaps and pass after correction. |
| P2 | Native/Swift/RN API snapshots are inert, while mapped symbol checks are lexical. | Signature and lifecycle compatibility remain incompletely verified. Eleven Rust documentation tour binaries compiled and executed, but that does not certify mobile/native consumers. |
| P2 | REQ-0034's non-`local:` ID example differs from strict command/entity rules; API-0011 leaves clarification pending operator choice. | Existing design question retained. No generator relaxation or incompatible rename. |
| P2 | Formatting differences predate this task. | `cargo fmt --all --check` still fails in existing entity/reconciliation/record and shared conformance files. No differences were reported for the new parser target or modified encrypted fixture. Existing user work was preserved. |

## Public methods and documentation

Checked API-0004/API-0005 against schema/query/reactive declarations,
API-0011/API-0012 against facade/JavaScript surfaces, and the site's mapped guide
symbols against actual source. The mapped checks found no errors. Kotlin and
React Native bootstrap source checks passed with their explicit runtime limits.
Eleven Rust documentation tours compiled and executed their assertions.

No naming mismatch was found in that checked set. Rust `where_`, `where_in`,
`order_by` and JavaScript `where`, `whereIn`, `orderBy` are intentional documented
language conventions. The tagged host and plain application APIs remain distinct.
No existing public method was renamed. API-0011 now records the operator's
post-crash recovery decision. The public browser entry passes the runtime
factory into `openApplicationDatabase`, which calls it per default open.

Name-only checks cannot establish all signatures or lifecycle behavior. DEF-0040,
the ID example and unenforced compatibility snapshots prevent an unqualified
claim that every public contract matches implementation.

## Verification

- **700 runtime tests passed on macOS/dev and separately Linux/release**, zero
  failures or ignored tests in either run, across 73 Cargo test targets each.
  Both runs collected actual LLVM coverage. A separate workspace doctest run
  passed the **two compile-fail ValidatedQuery cases**; stable instrumentation
  does not include them. The pre-review ordinary workspace run passed 698 tests.
- The supervised parser's intentional failed/empty children print nested test
  summaries. Counts above use the last parent summary in each Cargo `Running`
  target block, not a sum of every child line. The parent test process and cargo
  exit status are authoritative; these negative self-tests are not suite failures.
- **173 JavaScript client/testing tests passed** with the actual rebuilt ordinary
  WASM asset. The CI host selection passed **12 tests** locally. Factory cases
  live in separate test files/processes under pinned Node 22.18.0 to isolate
  module state; no query-string module duplicates inflate LCOV denominators.
- Imported client/testing source: **1,348/1,355 lines (99.48%)**, **1,039/1,126
  branches (92.27%)**, **360/407 functions (88.45%)**, 15 unique LCOV source records.
  Generated glue/test source are outside this denominator. Unimported modules
  and actual browser execution remain unmeasured.
- **29 evidence-tooling tests passed on the host and in the exact pinned Python
  CI image**, with networking disabled for the container. Cases include source
  drift, component/count integrity, mutation selection/baseline/failure results,
  stale collection, floor regressions, missing/incomplete/incompatible metadata,
  short release flags, test-argument separators and subprocess error artifacts.
  CLI fixtures copy temporary sources and inject Git responses, so this Python
  test job requires no Git binary. Production CLI execution still reads real Git.
- Python's standard-library trace with `--missing` measured **208/285 executable
  statement lines (73.0%)** in the current validator. Subprocess CLI execution
  is outside that trace. This is partial validator measurement, not whole-tooling
  or Python branch coverage. The current annotated files are in `python-final/`.
- Rust clippy for workspace/all targets passed with warnings denied. Shell syntax,
  CI YAML parsing, traceability and RTM checks passed. Forty-four documentation
  tooling tests passed earlier; mocked Kotlin output does not establish emulator
  execution. The current formatter failure is recorded above. Whole-tree
  `git diff --check` flags the pre-existing blank EOF in `bindings/native/Cargo.toml`;
  the task-only diff check passes.

### Measured native line coverage

Measured targets: `x86_64-apple-darwin` / dev and
`x86_64-unknown-linux-gnu` / release. Other architectures need their own baseline.

These are LLVM **source-file counts including inline unit-test code**. They are
not production-only percentages. Rust branch coverage remains unknown; regions
are not a substitute. Memory re-exports core's implementation and gets no invented
standalone percentage.

| Component | macOS / dev | Linux / release |
| --- | --- | --- |
| Core | 15,207/15,911 (95.58%) | 15,198/15,908 (95.54%) |
| Facade | 2,061/2,397 (85.98%) | 2,061/2,397 (85.98%) |
| SQLite | 1,759/1,816 (96.86%) | 1,759/1,816 (96.86%) |
| Memory alias | Implementation in core | Implementation in core |
| Rust branches | Unknown | Unknown |

Rust line percentages did not increase in this change. The additional assertions
improve termination, boundary and native capability verification; a line count
alone did not expose those gaps. The facade retains the largest proportional gap
(336 lines), including observations, entities, commands and codecs. Core's larger
absolute gaps include overlay, records and migrations. Five files without LLVM
records stay visible in each summary instead of being labelled covered.

Checked-in floors preserve their own measured target/profile and exact command.
Absolute tool/wrapper paths in the environment make local comparisons specific
to that machine layout; the Linux baseline matches the pinned CI image. Sidecar
measurement metadata is trusted output of these runners, not a cryptographic
binding supplied by LLVM itself.

The first macOS measurement predates the runner; its baseline compiler/command
identity was checked and imported afterwards. Both macOS commands explicitly
selected the installed matching LLVM 22 tools. The early metadata helper omitted
those environment variables; their verified values were imported retrospectively
and labelled in the sidecars. Future captures record them automatically.
No counts, source hashes, metric definitions or thresholds were changed to pass.

Linux was measured in the exact CI Rust image using an isolated source copy;
its Rust snapshot equals the host tree. Its first gate rejected the absent Linux
floor, and a bootstrap postprocessor then failed while expecting Git revision in
that rejected summary. The floor import used the actual host HEAD plus the
matching full Rust snapshot. The current validator subsequently passed the
recorded Linux floor in a container-owned isolated copy, with normal Git ownership
checks and host source/Git mounted read-only. This was local Docker verification,
not a GitLab pipeline run.

### Mutation testing

Full discovery listed **2,921 native candidates**: core 2,314, facade 395, SQLite
212. That inventory was not executed. Other language and excluded binding/spike
workspaces are outside it. The configuration keeps dependent workspace tests as
the default and has no source/function exclusion or baseline suppression.

The enforced decoder selection is:

```sh
sh tests/tooling/mutation.sh \
  --file crates/hoyadb-core/src/command/json.rs \
  --re 'hex_quad|unicode|digits|number' \
  --test-workspace=false --test-package hoyadb-core \
  -- --test json_bounded
```

The baseline passed. **42 selected experiments completed in about five minutes:
41 caught, 0 missed, 0 timeouts, 1 unviable.** The viable selected score is 100%,
not a workspace/project score. The unviable case returns `Ok(Default::default())`
for `Json`, which has no matching Default implementation.

`mutation-comparison.json` confirms identical selected names and preserves all
40 earlier catches. Its sole classification change is the no-progress `digits`
mutation: Timeout to CaughtMutant. Its log shows the parent termination assertion
failing after 10.17 seconds. A completion code distinguishes actually executed
corpus assertions from zero matching child tests. Empty, failed and stalled child
self-tests verify the supervisor. Original JSON unit tests still run in ordinary
workspace regression execution. Exact malformed-number categories and underflow
results are labelled characterization where API-0006 permits implementation detail.

The initial unit-test-only red run is retained in `mutation-initial/`. Earlier
interrupted exploratory runs, including the first supervisor draft, are excluded
from completed evidence. A seeded JavaScript jitter boundary mutant also failed
its regression assertion; that single experiment is not a JavaScript mutation score.

## Independent challenge and remaining gaps

Claude Opus 5.5 completed the review through Paseo advisor
`80491969-9191-460c-bc6d-c134e4bfd055`, followed by multiple challenges of the
corrections. It found the fixture's third missing override, impractical full-run
schedule, unwired floors, missing provenance and a zero-test false-green hole in
the first supervisor. Those findings drove the corrections above. It subsequently
traced the browser factory without finding a race. Its final round found no
blocking defect in these changes; it did not rerun the tests. Execution/count claims in this
report come from the parent agent's observed commands, not from advisor inference.

Remaining work includes the facade's measured gaps; production-only/Rust branch
measurement; broader critical-module mutations and the full inventory; real
browser/native/mobile runtime coverage; first-party tooling beyond this partial
validator measurement; enforcing compatibility snapshots; and the existing
Android/background-error and custom-ID contract questions. These are not accepted
as gap-free or replaced with aggregate percentages. No release certification is
implied by locally passing gates.

Raw evidence is under ignored `target/quality/`: coverage exports/source snapshots,
`coverage-summary-reviewed.json`, `mutation-summary-reviewed.json`, selected mutant
diffs/logs, `mutation-comparison.json`, JavaScript LCOV/counts, `python-final/`,
Linux reports under `linux-worktree/target/quality/` and validated Linux summary
under `linux-validation/`. `task.diff` separates this task from pre-existing work.
Tool pins: cargo-llvm-cov 0.9.1, cargo-mutants 27.1.0, rustc 1.98.1 / LLVM 22.1.8,
Node 22.18.0. Logs are preserved under `target/quality/logs/`.

See [the quality guide](../guides/test-quality.md),
[SPEC-0044](../design/spec/SPEC-0044-test-quality-and-mutation.md),
[NFR-0006](../design/requirements/nfr/NFR-0006-test-quality-evidence.md) and
[TC-0048](../design/tests/cases/TC-0048-test-quality-and-mutation.md).
