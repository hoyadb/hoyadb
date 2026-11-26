# hoyadb implementation plan and progress

Status: **Draft — awaiting operator validation.**

Origin: user request to describe the full implementation path, directory
organization, and progress in a file for follow-up.

This execution tracker began as a proposal. Subsequent operator instructions
authorized the slices recorded below; this file does not independently authorize
new work or replace formal contracts. It complements [ROADMAP.md](ROADMAP.md).
Stage 0 is complete as a bounded experiment. Roadmap Phase 1 storage is complete
at SPEC-0025's internal acceptance boundary; its current evidence is in
[Phase 1 storage](docs/implementation/phase-one-storage.md). The broader
production/release stages below retain their independent gates. Directory/module
names in the proposed tree are not all existing code. Only versions recorded in reviewed slice artifacts and locks are adopted.

## 1. Authority and starting point

Read before implementing a slice:

- [Project instructions](AGENTS.md).
- [Public contribution workflow](CONTRIBUTING.md) and
  [current repository layout](docs/guides/repository-layout.md).
- [Formal design index](docs/design/README.md),
  [system architecture](docs/design/architecture/ARC-0001-system.md), and
  [platform boundary](docs/design/architecture/ARC-0002-platform-bindings.md).
- [Storage contract](docs/design/spec/SPEC-0001-storage-interface.md),
  [default engine](docs/design/spec/SPEC-0002-default-engine.md), and
  [acceptance test plan](docs/design/tests/plan/TP-0001-test-plan.md).

The inspected design describes one Rust core, WASM on web, thin native bindings,
pluggable storage with a SQLite-family default, and command-first sync through
developer-supplied transport. It explicitly leaves concrete binding mechanisms,
browser persistence, indexing, initial sync, and multi-instance access open.
The formal test plan contains intended acceptance cases, not an executed suite.

Repository status was clean before this planning file was created. The inspected
branch was `main`. Before implementation, inspect branches and remotes and use
the governed feature-branch flow; do not create integration history blindly.
Commit and push only when requested, as required by CHG-3. This plan is uncommitted.

## 2. Progress dashboard

Use `Pending`, `In progress`, `Blocked`, or `Complete`. Completion requires linked
evidence; a checked task alone is insufficient. Do not derive a percentage from
these phases: their sizes differ substantially.

| Stage | Status | Exit evidence |
| --- | --- | --- |
| Planning proposal | In progress | This file; operator validation pending |
| 0. Cross-platform feasibility spike | Complete | All seven bounded exit questions answered in `docs/implementation/spike-report.md`; native restart passes, browser persistence is a negative finding for memory path |
| 1. Production workspace and test foundation | In progress | Core/memory/native+browser SQLite typed storage; see Phase 1 report; packaged production bridges and remote CI gates remain |
| 2. Storage and durability | In progress | Typed query/change conformance extends CRUD and restart; crash, concurrency and encryption gates remain separate |
| 3. Schema, queries, relations, migrations | Complete for public Rust scope | Phase 2 report and TC-0026/0027/0028 |
| 4. Reactive queries | Complete for Phase 3 scope | TC-0029/0030; memory/native/browser assertions and real callback delivery |
| 5. Local command execution | Complete for Phase 4 scope | API-0006/TC-0032; real Swift/browser and current Phase 5 regression evidence |
| 6. Durable queue and command reconciliation | Complete for Phase 4 scope | Phase 4 final report; current frozen-source host/browser/Swift regressions pass |
| 7. Entity diffs, conflict handling, pull/reset | Complete for Phase 5 scope | TC-0034 maps TC-0011/0013; [execution evidence](docs/implementation/phase-five-entity-sync.md) |
| 8. Release security verification | Complete for Phase 6 targets | TC-0035 maps TC-0012/0015; [security evidence](docs/implementation/phase-six-security.md) |
| 9. Platform breadth, performance, release | In progress | [Phase 7 checkpoint](docs/implementation/phase-seven-platform-sdk.md); complete platform, measurement and final review gates remain open |

Completed maintenance interlude: the operator paused feature work for
[Rust formatting rules and cleanup](docs/design/spec/SPEC-0030-rust-formatting.md).
[TC-0033](docs/design/tests/cases/TC-0033-rust-formatting.md) defines its checks;
234 workspace tests, 31 host spike tests and formatting/lint/build/traceability
checks passed locally. This pass does not advance any implementation stage.

## 3. Proposed directory organization

The original proposal below is retained as planning context. The current layout
and historical path map are in [repository layout](docs/guides/repository-layout.md),
implemented by SPEC-0045 after the operator approved public contributor independence.

Create directories only when the corresponding slice needs them. Avoid empty
packages, placeholder abstractions, and scaffolding for every platform on day one.

```text
hoyadb/
├── AGENTS.md                       existing project instructions
├── ROADMAP.md                      existing strategic sequence
├── IMPLEMENTATION_PLAN.md          this execution/progress tracker
├── DECISIONS.md                    existing append-only decision log
├── tools/traceability/             public checker/generator subset
├── docs/
│   ├── design/                     existing formal authority; stable IDs
│   │   ├── spec/                   formal specifications and registry
│   │   ├── tests/                  formal Gherkin cases and test plan
│   │   └── traceability/           generated RTM
│   ├── guides/                     existing consumer/developer explanations
│   ├── references/                 existing research and history
│   └── implementation/             proposed evidence and run instructions
│       ├── spike-report.md
│       └── platform-matrix.md
├── spikes/
│   └── cross-platform/             isolated feasibility experiment
├── Cargo.toml                      production workspace, after spike review
├── crates/
│   ├── hoyadb-core/                shared semantics and storage contract
│   │   ├── src/
│   │   │   ├── schema/             declarations, validation, migrations
│   │   │   ├── query/              typed query model and validation
│   │   │   ├── storage/            engine-independent contract
│   │   │   ├── reactivity/         result membership and invalidation
│   │   │   ├── commands/           execution scopes and write staging
│   │   │   └── sync/               queue/reconciliation state machines
│   │   └── tests/                  Rust integration tests
│   ├── hoyadb-storage-memory/      hermetic adapter
│   ├── hoyadb-storage-sqlite/      native default candidate, subject to spike
│   ├── hoyadb-wasm/                web bridge; no duplicated domain logic
│   └── hoyadb-native/              native bridge; mechanism still undecided
├── packages/
│   └── hoyadb/                     proposed JS/TS public wrapper
├── bindings/
│   ├── swift/                     handwritten wrapper source if needed
│   ├── kotlin/                    handwritten wrapper source if needed
│   └── react-native/              only after binding choice is validated
├── tests/
│   ├── conformance/               shared engine behavioral fixtures/harness
│   ├── acceptance/                executable drivers for formal test cases
│   └── cross-platform/            browser/native semantic parity tests
├── examples/                      small runnable consumer integrations
├── benchmarks/                    reproducible workloads and baseline data
└── tools/                         repeatable build/test helpers as needed
```

Directory rules:

- Core owns shared behavior; adapters implement storage and bindings translate
  calls/events. Query execution stays with the engine, using the core query model.
- The storage contract lives in core initially. Extract another crate only if a
  demonstrated dependency problem requires it; adapters depend on core, not vice versa.
- Concrete adapters are composed at the hosting boundary. Do not let core require
  a platform adapter or depend on JS/Swift/Kotlin runtime types.
- The browser engine may need a separate adapter. Its directory and dependencies
  are deliberately deferred until the spike establishes a viable persistence path.
- Native generation may eliminate some proposed handwritten wrapper directories.
  Do not keep both generated and handwritten copies of the same behavior.
- Keep generated bindings and build outputs separate from handwritten source;
  decide which generated artifacts must be versioned when packaging is specified.
- Keep formal `.feature` cases in their existing directory. Executable harnesses
  reference those IDs rather than maintaining competing behavioral specifications.
- Keep spike code isolated. Promote reviewed pieces deliberately; never make a
  published package depend on an experimental spike directory.
- Follow ADR-0029's formal artifact root. Resolve the generic governance reference
  to `specs/` against the project's existing `docs/design/spec/` convention before
  adding any per-slice spec; do not create a second formal tree.

## 4. Implementation stages

### Stage 0 — prove the risky boundaries

Scope follows Phase 0 of ROADMAP.md: one hardcoded table, typed fields, insert,
one predicate, one subscription, and one local command-shaped handler. No sync,
relations, full migration system, or consumer packaging.

- [x] Define the spike specification and named acceptance cases before code.
      SPEC-0016/0017/0018/0021 and their named cases preceded implementation.
- [x] Check installed toolchains; select one native host target for validation.
      Validated Swift desktop processes on x86_64 macOS; see the spike report.
      This does not establish mobile or React Native support.
- [x] Recheck binding/storage candidates against current primary documentation
      before adopting dependencies; record licensing and runtime requirements.
- [x] Write and observe failing Rust insert/query tests, then implement the minimum.
      SPEC-0016: five tests red before green; implementation `83879ef`.
- [x] Run the same core through a real browser and a real native binding.
      SPEC-0017 B01–B05 and native N01–N09 provide partial fixture evidence.
- [x] Verify native restart persistence; record browser reload behavior and mechanism.
      Separate Swift writer/reader pass; browser memory sentinel absent on reload.
- [x] Prove subscription delivery on web and native, record thread/runtime affinity,
      payload shape, disposal behavior, and measured end-to-end latency.
- [x] Prove an async host handler can re-enter for reads, see its staged writes,
      hide those writes from outside readers, and commit atomically after return.
- [x] Exercise failure cleanup and serialized execution; record unresolved behavior
      rather than adding arbitrary timeout or cancellation policies.
- [x] Record findings, failed approaches, reproducible commands, and affected formal
      artifacts in the spike report.

Gate: all seven roadmap exit criteria have evidence-backed answers. A negative
answer is a valid experiment result, but blocks assuming that architecture works.
Review binding/storage choices and necessary design changes before production work.

### Stage 1 — establish the production foundation

- [ ] Review spike findings and update affected specifications; use new superseding
      ADRs for changes to accepted decisions, preserving existing ADR bodies.
- [ ] Establish a minimal workspace with core, the first adapter, and proven bridges.
      Core, memory and native/browser SQLite now exist. The browser test bridge
      uses the production adapter; packaged production bridges remain deferred.
- [ ] Define error translation and database/subscription handle ownership contracts.
- [ ] Introduce deterministic clock/ID seams, hermetic storage, and fake transport
      when their first tests require them (TC-0016).
- [ ] Wire the first formal acceptance case into an executable runner; choose the
      runner from demonstrated fit rather than introducing a large test framework.
      TC-0019 is executable; TC-0016 memory write/read is covered only partially.
