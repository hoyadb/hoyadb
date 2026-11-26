# hoyadb — Implementation roadmap

The sequenced plan for *building* hoyadb, starting with the Rust/WASM/native-binding
validation spike required by
[ADR-0008](docs/design/decisions/ADR-0008-implementation-roadmap-validation-spike.md).
The roadmap was originally written on 2026-09-18 from the plain-Markdown design
and decision record, before their formalization into governance artifacts.

## Design authority and historical sources

The formal artifacts indexed by [spec/registry.yaml](docs/design/spec/registry.yaml) are
the source of truth for current decisions and behavior. Start with the
[product vision](docs/design/vision/VIS-0001-hoyadb.md),
[system architecture](docs/design/architecture/ARC-0001-system.md),
[platform bindings](docs/design/architecture/ARC-0002-platform-bindings.md), and
[decisions log](DECISIONS.md); the [specifications](docs/design/spec/),
[contracts](docs/design/contracts/), and [test plan](docs/design/tests/plan/TP-0001-test-plan.md)
detail the behavior and verification requirements.

This roadmap describes sequencing and remains plain Markdown rather than a
registered governance artifact. Each implementation change follows the
[public contribution workflow](CONTRIBUTING.md#change-workflow).

The phase links below lead to current formal artifacts or supporting guides.
Historical citations retain the original reasoning behind the roadmap; their
source paths resolve at the pinned baseline through the
[historical source index](docs/references/documentation-history.md#historical-citations). The completed
[original documentation-writing plan](https://gitlab.com/alocas-ia/hoyadb/-/blob/1647b178d4fb3ee925d574367d2f9d526a4bb116/legacy/PLAN.md)
is preserved in Git history and is separate from this implementation roadmap.

## The bet this roadmap is testing

legacy/VISION.md's "Riskiest assumptions" names two compounding risks. Only one of
them is a roadmap problem:

- **The Rust core has no precedent anywhere in this project's own
  research** — all 8 systems studied ship a JS/TS core; none validate the
  Rust-core-to-WASM-plus-native-bindings shape. legacy/VISION.md handles this "by
  giving them a place to be tested, not by declaring them resolved," and
  that place is Phase 0 below.
- **Command determinism** — resolved in legacy/VISION.md as an explicit
  responsibility boundary (the integrating developer's problem), not a
  technical problem hoyadb closes. Phase 0 does *not* try to solve it. It
  only has to prove the *mechanism* a handler runs through exists across
  the boundary (see "Hard problem B").

## Phase 0 — the validation spike

The concrete, well-specified part of this roadmap. Everything after it is
outline depth on purpose: what Phase 0 finds will reshape it.

### Scope

Taken from legacy/VISION.md's own framing, not invented here: "a minimal,
code-level spike (one table, no relations, no sync)", proving "a Rust core
compiles to WASM and runs in a browser, and to at least one native
binding" (legacy/PLAN_DECISIONS.md D0's question text).

In scope:

- One Rust core crate. One hardcoded table, a handful of typed columns.
- Insert, and a single-predicate query over that table.
- One subscription on that query, notifying platform-side code.
- One command-shaped call whose handler is platform-side developer code.
- A web/WASM build, and at least one native-binding build.

Explicitly out of scope for Phase 0 — these are later phases, and pulling
them in defeats the point of the spike being minimal:

- Relations, migrations, the full query model, indexing.
- Any sync: no transport, no queue, no backend, no conflict resolution.
  The "command" in Phase 0 is only the local half (optimistic apply), used
  as a vehicle for the callback question in Hard problem B.
- At-rest encryption, backup exclusion, key management.
- API ergonomics, packaging, published artifacts, docs for consumers.

### Exit criteria

Phase 0 is done when each of these has an answer *recorded with evidence*,
not when the code looks finished. A "no" is a legitimate result and feeds
the "What could kill or reshape this" section below.

1. **Core builds.** One crate, one table, insert + query, `cargo test`
   green on the host platform.
2. **Web.** That crate, built for `wasm32`, loads in a real browser page;
   an insert and a query run and the returned rows are visible in the
   browser console.
3. **Native.** The *same* crate, unmodified, called from at least one
   native binding path (see "Binding options" below) on a real
   device/simulator or a real desktop host process, running the same
   insert and query.
4. **Persistence.** Data written survives a process restart on the native
   target. On web, whether it survives a page reload — and through what
   storage mechanism — is itself a finding to record, not an assumption
   (see Risk R3).
5. **Reactivity.** A write made from platform-side code causes a
   platform-side subscription callback to fire, on both the web and the
   native target, with an observed end-to-end latency figure written down
   (a number, not "felt fast"). See Hard problem A.
6. **Command handler callback.** Platform-side async developer code is
   invoked in connection with a core operation, can call back into the
   core for a read, and the core applies the handler's writes as one unit
   after it returns — the contract
   [legacy/docs/06-sync/command-protocol.md](docs/design/spec/SPEC-0008-command-protocol.md)'s
   "Handler execution model — resolved (D17)" fixes. On at least one
   target. See Hard problem B.
7. **A written spike report** answering each of the above, plus every open
   question this file marks as unresolved, and identifying the current formal
   artifacts affected by the findings. That report informs Phase 1 planning
   and any governed changes to requirements, architecture, specifications, or
   decisions. Changes to accepted ADRs require superseding ADRs; legacy sources
   remain historical background.

### Binding options, and what's actually known about them

legacy/VISION.md and [legacy/docs/02-architecture/platform-bindings.md](docs/guides/platform-bindings.md)
fix the *shape* (one Rust core, WASM for web, native bindings for
mobile/desktop) and explicitly leave the toolchain open:
platform-bindings.md's "What's still open" says "the concrete FFI mechanism
per native platform (raw C ABI, UniFFI, or another binding-generation tool)
is implementation-time detail". So the options below are options, evaluated
in Phase 0, not a decision this file makes.

Everything asserted here was checked on 2026-09-18 via the GitHub API or by
fetching the page named; anything not checked is marked as such.

**Web — `wasm-bindgen` / `wasm-pack`.** `wasm-bindgen/wasm-bindgen`
("Facilitating high-level interactions between Wasm modules and
JavaScript", 9153 stars, last push 2026-09-18, not archived) and
`wasm-bindgen/wasm-pack` (7283 stars, last push 2026-08-12, not archived).
Both active. This is the default candidate for the web target; the spike
should confirm the JS→Rust call direction *and* the Rust→JS push direction
(Hard problem A), not just the first.

**iOS/Android native — UniFFI.** `mozilla/uniffi-rs`, "a multi-language
bindings generator for rust" (4974 stars, last push 2026-09-18, not
archived). Tradeoff: generates idiomatic per-language wrappers (the Swift
and Kotlin surfaces platform-bindings.md sketches), at the cost of a
codegen layer between the core and the app. Whether it offers a
first-class JS/React-Native target, and what it supports for callbacks
*from* Rust into the host language, was **not checked this session** —
both are Phase 0 questions.

**iOS/Android native — plain C ABI.** Hand-written `extern "C"` surface
plus hand-written Swift/Kotlin wrappers. Tradeoff: no codegen dependency
and full control over the callback mechanism, at the cost of writing and
maintaining N wrappers by hand — which is exactly the per-platform
duplication legacy/VISION.md's "What we deliberately avoid" warns about, though
here it's translation code rather than reimplemented logic.

**React Native — craby (craby.rs).** D0 named this as a candidate,
"unconfirmed, needs checking". Checked — findings below.

**Desktop.** platform-bindings.md already routes desktop by shell: an
Electron-style shell down the web/WASM path, a Tauri-style shell down the
native path. No separate binding decision needed for Phase 0; whichever of
the two Phase 0 exercises, it's covered by criterion 2 or 3. A Node-native
addon path (N-API) exists as a third option but was **not checked this
session**.

#### craby.rs — what the check found

Checked 2026-09-18 at <https://craby.rs/>,
<https://craby.rs/docs/get-started/introduction>,
<https://craby.rs/docs/guides/stateful-modules>,
<https://github.com/leegeunhyeok/craby>, and the GitHub API for
`leegeunhyeok/craby` and `leegeunhyeok/craby-kv`.

**What it is.** The repo describes itself as "Type-safe Rust for React
Native—auto generated, integrated with pure C++ TurboModule"; the site
calls it "a type-safe Rust integration framework for React Native
developers" that generates bindings from TypeScript schemas and bridges to
Rust through pure C++ TurboModules, bypassing the ObjC/Java TurboModule
layers. So: yes, it is genuinely a Rust-core-to-React-Native binding
generator — the category D0 hoped it was.

**Maintenance/maturity.** Repo created 2025-07-16, MIT, not archived, 240
stars, 12 forks, 3 watchers, last push 2026-09-02 (recent commits are
`chore: update template`, `chore: update dependencies`). Latest release is
`v0.1.0-rc.8`, published 2026-08-15; every release tag is `0.1.0-rc.*` or
`0.1.0-beta.*` — **there is no 1.0**. Read as: actively worked on, recent,
but pre-1.0 and effectively single-maintainer.

**The finding that matters most.** Its own introduction page lists, under
"When to Use Craby", a "Cannot be used for" list whose second entry is,
verbatim: "❌ Modules requiring standard TurboModule capabilities (e.g.,
event emitters, platform context access)". Event emitters are precisely the
Rust→JS push direction hoyadb's reactivity needs (Hard problem A). Its
positive list is "✅ Performance-critical native modules (e.g., large-scale
data processing, cryptographic operations, real-time parsing)", "✅
High-performance file I/O operations with large datasets", "✅ Building
high-performance modules in Rust with type safety" — all call-in-and-return
shapes. **Assessment: plausible for the JS→Rust call direction; on its own
documented limitations, it does not currently cover the Rust→JS event
direction hoyadb depends on.** Whether that's a hard architectural limit or
a not-yet-built feature is an open question the spike must settle with the
project (its `/docs/get-started/limitations` page returned HTTP 404 when
fetched, so the full limitations text was not read).

**Supporting facts, weaker evidence.** Its docs' "Stateful Modules" guide
says "Each module instance maintains its own state, allowing you to store
and access data across multiple method calls" — relevant, since hoyadb
needs a long-lived Rust-side database handle, not stateless calls; that
page says nothing about concurrency or threading. Requirements, as reported
by a fetch of the repo page rather than read verbatim by me: React Native
0.76+ with the New Architecture enabled, Node 18+, Rust 1.90+, Xcode 15.1+
for iOS, Android NDK for Android. The same author's `leegeunhyeok/craby-kv`
("Embedded-level lightweight key-value storage for React Native", 3 stars,
last push 2025-11-05) is an existence proof that a Rust storage module
ships through craby, but a very small one.

**How Phase 0 should treat it.** Evaluate, don't adopt. It's the leading
named candidate and worth a real spike attempt, but it is pre-1.0, has one
maintainer, sits on hoyadb's critical path, and legacy/VISION.md's "What we
deliberately avoid" includes "Depending on a component for a role that an
actively maintained alternative already fills better ... checked at
adoption time, not assumed from reputation." That check is Phase 0's job,
and it has to include the event-emitter gap above.

## Hard problem A — reactivity across the core/platform boundary

The first of the two things D0 called the difficult part: "the data we keep
in Rust vs. the data kept outside it — how to handle reactivity across that
boundary".

What's already fixed, and isn't reopened here:
[legacy/docs/05-reactivity/subscriptions.md](docs/design/spec/SPEC-0006-subscriptions.md)
fixes one query object with two modes (`.fetch()` / `.observe()`) and a
default that only re-emits on set-membership change, and names the
non-trivial engineering that implies (incremental result-set diffing;
sharing one subscription across N identical `.observe()` calls).
platform-bindings.md keeps that diffing *inside* the core and says a
binding "only translates calls in and events out" — so the question is not
where the logic lives, it's how the "events out" half physically works per
platform.

What Phase 0 has to surface (none of these has an answer in `legacy/docs/` — the
platform-bindings.md section "Reactivity across the core/platform boundary"
explicitly leaves it "unresolved here"):

- **Push vs. poll.** Can the core push a notification into platform code
  (a JS callback invoked from WASM, a Swift/Kotlin closure invoked over
  FFI), or does a platform have to poll/drain a queue the core fills?
  Record which platforms support which.
- **Thread and runtime affinity.** Which thread does a notification arrive
  on, and what does the binding have to do to land it somewhere a UI
  framework can consume (a React render cycle, the main thread on
  iOS/Android)?
- **What actually crosses.** The whole new result set, a diff, or just a
  "your query changed, re-read it" signal? This is the data-in-Rust vs.
  data-outside-Rust tradeoff D0 named, and it's a serialization-cost
  question the spike can measure rather than argue.
- **Cost per notification.** Serialization and boundary-crossing overhead
  per emit, with N subscriptions active. Criterion 5 above wants a number.
- **React Native specifically.** Given the craby event-emitter finding
  above, how does a notification reach JS at all on RN? Candidate answers
  to test: a craby feature that exists but isn't on its introduction page;
  a hand-written TurboModule/JSI event-emitter layer alongside the
  craby-generated call surface; a polling/drain loop driven from JS; or a
  different binding path for RN entirely.

Phase 0 does not have to *pick* the mechanism for every platform. It has
to prove at least one works end-to-end on web and one native target, and
report honestly where it couldn't.

## Hard problem B — the command handler callback mechanism

The second thing D0 called difficult: "the command part where we need
external/developer code. Maybe a callback mechanism or something similar."

What's already fixed:
[legacy/docs/06-sync/command-protocol.md](docs/design/spec/SPEC-0008-command-protocol.md)
specifies that a command handler is the *integrating developer's own code*,
is `async`, and takes a `db` parameter supporting awaited reads as well as
writes; and its D17 section fixes the contract around it — only one handler
runs against the local store at a time, its `db` access goes through a
tracked object (read-your-own-writes inside the handler, nothing visible
outside it), and its writes commit as one atomic ordered transaction when
it returns. platform-bindings.md then says plainly that "the concrete
FFI/WASM mechanics of that callback ... are still unresolved — D17 fixes
the ordering/atomicity contract, not the binding mechanism that has to
implement it."

So the shape of the problem is: the core is Rust, the handler is
JS/Swift/Kotlin, the handler must be able to re-enter the core mid-run for
reads and writes, and the whole thing must be serialized and atomic. That's
a re-entrant, asynchronous, bidirectional boundary crossing — strictly
harder than the one-way "events out" of Hard problem A.

Candidate approaches to evaluate in Phase 0, deliberately stated as
options, not a decision:

- **Core-driven callback.** The core exposes a trait/interface that the
  binding implements; the core calls it and awaits it, passing a handle
  the platform code uses to re-enter the core. Closest to the API surface
  command-protocol.md already sketches. Question: does each target's
  binding mechanism support an *async* host-language callback invoked from
  Rust, with re-entrancy, at all? (For RN via craby: unknown, and its
  documented exclusion of event emitters is a warning sign for any
  Rust→JS-initiated flow.)
- **Platform-driven, core passive.** The platform side drives: it opens a
  handler scope on the core, runs the developer's async function itself,
  issues reads/writes into the core through ordinary in-calls, then closes
  the scope, at which point the core commits. Every crossing is JS→Rust
  (the direction every binding path supports best); the cost is that the
  D17 guarantees (serialization, atomic commit, nothing visible outside
  until return) must be enforced by core state machine + scope handle
  rather than by the core owning the call stack. Cheapest to build on a
  binding with no callback support; needs care so a platform that
  misbehaves can't violate the contract.
- **Queue/message-pump.** The core enqueues "run handler X with args Y"
  and platform code drains and replies. Works even where no callback
  mechanism exists; adds latency and a pump the app has to keep running.
- **Embedded scripting runtime.** Run handler code *inside* the core.
  command-protocol.md's determinism section already floats this ("an
  embeddable scripting language/runtime that runs identically inside the
  Rust core and on whatever the backend is") and is careful to mark it
  "flagged as worth researching, not as a decided direction — no such
  runtime is named or committed to here". This roadmap keeps that status;
  it would remove the callback problem and the determinism problem at
  once, at the price of a much larger commitment and a worse developer
  experience.

Phase 0's job: implement one of these for one target well enough to show
the D17 contract holds, and report what the other targets' binding
mechanisms actually permit. Picking the cross-platform answer is a Phase 4
decision informed by that report, not a Phase 0 deliverable.

There are also three risks command-protocol.md's "What's still open"
already flags that this mechanism will run into — a stuck/never-resolving
handler under global serialization, whether finer-grained serialization is
needed, and whether the serialization guarantee also excludes entity-diff
writes and in-progress pulls. Phase 0 doesn't have to solve them; the spike
report should say which of them its chosen mechanism makes easier or harder.

## Sequencing after the spike

Outline depth, deliberately. Phase 0's findings will reshape this —
especially phases 3 and 4, which are where the two hard problems land for
real. Each phase links to the current artifacts or guides it is built against.

- **Phase 1 — storage.** The storage interface and the SQLite-family
  default engine, plus the in-memory/fake implementation the interface
  already requires. Current implementation and acceptance evidence are tracked in
  [Phase 1 storage](docs/implementation/phase-one-storage.md). Against
  [legacy/docs/04-storage/storage-interface.md](docs/design/spec/SPEC-0001-storage-interface.md),
  [legacy/docs/04-storage/default-engine.md](docs/design/spec/SPEC-0002-default-engine.md),
  [legacy/docs/09-testing.md](docs/guides/testing.md).
  The historical `rusqlite`/`redb` choice is now resolved for the internal
  foundation by the explicitly approved native and browser SQLite experiments
  (SPEC-0022/0023). Public release requirements remain separate.
- **Phase 2 — data model.** Schema declaration and typing, relations, the
  query model, migrations. Against
  [legacy/docs/03-data-model/](docs/design/spec/) (all four files). Indexing,
  and sort/pagination beyond a single `limit(n)`, are disclosed as open in
  query-model.md; index ownership is now fixed by ADR-0030, while pagination
  beyond limit remains open.
  **Implemented for the approved public Rust scope:**
  [SPEC-0026/API-0004 evidence](docs/implementation/phase-two-data-model.md)
  maps schema, lazy relations, queries and atomic lifecycle cases across memory,
  native SQLite and real browser SQLite. RFD-0002/ADR-0031 fix explicit through
  mappings and queue-free reset guards. Packaged platform SDKs remain Phase 7.
- **Phase 3 — reactivity.** Incremental result-set diffing, subscription
  sharing, the coarser/finer opt-ins, and the boundary mechanism Hard
  problem A settled on. Against
  [legacy/docs/05-reactivity/subscriptions.md](docs/design/spec/SPEC-0006-subscriptions.md)
  and [legacy/docs/05-reactivity/performance-guards.md](docs/design/spec/SPEC-0007-performance-guards.md).
  **Implemented for the approved scope:**
  [SPEC-0027/API-0005 evidence](docs/implementation/phase-three-reactivity.md)
  records incremental projected caches, normalized query sharing, membership and
  column events, throttled counts, error recovery and actual browser/Swift delivery.
  Host-driven drains retain explicit scheduling and lifecycle responsibilities.
- **Phase 4 — command sync.** Command definition and optimistic local
  apply, the handler execution model, the offline queue and its state
  machine, id-mapping for dependent writes, outcomes/rollback, and the
  reference HTTP transport. Against
  [legacy/docs/06-sync/overview.md](docs/design/architecture/ARC-0001-system.md),
  [command-protocol.md](docs/design/spec/SPEC-0008-command-protocol.md),
  [offline-queue.md](docs/design/spec/SPEC-0009-offline-queue.md),
  [transport-http-example.md](docs/design/contracts/API-0001-http-sync-transport.md),
  [implementation-guide.md](docs/guides/sync-implementation.md). The
  largest phase by a wide margin.
- **Phase 5 — entity-diff fallback and conflict resolution.** The
  lower-ceremony write path, per-field LWW with delete-outranks-update,
  per-table overrides, and scoping/topology. Against
  [legacy/docs/06-sync/entity-diff-fallback.md](docs/design/spec/SPEC-0010-entity-diff-fallback.md),
  [conflict-resolution.md](docs/design/spec/SPEC-0011-conflict-resolution.md),
  [scoping-topology.md](docs/design/spec/SPEC-0012-scoping-topology.md).
  Completed for the current experimental Rust/Swift/browser targets in
  [Phase 5 evidence](docs/implementation/phase-five-entity-sync.md), including
  atomic paged bootstrap and explicit reset. Production packaging remains later.
- **Phase 6 — security hardening.** At-rest encryption per engine (the
  concrete cipher/library is the one item legacy/VISION.md's "Deferred to
  implementation time" holds back for exactly this moment), backup/export
  exclusion, key-material configuration, logging/telemetry redaction.
  Against [legacy/docs/07-security/security-model.md](docs/design/spec/SPEC-0014-security-model.md).
- **Phase 7 — binding breadth, observability, benchmark baseline.** The
  remaining platform bindings beyond the one or two Phase 0 proved;
  per-command status introspection and retry backoff; and establishing the
  benchmark baseline
  [legacy/docs/08-non-functional/performance.md](docs/design/objectives/OBJ-0004-performance-baseline.md)
  defines as the near-term performance process. Against
  [legacy/docs/02-architecture/platform-bindings.md](docs/guides/platform-bindings.md),
  [legacy/docs/08-non-functional/observability.md](docs/design/spec/SPEC-0015-queue-observability.md),
  [legacy/docs/08-non-functional/performance.md](docs/design/objectives/OBJ-0004-performance-baseline.md).

Two cross-cutting items that don't belong to a single phase:
**concurrent access to one local store from two browser tabs or two desktop
windows**, which platform-bindings.md's "What's still open" calls "a real,
disclosed gap" with no locking, leader election, or cross-instance
propagation designed; and keeping the formal design aligned with implementation.
The original documentation formalization is complete; future changes follow the
[public contribution workflow](CONTRIBUTING.md#change-workflow).

## What could kill or reshape this approach

legacy/VISION.md's "Riskiest assumptions" frames the Rust-core shape as tested,
not proven. These are the concrete ways Phase 0 could come back negative,
and what each would force.

- **R1 — Reactivity latency or mechanism fails on a platform.** If change
  notifications can't get from the core into platform code fast enough, or
  at all, on a target that matters. React Native is the live suspect,
  given craby's documented exclusion of event emitters. *Reshape:* a
  different RN binding path; a polling/drain model with a worse latency
  profile; or, worst case, a platform that gets a reduced reactivity
  guarantee — which would contradict subscriptions.md's current promise
  that every query can be subscribed to, and would need that doc changed
  rather than quietly under-delivered.
- **R2 — No workable re-entrant async callback for command handlers.** If
  no binding mechanism on a target supports the D17 contract, the command
  path — legacy/VISION.md's *primary* sync unit — can't be delivered as specified
  there. *Reshape:* the platform-driven/core-passive option in Hard
  problem B; or, as a genuine fallback already sanctioned by
  [legacy/NEEDS.md](https://gitlab.com/alocas-ia/hoyadb/-/blob/1647b178d4fb3ee925d574367d2f9d526a4bb116/legacy/NEEDS.md) ("if it proves impractical, a more conventional
  change/entity-based sync ... is an acceptable fallback"), lean harder on
  the entity-diff path on the affected platform. That would be a
  significant walk-back of legacy/VISION.md's chosen direction, so it should be
  an explicit decision with its own record, not a drift.
- **R3 — The default storage engine doesn't work on web.** Whether the
  SQLite-family default engine (`rusqlite` per default-engine.md) builds
  for `wasm32` and persists durably in a browser was **not verified in
  this session** and is a Phase 0 exit criterion for a reason. *Reshape:*
  a different concrete engine behind the same storage interface on web —
  which storage-interface.md's pluggability is explicitly designed to
  absorb, so this is the cheapest of these risks to survive, but it does
  mean two engines to maintain from day one.
- **R4 — Binding-toolchain dependency risk.** craby is pre-1.0,
  single-maintainer, 240 stars, and would sit on hoyadb's critical path
  for React Native; UniFFI's suitability for the callback direction is
  unchecked. legacy/VISION.md's avoid-list forbids depending on a component where
  "an actively maintained alternative already fills [the role] better",
  checked at adoption time. *Reshape:* hand-written C ABI + TurboModule
  layer for RN, at higher maintenance cost.
- **R5 — The compounding bet.** legacy/VISION.md's own framing: an unprecedented
  core-language shape *and* a sync model with an unsolved correctness
  property is "a materially bigger combined bet than either alone." Phase
  0 tests the first only. If Phase 0 comes back mixed rather than clean —
  works on web, marginal on mobile — the honest options are narrowing the
  target platforms, or revisiting the Rust-core decision itself before
  Phase 1 rather than after Phase 4.
- **R6 — Multi-instance concurrency.** Two tabs or two windows on one
  store is undesigned (see above) and is a routine scenario on web and
  desktop, not an edge case. It won't kill the approach, but discovering
  its cost late could reshape the storage and reactivity layers after
  they're built — worth a deliberate look during Phase 0 or 1 rather than
  Phase 7.
- **R7 — Toolchain and CI cost.** N build toolchains (wasm32, iOS/Xcode,
  Android NDK, desktop hosts) for one crate, from the first commit. Not a
  correctness risk; a sustained-throughput one, and Phase 0 is where its
  real size becomes visible.

## Open questions this roadmap does not answer

Collected so they're not buried in prose above. Each is unresolved as of
2026-09-18 — not a guess deferred, an actual unknown.

- Whether craby supports, or plans to support, any Rust→JS event/callback
  path despite its introduction page excluding event emitters. Its
  `/legacy/docs/get-started/limitations` page 404'd when fetched, so its full
  limitations text is unread.
- Whether UniFFI offers a React Native / JS target, and what it supports
  for callbacks from Rust into the host language. Not checked.
- Whether React Native's JS engine can run WASM directly (which would
  collapse the RN binding question into the web one). Not checked.
- Whether a Node N-API path is worth having for Electron-style desktop
  alongside the WASM path. Not checked.
- Whether `rusqlite` builds for `wasm32` and persists durably in a
  browser, and through what mechanism. Not checked.
- Which of `rusqlite` or a hand-rolled layer over `redb` becomes the
  concrete default engine — default-engine.md narrows, doesn't pick.
- The three handler-execution risks command-protocol.md flags as open:
  stuck-handler liveness, finer-grained serialization, and the
  serialization guarantee's scope against entity-diff writes and pulls.
- Indexing, and sort/pagination beyond a single `limit(n)` — open in
  query-model.md.
- Cross-tab/cross-window concurrent access — open in platform-bindings.md.
- The concrete at-rest cipher/library per engine — deliberately held for
  Phase 6 by legacy/VISION.md's "Deferred to implementation time".
