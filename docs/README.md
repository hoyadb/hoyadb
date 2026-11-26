# hoyadb documentation

- [Contributing](../CONTRIBUTING.md): setup, required checks and review workflow.
- [GitHub delivery](guides/github-delivery.md): CI, publishing, native runner
  prerequisites, registry setup and GitHub Pages.
- [Repository layout](guides/repository-layout.md): package boundaries, public
  tooling and the historical source-path map.
- [Repository preparation verification](implementation/public-repository-layout.md):
  executed checks, preservation evidence and remaining release gates.
- [Runnable tutorials](../examples/README.md): the source examples used below.
- [AI integration catalog](../AI.md): expected remote installation and application
  implementation procedure, portable skill, versioned references and ID boundaries.

- **[Public HTML documentation](public/dist/index.html)**: getting started, concepts,
  practical guides and generated public reference. Build and test it using
  [docs/public/README.md](public/README.md). The generated site is local build output.

Start with the [formal design reading path](design/README.md), which is the
source of truth for current product decisions. The
[implementation roadmap](../ROADMAP.md) sequences the build, beginning with
validation of the Rust/WASM/native-binding architecture.

- [Design artifacts](design/README.md): vision, decisions, requirements,
  architecture, contracts, specifications and planned tests.
- [Developer guides](guides/overview.md): glossary, examples, walkthrough,
  testing and integration guidance. APIs are illustrative until implemented.
- [Test quality](guides/test-quality.md): executable coverage and mutation
  commands, report validation, CI scope and measurement limits.
- [Code/test quality review](implementation/test-quality-review.md): checked
  contracts and names, corrected gaps, actual measurements and remaining holds.
- [Local SDK integration](guides/sdk-integration.md): actual private Phase 7
  package entry points, retained memory, protected browser setup and limits;
  [current verification and open gates](implementation/phase-seven-platform-sdk.md).
- [Phase 3 reactive queries](implementation/phase-three-reactivity.md): shared
  incremental observation, count/column opt-ins and native/browser delivery evidence.
- [Phase 4 command sync](implementation/phase-four-command-sync.md): staged
  commands, durable recovery and actual Swift/optimized-browser execution evidence.
- [Phase 5 entity sync](implementation/phase-five-entity-sync.md): generated
  patches, field resolution, resumable bootstrap, explicit reset and final
  Rust/Swift/browser execution evidence.
- [Phase 0–3 review corrections](implementation/phase-three-review-resolution.md):
  reproduced query/recovery defects, regression evidence and three Claude reviews.
- [Phase 2 data model](implementation/phase-two-data-model.md): public Rust
  schema, queries, relations, migrations and native/browser persistence evidence.
- [Typed storage evidence](implementation/phase-one-storage.md) and
  [initial foundation](implementation/phase-one-foundation.md): partial
  storage foundation, native/browser restart checks and explicit remaining gates.
- [Prior-art research](references/README.md): imported observations, original
  source dates, alternatives and hidden progress trackers.
- [Documentation history](references/documentation-history.md): source coverage,
  historical versions and the evidence behind the consolidation.

The [contributor guide](../CONTRIBUTING.md) and [public artifact tools](../tools/traceability/README.md) define the independently runnable contribution workflow.
See the [repository layout and historical path map](guides/repository-layout.md).
[ADR-0029](design/decisions/ADR-0029-documentation-artifact-root.md) records this
project's documentation-layout deviation.