- [x] Establish repeatable format, lint, test, and target-build checks in CI.
      Select provider configuration after checking the actual repository hosting.
      Pinned GitLab CI image/commands executed locally, including browser WASM;
      remote run remains a merge gate.

Gate: a fresh checkout can build and run the first meaningful case on the proven
targets using documented commands. Dependency versions are pinned as appropriate
for the workspace, and all new runtime dependencies have explicit approval.

### Stage 2 — storage contract and durable adapters

- [x] Specify exact async storage operations, transaction semantics, change reporting,
      and capability differences without adding encryption-specific generic methods.
      API-0002/0003 and SPEC-0025 define the bounded internal contract.
- [x] Write adapter conformance cases before implementing adapters.
      TC-0019/0020/0022/0023/0025 precede their implementations.
- [x] Implement in-memory storage and the validated native persistent adapter.
      API-0002 CRUD/batches and API-0003 typed queries/committed changes.
- [x] Implement the validated browser persistence adapter/path.
      SPEC-0023 shares the production Rust adapter; worker/page persistence passes
      for its explicit single-connection subset.
- [x] Verify type round trips, query semantics, commit/rollback, and reopen behavior.
      TC-0025 P01–P13 pass; 70 host tests, real browser worker/page restart,
      pinned-container builds and source-only archive checks are recorded in the
      Phase 1 storage report.
- [ ] Test crash-sensitive atomic writes and storage-error propagation.
- [ ] Specify supported concurrent-open behavior before claiming multi-tab/process
      support; resolve RISK-0003 through an explicit design decision and tests.
- [ ] Establish engine encryption feasibility now so release hardening does not
      require discovering a replacement storage engine late in the project.

Gate: supported adapters pass the same relevant conformance cases. Weaker
transaction capabilities are explicit; they cannot claim strong queue durability.

### Stage 3 — schema, query model, relations, migrations

- [x] Implement schema declarations, typed values, IDs, and reserved-name validation.
- [x] Implement accepted predicate operators, implicit AND, ordering, and limit.
- [x] Implement relationship declarations and relation traversal.
- [x] Decide indexing from required query workloads and reactive dependencies;
      ADR-0030 accepts engine-owned indexes; TC-0025 P12 records actual selective
      plans/candidate counts and construction/update/query/ordering costs.
      Further reactive maintenance remains Stage 4 work.
- [x] Implement migration-chain behavior and the specified reset fallback.
- [x] Guard destructive reset: the supported Phase 2 format is queue-free; reject
      unknown bookkeeping, including future queues, unchanged (ADR-0031).
      Queue-bearing lifecycle policies must be specified with sync before extension.
- [x] Execute TC-0001 through TC-0003 behavior via the explicit TC-0026 mapping
      and shared adapter conformance; no historical Gherkin runner is claimed.

Gate: identical inputs produce equivalent supported results on memory, native,
and browser engines. Migration failures have documented recovery behavior.

Phase 2 completion and exact verification are recorded in
[the data-model report](docs/implementation/phase-two-data-model.md). This
stage implements API-0004 in Rust; packaged SDKs, live observation and sync
remain in their roadmap phases.

### Stage 4 — reactive queries

- [x] Implement fetch/observe behavior and initial subscription results.
- [x] Implement set-membership default emission semantics and declared opt-ins.
- [x] Implement query sharing, invalidation, and incremental result maintenance.
- [x] Deliver notifications through the proven bridges with explicit lifecycle
      and thread/runtime scheduling rules.
- [x] Test irrelevant writes, membership changes, updates inside transactions,
      unsubscribe, database close, and multiple equivalent observers.
- [x] Run TC-0004 and record boundary overhead using repeatable workloads.

Gate: accepted notification semantics hold across targets; disposal stops delivery
and releases retained resources. Numerical performance promises await a baseline.

Phase 3 completion, work counts, boundary scheduling/timings and remaining
limits are recorded in [the reactive report](docs/implementation/phase-three-reactivity.md).
The public surface is Rust API-0005; test bridges are not packaged SDKs.

### Stage 5 — local commands and isolation

- [x] Specify the selected handler mechanism and tracked execution-scope lifecycle.
- [x] Implement async reads, read-your-own-writes, staged ordered writes, and commit.
- [x] Enforce one active handler and atomic visibility in core, across all bridges.
- [x] Resolve handler failure, never-resolving execution, scope abandonment, and
      interaction with other writers before claiming those behaviors are supported.
- [x] Test rollback on failure, competing handlers, re-entry, and notification timing.
- [x] Run TC-0005 through host-language integrations, not Rust-only mocks.

Gate: a host handler cannot expose partial writes or bypass serialization through
the public binding. This stage adds no network transport.

### Stage 6 — queue and command reconciliation

Deliver in small slices, each with a reviewed specification and failing tests:

1. Persist envelopes and local mutations atomically on transactional engines.
2. Implement durable queue transitions and restart recovery.
3. Add developer-supplied push transport using deterministic fake responses first.
4. Add stable idempotency keys, independent batched outcomes, and retry/backoff.
5. Add dependency blocking, local-to-backend ID mapping, and dependent release.
6. Add authoritative correction, rejection, rollback, and specified cascades.
7. Add queue inspection, per-command outcomes, cancellation, and backlog signals.

- [x] Resolve multi-hop dependency and ambiguous cancellation behavior in the design.
- [x] Test dropped responses, duplicate delivery, mixed batch outcomes, crashes at
      commit boundaries, out-of-order replies, and failed dependent operations.
- [x] Run the command-only TC-0006–TC-0010/TC-0014 cases mapped by TC-0032;
      entity-diff cases remain Stage 7.
- [x] Keep any reference HTTP transport an opt-in integration/example; the core
      must never acquire its own network implementation.

Gate: pending work survives restart without losing its identity; client outcomes
match the specified state machine. Backend deduplication and authorization remain
explicit integrator responsibilities, not claims established by client tests.

### Stage 7 — entity diffs, conflict resolution, pull, reset

- [x] Implement partial entity-diff payloads through the same durable queue.
- [x] Implement client behavior for per-field LWW, delete precedence, and overrides.
- [x] Preserve the server's authority for identical-timestamp tie breaking.
- [x] Specify bootstrap/initial sync before building it (RISK-0005).
- [x] Implement membership-scoped pull, checkpoints, and authoritative local apply.
- [x] Resolve checkpoint/data atomicity, interrupted bootstrap recovery, and reset
      interactions with active handlers, subscriptions, and queued work.
- [x] Test TC-0011 and TC-0013, plus new cases for decisions that were previously open.

Gate: push/pull/local writes compose under tested concurrency rules, and reset has
a total, reviewed contract. The library still delegates identity and membership
calculation to the integrating backend.

### Stage 8 — release security verification

Security is reviewed during earlier contract and storage work; this is the final
verification stage, not the first time security is considered.

- [x] Implement and verify the selected engine encryption path with supplied keys.
- [x] Implement documented backup/export exclusion for each supported platform.
- [x] Verify trusted-context boundaries and sensitive-data/log redaction.
- [x] Verify no library-owned networking or telemetry is introduced.
- [x] Review trust-boundary changes against the threat model.
- [x] Run TC-0012 and TC-0015, including relevant on-disk inspection and platform
      procedures; document what automated tests cannot establish.

Gate: supported-platform security claims have evidence and explicit limitations.
Do not publish an intermediate unencrypted build as meeting the accepted floor.

### Stage 9 — platform breadth and release readiness

- [ ] Add remaining Swift/Kotlin/React Native/desktop surfaces one at a time after
      validating their bindings; do not infer support from desktop spike results.
- [ ] Run the same behavioral matrix on every claimed target, including lifecycle,
      persistence, error translation, callback cleanup, and handler re-entry.
- [ ] Complete TC-0016 and TC-0017, and all other applicable formal acceptance cases.
- [ ] Establish performance, memory, storage-footprint, and boundary-cost baselines
      with datasets, device/runtime details, and reproducible commands.
- [ ] Agree numerical budgets only after measurement, then add regression checks.
- [ ] Finalize consumer packaging, API compatibility checks, examples, installation,
      migration guidance, and developer documentation.
- [ ] Review the complete diff, dependency licenses, threat changes, traceability,
      release notes, rollback procedure, and remaining unsupported scenarios.
- [ ] Follow the governed release process; obtain explicit authorization before
      publishing packages, tagging releases, or pushing repository changes.

Gate: every claimed requirement/target is backed by executed evidence; unresolved
requirements are resolved or formally revised, never silently omitted.

## 5. Mandatory TDD and coverage gates

**TDD is the required implementation mode for every behavior, including spike
code, adapters, bindings, failure handling, and integration helpers.** Follow
the [public contribution workflow](CONTRIBUTING.md).

- [ ] Before implementation: reviewed specification and enumerated Given/When/Then
  cases, with source artifact and section, acceptance criterion, target platforms,
  proposed test location, and expected result for each behavior.
- [ ] RED: add the smallest meaningful test and run it. Record the exact command,
  test identity, and failure. Confirm failure is missing/wrong behavior, not a
  broken import, missing tool, compilation accident, or unrelated fixture error.
- [ ] GREEN: implement only enough behavior to pass that test; record the command
  and result. Do not write an entire subsystem before returning to its tests.
- [ ] REFACTOR: improve structure only while affected tests remain green; rerun
  required checks after changes. Never weaken expectations or disable a check.
- [ ] Repeat for each positive case, rejection case, boundary condition, lifecycle
  transition, persistence failure, and concurrency rule required by the slice.
- [ ] Evidence: preserve reviewable red/green steps and eventual authorized history.
  Tests written after working implementation do not establish a TDD red step.
- [ ] Completion: all applicable scenarios execute with real assertions; no skipped,
  pending, stubbed, or unbound scenarios count as verified. Expand every Scenario
  Outline example row. A passing parser or mocked bridge cannot prove engine or
  cross-language behavior.

If automation is impractical, stop before implementing that behavior: explain why,
agree on the alternative, then record and execute an approved FLOW-8 procedure
with expected and observed results. Do not silently waive TDD.

Run core tests for each small cycle, adapter conformance when storage changes,
real browser/native integration when bridges change, and the applicable broader
acceptance and quality checks before delivery. Use injected clocks, controlled
schedulers, deterministic IDs and reproducible faults instead of sleep-based
assertions. Measure real cross-boundary latency separately from deterministic tests.

### Coverage reconciliation before starting and finishing a slice

The inventories below are scheduling checklists, not proof that every normative
sentence has already been translated into a test. Before implementing each source,
read its complete current contents and dependencies, including linked schemas and
Open sections. Split every obligation into a named acceptance case. Track:

`source ID + section → behavior → case/example → executable test → target/engine
→ RED evidence → GREEN evidence → review reference → status`.

Every source item must receive one disposition: implement/test, verify an
architectural constraint, document an integrator responsibility, explicitly exclude
by an accepted decision, or block pending a decision. An unresolved feature is not
permission to invent behavior or silently omit it. A deferred accepted obligation
prevents a full-completion claim unless its scope is formally revised.

At each milestone compare the current registry, Gherkin files, machine-readable
contracts, guides and changed source documents against this checklist. Add newly
introduced obligations before marking the milestone complete. Trace/RTM checks
prove document consistency, not that code satisfies the documents.

### Detailed implementation checks supplementing stages 0–9

| Source | Stage | Required behavior and tests to decompose before coding |
| --- | --- | --- |
| SPEC-0001, SPEC-0002 | 2 | Typed CRUD; awaited queries; change reporting; transaction capabilities; memory/persistent parity; errors/rollback; durable reopen; engine-specific encryption hidden behind the generic contract. |
| SPEC-0003, DM-0001 | 3 | String/integer/float/boolean/timestamp/reference values; required/optional fields; implicit IDs/bookkeeping; schema version; all three relation kinds navigable from both sides; lazy query objects; reject undeclared tables, invalid relation targets and reserved names. Specify unresolved value/validation boundaries first. |
| SPEC-0004 | 3 | Separate cases for =, !=, <, <=, >, >=, in; implicit AND; direct to-one dot paths; reject unsupported to-many traversal; ascending/descending order and limit composed with fetch/observe. Specify null, invalid operand, limit-boundary and tied-order behavior before asserting it. Do not add OR, aggregates or pagination. |
| SPEC-0005 | 3 | Same-version no-op; ordered complete chain; no inferred steps; missing-chain reset; no partial upgrades; migration before sync; schema/library version distinction. Resolve migration primitives, failure recovery, downgrade and queued-work/reset policy before implementing those paths. |
| SPEC-0006, SPEC-0007 | 4 | Initial fetch/live parity; enter/leave versus matched-field edits; unsubscribe; shared equivalent subscriptions; incremental maintenance; count-only number with throttling; only named columns trigger finer observation. Decide throttle policy, limited-result replacement/reordering and bootstrap visibility before their tests. |
| SPEC-0008, DM-0002 | 5–6 | Argument validation/serialization; awaited read/write and isolation; global serialization; ordered atomic commit; optimistic local IDs; envelope name/version/args/key/context/capturedAt; no authoritative row-log replay; immutable forwarded identity; wire/app outcome separation with correlation stripped; correction/rejection and explicit rollback mechanism. |
| SPEC-0009, DM-0004 | 6 | queued/sent/acknowledged transitions; atomic local mutation plus queue entry; reserved storage; kill before/after commit; recovery; receipt distinct from outcome; dependency blocking; ID swap only on applied/corrected; durable dependency recovery; identical retries; automatic retry on new command and explicit retry; retry only unacknowledged entries; configurable backoff/jitter; revoked writes follow backend rejection. |
| SPEC-0010, DM-0003 | 7 | Generated create/update/delete patches, only written fields; timestamps/schema/context; no command idempotency key under current contract; same queue and authorization boundary; mixed command/diff arrays. Resolve same-record multiple-write correlation and dependent-diff behavior before claiming these paths work. |
| SPEC-0011 | 7 | Distinct-field preservation; same-field LWW; delete beats later update; table override surface; no client timestamp-tie authority; ordinary same-identity/multiple-device handling. Explicitly account for all three documented override alternatives: changed-field strategy, CRDT dispatch, and loser preservation. Resolve hook semantics and the library/backend division before promising implementations; do not silently reduce them to an unspecified hook. |
| SPEC-0012 | 7 | Null initial checkpoint versus unchanged opaque later tokens; apply server-selected data, no UI-derived membership; total explicit reset; independent stores/reopen; no automatic revocation purge; normal authorization rejection for revoked queued writes. Bucket-count precedent is research context, not an invented client limit. |
| SPEC-0013, API-0001 | 6–7, 9 | Eligible arrays in execution order; independent outcomes; reverse-order correlation; missing outcomes remain retryable; separate wire/app enums; mixed payload discrimination; pull checkpoints; fake auto-correlation; reference HTTP request/response schema checks; sequential single-entry request opt-out; verify a second real carrier without changing the core contract. Specify scheduling and separate receipt/outcome recovery window first. |
| SPEC-0014 | 2, 5, 8 | Supplied engine keys; encrypted read/write parity; wrong-key failure without plaintext fallback; no generated/persisted key; backups excluded by default and explicit opt-in; no own authentication/network/telemetry; confidential queue payload retained for retries while incidental diagnostics obey privacy policy. File string absence alone is not sufficient encryption proof: verify actual engine configuration and reopen behavior. |
| SPEC-0015 | 6 | Aggregate plus per-command status; blockedOn is local ID; live observation; queue versus bounded resolved-history lifecycles; rejected history visible; no automatic cancellation; queued/rejected cancellation rollback/cascade; sent cancellation limitation; configurable backlog policy. Specify threshold scope/action, retention and backoff parameters before implementation. |
| UX-0001, supporting guides | 4–9 | Executable developer walkthrough: schema, offline read/observe, optimistic command, queued state, backend correction/rejection and rollback; honest initial-loading state after bootstrap decision; idiomatic host error handling; accurate runnable examples and installation guidance. |
| NFRs, threats, architecture | All | Offline measured operation set; queue crash durability; single semantic core; API semver and non-destructive library upgrades; baseline responsiveness/footprint without invented budgets; mitigations tested at the actual owned boundary; documented accepted risks remain explicit. |

### Decisions that must remain visible

In addition to stage gates, track the following source-level open items. Resolve
before the affected behavior, or explicitly record the accepted exclusion/deferment
and its release impact. Their presence in a document does not automatically make
them an accepted feature requirement.

- Schema/query: exhaustive/custom types and validators; join-table visibility;
  eager/batched relation loading; indexing; large-schema composition and name
  collisions; migration primitive catalogue and adoption from another database.
- Handler/queue: argument encoding; optional debug-log format; hung handlers;
  other-writer isolation; multi-hop/multiple-parent dependencies; reference
  detection and durable edges; entity-diff dependencies; unrelated-command grouping;
  reset versus queued work; cancelled in-flight responses; rollback overlapping
  later writes; acknowledged-but-not-yet-resolved recovery.
- Sync/observability: initial sync and loading visibility; checkpoint atomicity;
  same-record diff correlation; diff status visibility; sync schedule; backlog
  threshold/action; bounded history retention; exact backoff; attempt/timing fields;
  tenant/environment scoping; membership rule representation remains backend-owned.
- Security/platform: concrete per-engine cipher/library; browser encrypted storage
  feasibility; supplied-key failure behavior; key rotation scope; diagnostic-export
  trigger/schema/redaction; multi-instance locking/propagation; remaining FFI paths.
- Integration: second carrier proof; handler-drift guidance without moving business
  determinism into core; broader integration examples; release packaging and
  compatibility testing. Do not implement a server product to fill backend-owned gaps.

## 6. Per-slice follow-up procedure

For each small unit of work:

1. Recheck Git status and applicable instructions; preserve unrelated work.
2. Select the requirement/spec/test-case IDs and define a narrow stop condition.
3. Prepare the governed specification and named cases, including scope, acceptance,
   rollback, and relevant quality/security considerations. Obtain required review
   and requested commit authorization before implementation under FLOW-1–FLOW-3.
4. Add a meaningful test and observe its expected failure.
5. Implement the smallest change; confirm green; refactor only under green.
6. Run affected tests and required format/lint/type/build checks; inspect the diff.
7. Update affected formal artifacts and this tracker with actual evidence.
8. Request review/commit/release actions only within the authorization granted.

Existing documentation checks, run from repository root:

```sh
python3 tools/traceability/trace_check.py docs/design
python3 tools/traceability/rtm_gen.py docs/design --check
```

Regenerate the RTM only when formal links change. Record future implementation
commands after they exist and have actually been executed. Do not report a
manual check or intended Gherkin case as a passing automated test.

Progress entry template:

```text
Slice / requirement IDs:
Status:
Acceptance criteria:
Source sections / coverage dispositions:
Scenario and outline-example IDs:
Target engines / platforms:
Changed files:
Expected failing test and observed result:
Passing checks and observed results:
Evidence/report/commit references:
Open decisions or blockers:
Next smallest action:
```

## 7. Validation requested

Please validate this sequence and the proposed directory responsibilities before
implementation. In particular, the proposed first milestone is the isolated
browser + native-desktop spike, followed by a findings review; it does not commit
us to a native binding tool, browser engine, dependency, or public package layout.

Approval of the plan permits moving toward the next governed slice; it does not
replace dependency, contract, security, destructive-action, commit, or release
approvals required by the linked project rules.

## 8. Follow-up record

- Planning: inspected the root instructions, governance indexes and applicable
  process/change/delivery rules, roadmap, architectures, storage specifications,
  and formal test plan. Created this proposal at the user's request.
- Implementation: SPEC-0016 host-only crate added under
  `spikes/cross-platform/`; full Stage 0 remains pending.
- Initialization preparation: drafted SPEC-0016 with named host-test scenarios
  and verified local toolchain inventory. Specification review and commit were
  completed in the preparation commit `ebd4146`; five executable host cases
  observed failing at the missing insert/query operations before implementation.
- Validation: five host cases pass via `cargo test --manifest-path
  spikes/cross-platform/Cargo.toml --offline --locked`. Formatting and Clippy
  with warnings denied pass. Traceability reports zero violations; RTM check
  and `git diff --check` pass. At that initial host checkpoint no browser/native
  binding or persistence test had run; later native evidence follows below.
- Follow-up: host implementation committed as `83879ef`. The existing core
  also builds with `cargo build --manifest-path spikes/cross-platform/Cargo.toml
  --target wasm32-unknown-unknown --offline --locked`; this produces a Rust
  library, not a tested browser module. SPEC-0017 proposes the browser bridge
  and four cases. Its specification was reviewed and committed as a proposal
  in `dd6d5a2`; dependency approval and browser implementation remain pending.
- Native boundary: SPEC-0018, REQ-0035, THR-0009 and TC-0018 reviewed and
  committed before code in `dd6d5a2`. A std-only native bridge and real Swift
  host now pass N01–N05, plus three Rust N06 error-barrier tests after RED runs.
  Formatting, Clippy and Swift warnings-as-errors pass. Independent unsafe-code
  review found no blockers. Reproducible commands and ownership limitations are
  in `spikes/cross-platform/native/README.md`. That initial slice did not cover
  subscriptions or async handlers; the continuation below adds those experiments.
- Native drain/async continuation: specification and N07–N09 cases reviewed and
  committed in `0ba1d69` before tests. Five core cases and three real Swift cases
  observed RED before GREEN. Core now has 11 passing Rust tests, native has three
  Rust error-barrier tests, and the two Swift executables have five plus three
  passing scenarios. Core owns subscription membership and staged scope state;
  native notification delivery is explicit drain, not push. Scope serialization
  is per store; other-writer/liveness policies remain unresolved. See
  `docs/implementation/spike-report.md` for the partial exit matrix and latency
  evidence. No runtime dependency was added; browser and persistence gates remain.
- Phase 1 foundation (operator-requested sequencing exception, 2026-10-21):
  SPEC-0019/API-0002/TC-0019 reviewed and committed in `2625faa` before code.
  Added an independent core/memory workspace, pinned Rust 1.98.1, and observed
  all nine contract tests RED before GREEN. Workspace format, Clippy and WASM
  compilation pass. All 14 Rust spike tests and eight Swift scenarios still
  pass under the new toolchain; the spike crates remain separate workspaces.
  GitLab CI YAML parses locally, and Rust/design scripts pass locally; no
  GitLab pipeline has run. Browser, persistence and full Phase 1 gates remain
  open. See `docs/implementation/phase-one-foundation.md` for commands/limitations.
- Memory atomic batches: SPEC-0020/TC-0020 and the optional API-0002 capability
  reviewed and committed in `d0f3cb9` before tests. Seven T01–T07 cases failed
  at the missing implementation with the nine CRUD cases still green, then
  all 16 passed. Ordered mutations reuse the existing CRUD semantics on a
  private map copy; the first storage error preserves every original table.
  Format, Clippy and WASM compilation pass. This is memory atomicity only,
  not durable queue transactions, engine parity or persistent storage.

- Browser continuation: operator explicitly approved wasm-bindgen 0.2.104 and
  rusqlite 0.40.2 on 2026-10-21. Browser cases/boundary review committed in
  `1aa1ff7` before implementation. B01–B05 all failed for expected missing
  operations, then passed in Chrome 152.0.7977.76 with visible complete rows.
  Independent review, formatting, Clippy and WASM build pass. One measured
  write-to-callback sample is 3.5 ms; memory-only reload sentinel is absent
  after reload. SPEC-0021/TC-0021 persistence cases committed in `37fc08d`;
  native persistence evidence follows below.
- Native persistence: SPEC-0021/TC-0021 passed after behavioral RED. Native Rust
  suite now has eight passing cases; shared core retains 11 and original Swift
  scenarios retain eight. Separate Swift writer PID 92818 and reader PID 92912
  both exited 0 with exact Unicode/i64 values, committed scope rows and aborted
  row absence. SQLite 3.53.2; ordinary SQL failure preserves memory/disk/events
  and retryable scope. The Stage 0 report now records every exit question and
  researched platform disposition. This closes the bounded experiment, while
  production SQLite/browser adapters and complete Phase 1 remain in progress.

- Native production storage: SPEC-0022/API-0002/TC-0022 reviewed and committed
  in `5899da5` before code. Shared extraction preserved all 16 memory assertions;
  all 16 SQLite cases and additional native cases failed before implementation.
  Now 40 workspace tests pass, including real restart and failed-COMMIT rollback.
  Independent review, formatting, Clippy and explicit core/memory WASM build pass.
  Both configured Rust job scripts also passed locally from a fresh task Cargo
  cache after locked dependency fetch; remote GitLab execution is still pending.
  Persistent browser runtime SPEC-0023 was explicitly approved and is now
  implemented; actual worker recreation/page reload, typed rollback, shared
  assertions and a fresh locked build pass. See
  `docs/implementation/browser-persistence.md` and the captured full results.
  The clean-source run later exposed DEF-0001 pool cleanup; reviewed
  SPEC-0024/TC-0024 regression RED then GREEN adds acknowledged VFS shutdown
  before namespace deletion. No new agents were started.
  Stage 1 and full roadmap Phase 1 remain open: the
  test bridge is not a production package; query/change responsibilities,
  schema/types, CI runner/compiler validation and later durability/security gates
  must not be inferred from this subset.

The next Phase 1 query/change-boundary gate is
[RFD-0001](docs/design/rfd/RFD-0001-storage-query-index-ownership.md). It
recommends engine-owned physical indexes; the operator decision and concrete
query/type/change contracts are pending. SPEC-0004 explicitly requires this
indexing decision before implementation.

## 9. Formal source inventory — coverage ownership

Every registered artifact is listed below so none disappears between phases. All
implementation/verification work remains pending. For decisions, vision, objectives
and the test plan, the obligation is to enforce/read their constraints in each
relevant slice, not manufacture one implementation module per document. The
registry remains authoritative; reconcile additions/removals at each milestone.

| Source | Title | Stage ownership | Status |
| --- | --- | --- | --- |
| [VIS-0001](docs/design/vision/VIS-0001-hoyadb.md) | hoyadb — an offline-first, relational, reactive embedded database | All applicable stages | Pending |
| [OBJ-0001](docs/design/objectives/OBJ-0001-offline-operability.md) | An application built on hoyadb stays fully usable with no network | All applicable stages | Pending |
| [OBJ-0002](docs/design/objectives/OBJ-0002-one-portable-core.md) | One core implementation serves every supported platform | All applicable stages | Pending |
| [OBJ-0003](docs/design/objectives/OBJ-0003-intent-preserving-sync.md) | Writes reach the backend as intent, resolved authoritatively server-side | All applicable stages | Pending |
| [OBJ-0004](docs/design/objectives/OBJ-0004-performance-baseline.md) | Establish a performance baseline — no numeric target is set yet | All applicable stages | Pending |
| [OBJ-0005](docs/design/objectives/OBJ-0005-live-reactivity.md) | Applications observe query results live, without re-filtering a raw change feed | All applicable stages | Pending |
| [OBJ-0006](docs/design/objectives/OBJ-0006-library-not-service.md) | hoyadb stays a library — it owns local data protection and nothing beyond it | All applicable stages | Pending |
| [ADR-0001](docs/design/decisions/ADR-0001-rust-core-wasm-native-bindings.md) | Platform & language — one Rust core, compiled to WASM and exposed via native bindings | All applicable stages | Pending |
| [ADR-0002](docs/design/decisions/ADR-0002-hoyadb-owns-its-schema.md) | Data model — hoyadb owns its own relational schema | All applicable stages | Pending |
| [ADR-0003](docs/design/decisions/ADR-0003-pluggable-storage-interface.md) | Storage — one narrow pluggable interface, SQLite-family engine as the default | All applicable stages | Pending |
| [ADR-0004](docs/design/decisions/ADR-0004-reactivity-set-membership-default.md) | Reactivity — one query object, two modes, set-membership change as the default trigger | All applicable stages | Pending |
| [ADR-0005](docs/design/decisions/ADR-0005-command-first-sync.md) | Sync mechanism — named commands as the primary write path, entity-diff as fallback | All applicable stages | Pending |
| [ADR-0006](docs/design/decisions/ADR-0006-authoritative-backend-membership-scoping.md) | Topology & scoping — one authoritative backend, server-computed membership | All applicable stages | Pending |
| [ADR-0007](docs/design/decisions/ADR-0007-per-field-lww-conflict-default.md) | Conflict resolution — per-field last-write-wins, deletes win, overridable per table | All applicable stages | Pending |
| [ADR-0008](docs/design/decisions/ADR-0008-implementation-roadmap-validation-spike.md) | Sequence the work as an implementation roadmap whose first step validates the Rust core shape | All applicable stages | Pending |
| [ADR-0009](docs/design/decisions/ADR-0009-migration-chain-or-reset.md) | Schema migrations — apply an authored chain, otherwise reset the local database | All applicable stages | Pending |
| [ADR-0010](docs/design/decisions/ADR-0010-client-generated-idempotency-key.md) | Every command carries a client-generated, time-sortable idempotency key | All applicable stages | Pending |
| [ADR-0011](docs/design/decisions/ADR-0011-ack-receipt-and-outcome-enum.md) | Ack is durable receipt, separate from outcome, surfaced as one transport-agnostic enum | All applicable stages | Pending |
| [ADR-0012](docs/design/decisions/ADR-0012-correction-via-subscription-and-rollback.md) | Corrections and rejections surface through the ordinary subscription path, with an explicit rollback | All applicable stages | Pending |
| [ADR-0013](docs/design/decisions/ADR-0013-schema-version-and-versioned-command-names.md) | Every envelope declares the client schema version; command names carry a version suffix | All applicable stages | Pending |
| [ADR-0014](docs/design/decisions/ADR-0014-command-determinism-is-a-non-goal.md) | Command determinism between client and server runs is an explicit non-goal | All applicable stages | Pending |
| [ADR-0015](docs/design/decisions/ADR-0015-membership-revocation-developer-invoked-reset.md) | On membership revocation hoyadb exposes a reset operation, it does not purge on its own | All applicable stages | Pending |
| [ADR-0016](docs/design/decisions/ADR-0016-durable-offline-queue.md) | Offline writes go through a durable, state-tracked, locally-persisted queue | All applicable stages | Pending |
| [ADR-0017](docs/design/decisions/ADR-0017-server-side-tie-break.md) | Identical-timestamp conflicts are broken server-side — first received wins | All applicable stages | Pending |
| [ADR-0018](docs/design/decisions/ADR-0018-multi-device-is-an-ordinary-conflict.md) | Two devices under one identity are an ordinary conflict case, not a special one | All applicable stages | Pending |
| [ADR-0019](docs/design/decisions/ADR-0019-security-standards-scope.md) | Transport and auth standards are out of hoyadb's scope; at-rest encryption is engine-native | All applicable stages | Pending |
| [ADR-0020](docs/design/decisions/ADR-0020-developer-supplied-encryption-key.md) | The developer supplies at-rest key material; hoyadb never generates, stores or manages it | All applicable stages | Pending |
| [ADR-0021](docs/design/decisions/ADR-0021-local-only-logging-no-telemetry.md) | Command and trusted-context contents stay in local state; hoyadb sends no telemetry | All applicable stages | Pending |
| [ADR-0022](docs/design/decisions/ADR-0022-performance-baseline-then-hold.md) | Performance — establish a baseline once there is an implementation, then hold or improve it | All applicable stages | Pending |
| [ADR-0023](docs/design/decisions/ADR-0023-semver-and-reserved-namespace.md) | hoyadb's public API follows semver; its own bookkeeping uses a reserved namespace | All applicable stages | Pending |
| [ADR-0024](docs/design/decisions/ADR-0024-durability-and-footprint-scope.md) | No durability guarantees beyond the queue's; hoyadb targets a small, mobile-appropriate dataset | All applicable stages | Pending |
| [ADR-0025](docs/design/decisions/ADR-0025-async-serialized-handler-execution.md) | Command handlers are async and may read locally; hoyadb serializes them and commits each atomically | All applicable stages | Pending |
| [ADR-0026](docs/design/decisions/ADR-0026-batching-is-the-default.md) | The reference transport always sends an array; each write resolves independently | All applicable stages | Pending |
| [ADR-0027](docs/design/decisions/ADR-0027-minimal-query-model.md) | Query model — a deliberately minimal operator catalogue; indexing left undecided | All applicable stages | Pending |
| [ADR-0028](docs/design/decisions/ADR-0028-per-command-observability.md) | Observability — per-command status, manual cancel, backoff with jitter, backend-owned key retention | All applicable stages | Pending |
| [REQ-0001](docs/design/requirements/REQ-0001-declare-schema.md) | An application declares a schema of typed tables and columns | 3 | Pending |
| [REQ-0002](docs/design/requirements/REQ-0002-first-class-relations.md) | Relations are first-class schema constructs, navigable from either side | 3 | Pending |
| [REQ-0003](docs/design/requirements/REQ-0003-query-predicates.md) | A query filters with comparison operators combined as implicit AND | 3 | Pending |
| [REQ-0004](docs/design/requirements/REQ-0004-relation-traversal-in-predicates.md) | A query filters on a to-one relation's field by dot-path | 3 | Pending |
| [REQ-0005](docs/design/requirements/REQ-0005-ordering-and-limit.md) | A query orders its results and caps their count | 3 | Pending |
| [REQ-0006](docs/design/requirements/REQ-0006-migration-chain-or-reset.md) | A schema version change applies an authored migration chain, or resets the local database | 3 | Pending |
| [REQ-0007](docs/design/requirements/REQ-0007-read-once-or-subscribe.md) | One query object supports both a one-shot read and a live subscription | 4 | Phase 3 Rust + native/browser evidence; TC-0029/0030 |
| [REQ-0008](docs/design/requirements/REQ-0008-set-membership-default-granularity.md) | A subscription re-emits only on set-membership change by default | 4 | Phase 3 Rust + native/browser evidence; TC-0029/0030 |
| [REQ-0009](docs/design/requirements/REQ-0009-observation-granularity-opt-ins.md) | Coarser and finer observation granularity are available as explicit opt-ins | 4 | Phase 3 Rust + native/browser evidence; TC-0029/0030 |
| [REQ-0010](docs/design/requirements/REQ-0010-command-optimistic-apply.md) | A command applies optimistically against the local store and is queued for an authoritative re-run | 5–6 | Pending |
| [REQ-0011](docs/design/requirements/REQ-0011-handler-execution-model.md) | Command handlers run one at a time and commit their writes as one atomic transaction | 5 | Pending |
| [REQ-0012](docs/design/requirements/REQ-0012-command-envelope.md) | A command travels as an envelope of name, arguments, keys and trusted context | 6 | Pending |
| [REQ-0013](docs/design/requirements/REQ-0013-idempotency-key.md) | Every command carries a client-generated, time-sortable idempotency key, reused on retry | 6 | Pending |
| [REQ-0014](docs/design/requirements/REQ-0014-schema-version-and-command-versioning.md) | Every write envelope declares the client's schema version; command names carry a version suffix | 6 | Pending |
| [REQ-0015](docs/design/requirements/REQ-0015-trusted-context-boundary.md) | hoyadb carries an already-authenticated identity client-unforgeably and implements no authentication | 5, 8 | Pending |
| [REQ-0016](docs/design/requirements/REQ-0016-transport-agnostic-outcome.md) | A command's outcome is surfaced as one transport-agnostic status, with ack distinct from acceptance | 6 | Pending |
| [REQ-0017](docs/design/requirements/REQ-0017-correction-rollback.md) | Corrections and rejections reach the app through its subscriptions, with a rollback available | 6 | Pending |
| [REQ-0018](docs/design/requirements/REQ-0018-offline-queue-state-machine.md) | Queued writes are persisted locally and tracked through explicit states across restarts | 6 | Pending |
| [REQ-0019](docs/design/requirements/REQ-0019-dependent-writes-and-id-swap.md) | A write referencing an unresolved local id stays queued until the backend id is known | 6 | Pending |
| [REQ-0020](docs/design/requirements/REQ-0020-retry-with-backoff.md) | Unacknowledged writes are retried with exponential backoff and jitter | 6 | Pending |
| [REQ-0021](docs/design/requirements/REQ-0021-queue-introspection.md) | Queue state is introspectable in aggregate and per command | 6 | Pending |
| [REQ-0022](docs/design/requirements/REQ-0022-explicit-command-cancel.md) | A developer can explicitly cancel a queued or rejected command; hoyadb never auto-cancels | 6 | Pending |
| [REQ-0023](docs/design/requirements/REQ-0023-entity-diff-fallback.md) | Plain table writes sync as entity-diff payloads under the same authorization bar as a command | 7 | Pending |
| [REQ-0024](docs/design/requirements/REQ-0024-per-field-lww.md) | Entity-diff conflicts resolve per field, with deletes winning and a per-table override | 7 | Pending |
| [REQ-0025](docs/design/requirements/REQ-0025-timestamp-tie-break.md) | The client does not break an identical-timestamp tie; the backend does, by arrival order | 7 | Pending |
| [REQ-0026](docs/design/requirements/REQ-0026-membership-scoped-sync.md) | What a client holds is computed server-side from membership, never from UI state | 7 | Pending |
| [REQ-0027](docs/design/requirements/REQ-0027-reset-operation.md) | hoyadb exposes an explicit local reset; it never purges revoked data on its own | 7 | Pending |
| [REQ-0028](docs/design/requirements/REQ-0028-no-network-calls-transport-interface.md) | hoyadb opens no network connection; it calls a developer-supplied transport | 6, 8 | Pending |
| [REQ-0029](docs/design/requirements/REQ-0029-batched-independently-resolved-writes.md) | Writes always push as an array, each resolved independently and correlated by key | 6–7 | Pending |
| [REQ-0030](docs/design/requirements/REQ-0030-at-rest-encryption.md) | The local store is encrypted at rest using the engine's own capability and a developer-supplied key | 2, 8 | Pending |
| [REQ-0031](docs/design/requirements/REQ-0031-backup-exclusion.md) | The local store is excluded from platform device and app backups by default | 8 | Pending |
| [REQ-0032](docs/design/requirements/REQ-0032-local-only-logging.md) | Command and trusted-context contents stay in local state; no telemetry leaves the device | 8 | Pending |
| [REQ-0033](docs/design/requirements/REQ-0033-reserved-namespace.md) | hoyadb's own tables and fields use a reserved namespace that cannot collide with an app schema | 3 | Pending |
| [REQ-0034](docs/design/requirements/REQ-0034-testing-support.md) | An in-memory engine, a fake transport, and injectable clock/id make app tests hermetic | 1–9 | Pending |
| [NFR-0001](docs/design/requirements/nfr/NFR-0001-local-operation-responsiveness.md) | Local operation responsiveness — no numeric target is set yet | 9 | Pending |
| [NFR-0002](docs/design/requirements/nfr/NFR-0002-offline-operability.md) | Every documented local operation works with no network connection | 3–9 | Pending |
| [NFR-0003](docs/design/requirements/nfr/NFR-0003-queued-write-durability.md) | A queued write survives an app kill, atomically where the engine supports transactions | 2, 6 | Pending |
| [NFR-0004](docs/design/requirements/nfr/NFR-0004-local-dataset-footprint.md) | hoyadb targets a mobile-appropriate local dataset — no numeric ceiling is set | 9 | Pending |
| [NFR-0005](docs/design/requirements/nfr/NFR-0005-api-compatibility.md) | hoyadb's public API follows semver, and a local reset is never the default on an upgrade | 3, 9 | Pending |
| [THR-0001](docs/design/security/threats/THR-0001-trusted-context-forgery.md) | Trusted context forged or overwritten on the client | 5, 8 | Pending |
| [THR-0002](docs/design/security/threats/THR-0002-local-store-read-off-device.md) | Local store file read outside the application's own runtime | 2, 8 | Pending |
| [THR-0003](docs/design/security/threats/THR-0003-command-replay-double-apply.md) | A resent command is applied twice by the backend | 6 | Pending |
| [THR-0004](docs/design/security/threats/THR-0004-data-retained-after-revocation.md) | Synced data stays on a device after the client's membership is revoked | 7–8 | Pending |
| [THR-0005](docs/design/security/threats/THR-0005-unprotected-transport.md) | Sync traffic carried over an unencrypted or weakly authenticated transport | 6–8 | Pending |
| [THR-0006](docs/design/security/threats/THR-0006-unbounded-queue-growth.md) | Unbounded offline-queue growth exhausts local device storage | 6 | Pending |
| [THR-0007](docs/design/security/threats/THR-0007-command-contents-leak-via-logs.md) | Command arguments and trusted-context values leak through logs or crash reports | 6, 8 | Pending |
| [THR-0008](docs/design/security/threats/THR-0008-write-attribution-repudiation.md) | A write cannot be reliably attributed, because hoyadb only carries identity and never verifies it | 5–8 | Pending |
| [RISK-0001](docs/design/risks/RISK-0001-unprecedented-core-shape.md) | The Rust-core-to-WASM-plus-native-bindings shape has no precedent in this project's research | 0 | Pending |
| [RISK-0002](docs/design/risks/RISK-0002-command-determinism-unowned.md) | Command determinism is a responsibility boundary, not a solved problem | 5–6, 9 | Pending |
| [RISK-0003](docs/design/risks/RISK-0003-concurrent-local-store-access.md) | Concurrent access to one local store from two tabs or windows is unaddressed | 2, 4 | Pending |
| [RISK-0004](docs/design/risks/RISK-0004-indexing-undecided.md) | Indexing is undecided, and both query performance and reactive diffing depend on it | 3–4 | Pending |
| [RISK-0005](docs/design/risks/RISK-0005-bootstrap-initial-sync-unspecified.md) | Bootstrap / initial sync for a fresh install is unspecified | 7 | Pending |
| [ARC-0001](docs/design/architecture/ARC-0001-system.md) | hoyadb — system architecture | 0–9 | Pending |
| [ARC-0002](docs/design/architecture/ARC-0002-platform-bindings.md) | hoyadb — core/platform boundary and per-platform bindings | 0–9 | Pending |
| [DM-0001](docs/design/data/DM-0001-application-record.md) | Application record — a row in an app-declared table | 3 | Pending |
| [DM-0002](docs/design/data/DM-0002-command-envelope.md) | Command envelope — what travels for a domain-level write | 5–6 | Pending |
| [DM-0003](docs/design/data/DM-0003-entity-diff-payload.md) | Entity-diff payload — what travels for a plain create/update/delete | 7 | Pending |
| [DM-0004](docs/design/data/DM-0004-offline-queue-entry.md) | Offline queue entry — the persisted record of a pending write | 6 | Pending |
| [SPEC-0001](docs/design/spec/SPEC-0001-storage-interface.md) | Storage interface — the engine-agnostic persistence contract | 2 | Pending |
| [SPEC-0002](docs/design/spec/SPEC-0002-default-engine.md) | Default storage engine — SQLite-family behind the storage interface | 2, 8 | Pending |
| [SPEC-0003](docs/design/spec/SPEC-0003-schema-and-relations.md) | Schema declaration and relations | 3 | Pending |
| [SPEC-0004](docs/design/spec/SPEC-0004-query-model.md) | Query model — predicates, relation traversal, ordering and limit | 3 | Pending |
| [SPEC-0005](docs/design/spec/SPEC-0005-schema-migrations.md) | Schema migrations — chain-or-reset | 3 | Pending |
| [SPEC-0006](docs/design/spec/SPEC-0006-subscriptions.md) | Subscriptions — one query object, two consumption modes | 4 | Phase 3 Rust + native/browser evidence; TC-0029/0030 |
| [SPEC-0007](docs/design/spec/SPEC-0007-performance-guards.md) | Subscription performance guards — the coarser and finer opt-ins | 4 | Phase 3 Rust + native/browser evidence; TC-0029/0030 |
| [SPEC-0008](docs/design/spec/SPEC-0008-command-protocol.md) | Command protocol — shape, execution model, reliability and error handling | 5–6 | Pending |
| [SPEC-0009](docs/design/spec/SPEC-0009-offline-queue.md) | Offline queue — durability, ordering, dependent writes and retry | 6 | Pending |
| [SPEC-0010](docs/design/spec/SPEC-0010-entity-diff-fallback.md) | Entity-diff fallback — the plain create/update/delete write path | 7 | Pending |
| [SPEC-0011](docs/design/spec/SPEC-0011-conflict-resolution.md) | Conflict resolution — per-field LWW, deletes win, per-table override | 7 | Pending |
| [SPEC-0012](docs/design/spec/SPEC-0012-scoping-topology.md) | Scoping and topology — membership-computed sync and the reset operation | 7 | Pending |
| [SPEC-0013](docs/design/spec/SPEC-0013-transport-integration.md) | Transport integration — the developer-supplied push/pull object | 6–7, 9 | Pending |
| [SPEC-0014](docs/design/spec/SPEC-0014-security-model.md) | Security model — what hoyadb owns, and what it requires but does not implement | 2, 5, 8 | Pending |
| [SPEC-0015](docs/design/spec/SPEC-0015-queue-observability.md) | Queue observability — per-command status, cancel, and retry backoff | 6 | Pending |
| [API-0001](docs/design/contracts/API-0001-http-sync-transport.md) | Reference HTTP sync transport contract | 6–7, 9 | Pending |
| [UX-0001](docs/design/ux/flows/UX-0001-developer-integration-flow.md) | Developer integration flow — schema, subscribe, mutate, handle correction | 4–9 | Pending |
| [TP-0001](docs/design/tests/plan/TP-0001-test-plan.md) | hoyadb — test plan | All applicable stages | Pending |
| [TC-0001](docs/design/tests/cases/TC-0001-schema-and-relations.md) | Schema declaration and first-class relations | 3 | Pending |
| [TC-0002](docs/design/tests/cases/TC-0002-query-model.md) | Query predicates, relation traversal, ordering and limit | 3 | Pending |
| [TC-0003](docs/design/tests/cases/TC-0003-migrations-and-namespace.md) | Migration chain-or-reset, and the reserved namespace | 3 | Pending |
| [TC-0004](docs/design/tests/cases/TC-0004-subscriptions.md) | One-shot reads, live subscriptions, and observation granularity | 4 | Phase 3 Rust + native/browser evidence; TC-0029/0030 |
| [TC-0005](docs/design/tests/cases/TC-0005-command-execution.md) | Optimistic apply and the handler execution model | 5–6 | Pending |
| [TC-0006](docs/design/tests/cases/TC-0006-command-envelope.md) | Command envelope contents and versioning | 6–7 | Pending |
| [TC-0007](docs/design/tests/cases/TC-0007-outcomes-and-rollback.md) | Command outcomes, corrections and rollback | 6 | Pending |
| [TC-0008](docs/design/tests/cases/TC-0008-queue-durability-and-dependents.md) | Queue state machine, crash recovery, and dependent writes | 6 | Pending |
| [TC-0009](docs/design/tests/cases/TC-0009-retry-and-idempotency.md) | Retry behaviour and idempotency-key reuse | 6 | Pending |
| [TC-0010](docs/design/tests/cases/TC-0010-queue-observability.md) | Queue introspection, cancellation and the backlog threshold | 6 | Pending |
| [TC-0011](docs/design/tests/cases/TC-0011-entity-diff-and-conflicts.md) | Entity-diff payloads and conflict resolution | 7 | Pending |
| [TC-0012](docs/design/tests/cases/TC-0012-boundary-identity-and-egress.md) | The library boundary — identity carried, no network call, no telemetry | 5–8 | Pending |
| [TC-0013](docs/design/tests/cases/TC-0013-scoping-and-reset.md) | Membership-scoped sync and the explicit reset operation | 7 | Pending |
| [TC-0014](docs/design/tests/cases/TC-0014-batched-push.md) | Array-always push with independent per-write resolution | 6–7 | Pending |
| [TC-0015](docs/design/tests/cases/TC-0015-at-rest-protection.md) | At-rest encryption and backup exclusion | 2, 8 | Pending |
| [TC-0016](docs/design/tests/cases/TC-0016-testing-support.md) | In-memory engine, fake transport and deterministic injection | 1–9 | Pending |
| [TC-0017](docs/design/tests/cases/TC-0017-offline-operability.md) | Every documented local operation works with no network | 3–9 | Pending |
| [ADR-0029](docs/design/decisions/ADR-0029-documentation-artifact-root.md) | Group formal artifacts under docs/design and retain supporting guides and research separately | All applicable stages | Pending |
| [SPEC-0016](docs/design/spec/SPEC-0016-initialize-validation-spike.md) | Host typed insert/query experiment | 0 | Complete — five host cases in `83879ef` |
| [SPEC-0017](docs/design/spec/SPEC-0017-browser-validation-spike.md) | Browser fixture and subscription experiment | 0 | B01–B05 pass in a real browser; memory-only reload |
| [SPEC-0018](docs/design/spec/SPEC-0018-native-validation-spike.md) | Native boundary, drain delivery and async scopes | 0 | In progress — N01–N09 executed; spike-only evidence |
| [REQ-0035](docs/design/requirements/REQ-0035-validated-native-spike-boundary.md) | Experimental native boundary validation and ownership | 0 | In progress — tested valid-memory paths; production review pending |
| [THR-0009](docs/design/security/threats/THR-0009-native-pointer-misuse.md) | Native pointer/ownership misuse | 0 | Pending — caller pointer preconditions remain |
| [TC-0018](docs/design/tests/cases/TC-0018-native-validation-spike.md) | Native Swift integration and core semantic cases | 0 | Complete for current spike cases — see report |
| [SPEC-0019](docs/design/spec/SPEC-0019-phase-one-foundation.md) | Initial workspace and memory foundation | 1 | In progress — runtime slice passes; remote CI and remaining gates pending |
| [API-0002](docs/design/contracts/API-0002-initial-rust-storage.md) | Initial async Rust storage subset | 1 | Shared memory/native/browser CRUD and atomic batches; typed extension in API-0003 |
| [TC-0019](docs/design/tests/cases/TC-0019-memory-storage-foundation.md) | Initial memory storage contract cases | 1 | Complete for M01–M09 — nine passing tests |
| [SPEC-0020](docs/design/spec/SPEC-0020-memory-atomic-batches.md) | Optional ordered atomic memory writes | 1–2 | Implemented across memory/native/browser; TC-0020 passes |
| [TC-0020](docs/design/tests/cases/TC-0020-memory-atomic-batches.md) | Atomic commit/error rollback cases | 1–2 | Complete for T01–T07 — seven passing tests |
| [SPEC-0021](docs/design/spec/SPEC-0021-native-sqlite-persistence-spike.md) | Native SQLite restart experiment | 0 | P01–P06 pass; same core, clean restart only |
| [TC-0021](docs/design/tests/cases/TC-0021-native-sqlite-persistence-spike.md) | Restart, rollback and file rejection | 0 | Five Rust cases plus real Swift processes pass |
| [REQ-0036](docs/design/requirements/REQ-0036-validated-browser-spike-boundary.md) | Browser precision and owned snapshots | 0 | B03–B05 pass; bounded spike |
| [THR-0010](docs/design/security/threats/THR-0010-browser-boundary-coercion.md) | Browser coercion/lifetime risks | 0 | Identified; bounded mitigations tested |
| [REQ-0037](docs/design/requirements/REQ-0037-native-persistence-publication.md) | Persistent candidate publication | 0 | P03/P05 pass; bounded spike |
| [THR-0011](docs/design/security/threats/THR-0011-native-persistence-publication.md) | Stored input and publication risks | 0 | Identified; bounded mitigations tested |
| [SPEC-0022](docs/design/spec/SPEC-0022-sqlite-storage-foundation.md) | Shared memory/native SQLite subset | 1–2 | 40 workspace tests pass; remote CI pending |
| [TC-0022](docs/design/tests/cases/TC-0022-sqlite-storage-conformance.md) | Adapter parity and native failures/restart | 1–2 | Shared 16 per adapter plus eight SQLite cases pass |
| [REQ-0038](docs/design/requirements/REQ-0038-sqlite-storage-safety.md) | Native typed storage/error safety | 1–2 | Bounded cases pass; full product security pending |
| [THR-0012](docs/design/security/threats/THR-0012-sqlite-storage-input-and-atomicity.md) | Typed storage/SQL error risks | 1–2 | Identified; bounded mitigations tested |
| [SPEC-0023](docs/design/spec/SPEC-0023-browser-sqlite-persistence-proposal.md) | Persistent OPFS worker proposal | 1–2 | Implemented; real worker/page restart and cleanup pass; see browser persistence report |
| [TC-0023](docs/design/tests/cases/TC-0023-browser-sqlite-persistence.md) | Worker/page restart and failures | 1–2 | W01–W09 executed; real browser evidence in browser persistence report |
| [REQ-0039](docs/design/requirements/REQ-0039-browser-sqlite-persistence-safety.md) | Proposed browser persistence safety | 1–2 | Bounded worker lifecycle/precision mitigations tested; general security remains open |
| [THR-0013](docs/design/security/threats/THR-0013-browser-sqlite-persistence-boundary.md) | Worker/OPFS boundary risks | 1–2 | Bounded controls tested; multi-tab/crash/encryption remain unsupported |

### Current Phase 1 closure artifacts — 2026-10-22

- ADR-0030 accepts engine-owned indexes following the operator's instruction.
- SPEC-0025/API-0003/TC-0025 define the bounded typed-storage acceptance gate;
  REQ-0040 and THR-0014 cover validation and post-commit publication.
- [Phase 1 storage evidence](docs/implementation/phase-one-storage.md) records
  shared cases, regression review, index work/costs and final verification: all
  TC-0025 P01–P13 pass, with 70 host tests and real browser restart/cleanup.
- Roadmap Phase 1 storage and this plan's broader production/durability stages
  have different boundaries. Packaged bindings, remote merge CI, crash-tested
  queue durability, encryption and concurrent clients remain explicit later gates.

## 10. Scenario-level implementation checklist

These are the current scenario headings extracted from the formal `.feature`
files, not newly invented cases. Each checkbox requires its executable test and
red/green evidence before completion. An outline checkbox includes every Examples
row. Existing scenarios are a floor: the source-section review above must add
missing cases, including conformance, lifecycle, faults and newly resolved decisions.

### [TC-0001](docs/design/tests/cases/TC-0001-schema-and-relations.feature) — stages 3

- [x] A declared table exists with its typed columns and an implicit id
- [x] An undeclared table is rejected, not created implicitly
- [x] A to-one relation is navigable without writing a join
- [x] A has-many relation is navigable from the other side
- [x] A many-to-many relation is navigable from both sides

### [TC-0002](docs/design/tests/cases/TC-0002-query-model.feature) — stages 3

- [x] Each comparison operator filters as specified (all example rows)
- [x] The "in" operator matches any of several values
- [x] Multiple predicates combine as implicit AND
- [x] A dot-path filters on a to-one relation's field
- [x] Traversing a has-many relation in a predicate is rejected
- [x] Results are ordered and capped

### [TC-0003](docs/design/tests/cases/TC-0003-migrations-and-namespace.feature) — stages 3

- [x] An authored chain applies in order and preserves data
- [x] A missing chain resets the local database rather than guessing
- [x] A migration step is never inferred
- [x] A schema declaring a reserved name is rejected
- [x] hoyadb's own bookkeeping does not collide with an application table

### [TC-0004](docs/design/tests/cases/TC-0004-subscriptions.feature) — stages 4

- [x] The same query supports a one-shot read and a live subscription
- [x] Unsubscribing stops further notifications
- [x] A record entering the set re-emits
- [x] A record leaving the set re-emits
- [x] An unrelated field edit on a matched record does NOT re-emit
- [x] Count-only observation emits a number, not records
- [x] Per-field observation re-emits where the default would not

### [TC-0005](docs/design/tests/cases/TC-0005-command-execution.feature) — stages 5–6

- [ ] A command applies locally and immediately
- [ ] A command applies with no network connection
- [ ] A handler reads its own earlier writes
- [ ] A handler's awaited local read sees current local state
- [ ] Nothing outside the handler observes partial state
- [ ] Two command handlers never interleave

### [TC-0006](docs/design/tests/cases/TC-0006-command-envelope.feature) — stages 6–7

- [ ] The envelope carries the command name, args and context
- [ ] The envelope does NOT carry the local row mutations
- [ ] hoyadb attaches a time-sortable idempotency key without app code doing so
- [ ] Two distinct commands get distinct keys
- [ ] The envelope declares the client's schema version
- [ ] An entity-diff payload also declares the schema version
- [ ] A command's name carries its own version suffix

### [TC-0007](docs/design/tests/cases/TC-0007-outcomes-and-rollback.feature) — stages 6

- [ ] The outcome is surfaced as a transport-agnostic status (all example rows)
- [ ] Acknowledgement is distinct from acceptance
- [ ] No acknowledgement leaves the command queued for retry
- [ ] A correction reaches the app through its ordinary subscription
- [ ] A rejection rolls back the optimistic mutation
- [ ] Rolling back a rejected parent cascades to a blocked dependent

### [TC-0008](docs/design/tests/cases/TC-0008-queue-durability-and-dependents.feature) — stages 6

- [ ] A write moves through the queue states
- [ ] The state transition and the local mutation commit together
- [ ] A kill before the transaction commits leaves neither
- [ ] A restart recovers the last known state rather than losing the write
- [ ] A non-transactional engine claims no atomicity
- [ ] A dependent write is held back until the parent id resolves
- [ ] The local id is swapped on the authoritative outcome, not on acknowledgement

### [TC-0009](docs/design/tests/cases/TC-0009-retry-and-idempotency.feature) — stages 6

- [ ] A retry resends the identical envelope with the same key
- [ ] Retries fire automatically when a new command is issued
- [ ] Retries can be requested explicitly
- [ ] Retries back off exponentially with jitter
- [ ] Only still-unacknowledged writes are resent

### [TC-0010](docs/design/tests/cases/TC-0010-queue-observability.feature) — stages 6

- [ ] One call returns both aggregate counts and per-command entries
- [ ] A dependency-blocked command is distinguishable from an ordinary queued one
- [ ] blockedOn reports the local id the app already holds
- [ ] A recently-resolved command is still visible after leaving the queue
- [ ] Queue status is observable live
- [ ] A repeatedly rejected command is never auto-cancelled
- [ ] Explicit cancellation rolls back and cascades
- [ ] Cancelling a sent command is best-effort only
- [ ] The backlog threshold bounds the queue

### [TC-0011](docs/design/tests/cases/TC-0011-entity-diff-and-conflicts.feature) — stages 7

- [ ] An ordinary table mutation generates an entity-diff payload
- [ ] The payload carries only the fields actually written
- [ ] An entity-diff carries no idempotency key
- [ ] An entity-diff goes through the same queue as a command
- [ ] An entity-diff gets the same authorization bar as a command
- [ ] Per-field last-write-wins preserves both sides' distinct fields
- [ ] A delete beats a later update to the same record
- [ ] A table may override the default resolution
- [ ] The client attaches a timestamp but breaks no tie itself

### [TC-0012](docs/design/tests/cases/TC-0012-boundary-identity-and-egress.feature) — stages 5–8

- [ ] The trusted context is forwarded unmodified
- [ ] A command handler cannot reassign the identity
- [ ] hoyadb implements no authentication of its own
- [ ] hoyadb opens no network connection
- [ ] The transport is the only egress path
- [ ] No telemetry leaves the device
- [ ] Command contents are retained only in local state

### [TC-0013](docs/design/tests/cases/TC-0013-scoping-and-reset.feature) — stages 7

- [ ] A pull carries the opaque checkpoint back unchanged
- [ ] A first pull has no checkpoint
- [ ] The client holds what the backend returned, nothing inferred
- [ ] Scoping does not follow UI state
- [ ] The reset operation is total
- [ ] Reopening against a different local store leaves the original untouched
- [ ] hoyadb never purges revoked data on its own
- [ ] A queued write against revoked data is sent and rejected by the backend

### [TC-0014](docs/design/tests/cases/TC-0014-batched-push.feature) — stages 6–7

- [ ] A single eligible write is still pushed as an array of one
- [ ] Several eligible writes are pushed as one array in local execution order
- [ ] A command and an entity-diff can share one array
- [ ] Each entry resolves independently — no cross-write rollback
- [ ] Outcomes correlate by idempotency key, not array position
- [ ] An entity-diff outcome correlates by record id
- [ ] A missing outcome leaves its write retryable

### [TC-0015](docs/design/tests/cases/TC-0015-at-rest-protection.feature) — stages 2, 8

- [ ] The store is encrypted using the engine's own capability
- [ ] Encryption is invisible above the storage interface
- [ ] hoyadb generates and stores no key of its own
- [ ] Opening without the correct key fails rather than falling back
- [ ] The local store is excluded from platform backups by default
- [ ] An integrator can opt back into platform backups

### [TC-0016](docs/design/tests/cases/TC-0016-testing-support.feature) — stages 1–9

- [ ] The in-memory engine touches no filesystem or platform storage
- [ ] Behaviour above the storage interface is identical to a real engine
- [ ] The fake transport implements the same contract as a real one
- [ ] The fake attaches the correlation field automatically
- [ ] A rejection is simulated by the handler's return value
- [ ] A dropped ack is simulated by the push rejecting
- [ ] An injected clock makes a handler's timestamp deterministic
- [ ] An injected id generator makes local ids deterministic

### [TC-0017](docs/design/tests/cases/TC-0017-offline-operability.feature) — stages 3–9

- [ ] Declaring a schema succeeds offline
- [ ] Applying a migration succeeds offline
- [ ] A one-shot query succeeds offline
- [ ] A live subscription delivers and updates offline
- [ ] A command applies optimistically offline
- [ ] An entity-diff write applies offline
- [ ] Queue introspection succeeds offline
- [ ] Zero documented local operations require connectivity

## 11. Supporting-document reconciliation

Formal artifacts govern implementation. Supporting guides must be checked for
example parity; research and historical alternatives do not become requirements.
Do not implement every capability mentioned in a comparison or research source.

- [ ] [glossary.md](docs/guides/glossary.md): reconcile examples/terminology against implemented contracts; record runnable verification or explicit conceptual-only status.
- [ ] [overview.md](docs/guides/overview.md): reconcile examples/terminology against implemented contracts; record runnable verification or explicit conceptual-only status.
- [ ] [platform-bindings.md](docs/guides/platform-bindings.md): reconcile examples/terminology against implemented contracts; record runnable verification or explicit conceptual-only status.
- [ ] [relations.md](docs/guides/relations.md): reconcile examples/terminology against implemented contracts; record runnable verification or explicit conceptual-only status.
- [ ] [sync-implementation.md](docs/guides/sync-implementation.md): reconcile examples/terminology against implemented contracts; record runnable verification or explicit conceptual-only status.
- [ ] [testing.md](docs/guides/testing.md): reconcile examples/terminology against implemented contracts; record runnable verification or explicit conceptual-only status.
- [ ] [walkthrough.md](docs/guides/walkthrough.md): reconcile examples/terminology against implemented contracts; record runnable verification or explicit conceptual-only status.
- [ ] [OpenAPI schema](docs/design/contracts/openapi.yaml): validate applicable request/response fixtures and status/correlation constraints against the actual schema, not just the prose example.
- [ ] [Documentation history](docs/references/documentation-history.md) and [source ledger](docs/references/documentation-sources.csv): preserve provenance; consult when old paths occur.
- [ ] [Research index](docs/references/README.md): consult relevant primary evidence when selecting a mechanism; record adoption decisions separately from prior-art observations.
- [ ] README, ROADMAP, AGENTS, DECISIONS and documentation indexes: review for accuracy at delivery; preserve append-only accepted decision bodies and avoid duplicating governance rules.

Update record: strengthened mandatory TDD, source-section disposition gates,
component details, registered-artifact ownership and scenario-level follow-up.
Stage 0 host/native experiments have started; see the follow-up record and
spike report. Production implementation and full Stage 0 acceptance remain pending.

## Current roadmap Phase 2 closure — 2026-10-22

The approved RFD-0002 policies are implemented by SPEC-0026/API-0004.
TC-0026 maps the Stage 3 source sections and original TC-0001/0002/0003
scenarios to shared Rust assertions, physical SQLite fault fixtures and
independent native/browser restart checks. TC-0027/0028 permanently guard the
two review findings. See [execution evidence](docs/implementation/phase-two-data-model.md).
The older Stage 0 update above is historical; this record supersedes its
implementation-status wording, without declaring later sync/security/release
stages complete.

## Current roadmap Phase 3 closure — 2026-10-23

SPEC-0027/API-0005 implement shared incremental observation and explicit
count/column opt-ins over the Phase 2 model. TC-0029/0030 map original TC-0004
to shared memory/native/browser assertions and real Swift/main-thread callback
lifecycle tests. See [execution evidence](docs/implementation/phase-three-reactivity.md).
Roadmap Phase 4 commands/sync and Stage 5 onward remain unimplemented here.

### Phase 3 review-resolution closure — 2026-10-23

The operator accepted the corrected phase after two completed Claude reviews;
the third attempt hit its provider quota without a verdict. SPEC-0028 / TC-0031
corrections and verification are recorded in
[review-resolution evidence](docs/implementation/phase-three-review-resolution.md):
171 workspace tests and pinned-container checks, browser persistence/reload,
and native callback tests passed. No third PASS is claimed. The scheduled
review continuation was canceled. The operator directed a stop before Phase 4;
no Phase 4 specification or implementation has been created.

### Phase 4 resumed — 2026-10-23

The operator subsequently requested resuming Phase 4. The stop above records
the previous state; it no longer prohibits authorized Phase 4 preparation.
[RFD-0003](docs/design/rfd/RFD-0003-command-sync-lifecycle.md) proposes the
previously open execution, dependency, receipt/outcome, rollback and retention
policies. SPEC-0029/TC-0032 and REQ-0043/THR-0017 provide the delivery, acceptance
and integrity preparation. The operator subsequently approved those policies;
ADR-0032 adopts them. The public Rust command owner, atomic recovery storage,
push/receipt/reconciliation/inverse and owned events now pass their memory/native
SQLite checkpoint: 233 workspace tests, fmt, Clippy and core/memory WASM build.
The remaining actual host gates subsequently passed: native H01–H06, optimized
browser 27 report groups including all 14 shared cases and three page reloads,
and the full older native/browser regressions. Fresh pinned-container checks
passed 234 workspace tests plus 31 isolated spike tests, examples, fmt, Clippy
and WASM builds. Stages 5 and 6 are complete for these experimental targets.
The debug browser large-message RPC timeout remains failed debug evidence;
the complete optimized-profile run preserves all payloads/assertions/deadlines.
Stage 7 / roadmap Phase 5 has not started.
See [final evidence and limits](docs/implementation/phase-four-command-sync.md).

### Phase 5 preparation resumed — 2026-10-24

The operator requested full roadmap Phase 5 after the formatting pass. Its
uncommitted changes and user CI/skills work are preserved separately.
RFD-0004 proposes entity correlation, opt-in generated patches, comparison and
inverse metadata, atomic paged pull/bootstrap and explicit reset policies;
SPEC-0031/TC-0034 and REQ-0044/THR-0018 prepare scope, acceptance and integrity.
Policy review is in progress; operator approval has not been obtained and no
policy-dependent implementation or Phase 6 work has started.

### Phase 5 approved implementation and verification — 2026-10-24

The operator approved RFD-0004's reviewed policies. ADR-0033, API-0009 and
TC-0034 supersede the earlier preparation-only entry above. Generated entity
patches, durable comparison metadata, typed correction clocks, guarded paged
pull/bootstrap and explicit reset/reconfiguration are complete for the existing
experimental targets. A demonstrated optimistic-filtered checkpoint-loss hazard
was fixed and tested on memory, native SQLite, actual OPFS and actual Swift.
One bounded Claude Opus 5 checkpoint and a separate post-fix closure review pass.

Final verification: 289 workspace + 37 isolated Rust tests passed on the host
and again in a fresh pinned Linux container; all formatting/Clippy/example/WASM
gates passed. Actual Swift passed 24 groups and actual optimized browser passed
116 report groups, with source/artifact hashes and cleanup verified. Traceability
has 0 violations and RTM is current. Stage 7 is complete; Phase 6 has not started.
See [full execution evidence](docs/implementation/phase-five-entity-sync.md).


### Phase 6 — reviewed security preparation and independent hardening (2026-10-24)

Preparation `6f5ffb4` adds RFD-0005/REQ-0045/THR-0019/SPEC-0032/API-0010/TC-0035
and a frozen exact dependency review packet. Recommended policies are authorized;
explicit new runtime dependency approval remains pending. No crypto dependency
has been activated. Independent hardening implements fail-closed missing-key Rust
opens, explicit legacy fixtures, bounded diagnostics/redacted Debug, immutable
JS context, static foreign errors and real Swift backup-policy preparation.
See [Phase 6 status](docs/implementation/phase-six-security.md) for observed
checks, active RED integration case and remaining gates. Stage 8 stays open;
Phase 7 has not started. User formatting/CI work remains separate and preserved.


### Phase 6 approved protection and verification — 2026-10-24

The operator approved the exact reviewed graph after the earlier preparation
entry. ADR-0034 adopts SQLCipher 4.19.0, supplied-key protected storage and the
bounded diagnostics/backup policies. Actual native encrypted engine tests,
Swift backup/protected restart and OPFS protected/reload cases pass; full upgraded
workspace verification passes 347 tests. A fresh pinned Linux container also
passes workspace, spike, formatting, Clippy, examples and WASM checks. Claude
Opus's final crypto review passes without blocking issues; execution and review
scopes and residual limits are recorded in [Phase 6 evidence](docs/implementation/phase-six-security.md).

Stage 8 is complete for the existing experimental native/Swift/browser targets.
No user stores were converted and no package was published. User formatting/CI
work remains separate. The operator subsequently requested full Phase 7 with
recommended policies and a final Claude cross-review. Phase 7 retains separate
binding, packaging, measured-budget, exact-dependency and actual-mobile gates.

### Phase 7 general binding implementation checkpoint — 2026-10-24

Stage 9 has started under SPEC-0033/API-0011/TC-0036/0037. The shared facade and
real memory/protected adapters are being exercised through WebAssembly, native
C ABI and Swift consumers. Focused regressions cover retained-memory migration,
complete reply preflight and host boundaries. This is not full Stage 9 acceptance:
exact new dependency and historical ID-example decisions, actual mobile targets,
consumer packages, measured budgets and final cross-review remain open. See the
[checkpoint evidence index](docs/implementation/phase-seven-platform-sdk.md).


### Phase 7 approved activation and consumer checkpoints — 2026-10-24

ADR-0035 records the operator-approved exact dependency bundle, restricted builds
and exact ordinary/command ID clarification; these supersede the earlier open
approval wording above. The [current evidence index](docs/implementation/phase-seven-platform-sdk.md)
distinguishes shared facade, installed JavaScript, native C/JNI and Swift
checkpoints from the still-open mobile, measurement and final-review gates.
Swift35 tests and ordinary macOS writer/reader pass; saved Swift lint failed and
current iOS runtime remains unknown. Kotlin development logs record22 passing
groups, while final AAR/APK, complete fake/scheduler and restart gates remain open.
Native cold/resource measurements are retained; full warm measurements are in
progress under a quiet window. No numerical budget or complete Phase7 support
claim has been accepted. Additional official Android plugin build graphs require
an exact reviewed addendum before activation. Formatting/CI work stays separate.


### Phase 7 correction and Opus5.5 implementation handoff — 2026-10-24

The exact Android build-plugin addendum03048bb is explicitly approved and recorded
at8a87389. Runtime pins and restricted build controls remain unchanged; the lazy
formatter and JDK downloads are excluded. JavaScript DEF15 is fixed/verified at
69f43d7. Swift public helpers/lifetime fixes16c15b8/cf3f03a pass44 tests, separate
strict formatting and ordinary public macOS writer/reader checks; current iOS
runtime remains open. Kotlin's frozen full-D8 handoff records12 focused checks
and foundation8 split/full equivalence, with final AAR/notices/23-case APK open.

The full native warm attempt was explicitly interrupted and preserved as failed;
145 completed raws are unchanged, no accepted full baseline or numeric budget
exists. Reviewed between-worker admission86c7420 precedes further long runs.
The [updated evidence index](docs/implementation/phase-seven-platform-sdk.md) and
[precise Opus5.5 lane ownership](docs/implementation/phase-seven-opus-delegation.json)
record three implementation successors and the separate final reviewer. Existing
Codex lanes are frozen; root retains Git/integration. Formatting, Clippy, WASM
and trace/RTM pass; the new broad workspace regression run is still active.
Stage9 remains open until actual mobile/package/performance and final review gates
pass. User formatting/CI/.agents work remains separate and no publication occurs.


The broad post-fix workspace run has now completed:381 tests pass, with unchanged
relevant source hashes. [Exact command/log evidence](docs/implementation/phase-seven-workspace-verification/README.md)
also records Clippy and core/memory WASM success. This clears the broad workspace
regression gate for that source cut; mobile/package/performance acceptance and
the independent final review remain open under the Opus5.5 implementation lanes.
