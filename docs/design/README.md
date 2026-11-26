# Formal design artifacts

The [public contribution workflow](../../CONTRIBUTING.md) and
[artifact validation tools](../../tools/traceability/README.md) are self-contained.
Historical references to private process material describe their original context;
the [current path map](../guides/repository-layout.md) explains moved sources.

This directory is hoyadb's formal artifact root (ADR-0029).
[spec/registry.yaml](spec/registry.yaml) allocates stable IDs; its file paths
are relative to this directory. The generated
[traceability matrix](traceability/rtm.md) connects requirements and tests.

## Reading path

1. [Vision](vision/VIS-0001-hoyadb.md) and [objectives](objectives/).
2. [System architecture](architecture/ARC-0001-system.md) and
   [platform boundary](architecture/ARC-0002-platform-bindings.md).
3. [Decisions](decisions/) and [the root decision log](../../DECISIONS.md).
4. [Requirements](requirements/), [data models](data/),
   [specifications](spec/) and [contracts](contracts/).
5. [Developer flow](ux/flows/UX-0001-developer-integration-flow.md),
   [design system](ux/README.md),
   [test plan](tests/plan/TP-0001-test-plan.md) and [test cases](tests/cases/).
6. [Risks](risks/) and [security threats](security/threats/).

These artifacts describe intended behavior. An unpublished storage subset and
platform experiments now exist; see [implementation evidence](../implementation/phase-one-foundation.md).
They do not implement the complete database.
Examples in [supporting guides](../guides/overview.md) illustrate the design;
they do not settle open implementation choices.

## Historical citations

Old source path strings inside artifacts identify their provenance. In
particular, accepted ADR bodies are preserved unchanged. They refer to the
pinned baseline snapshot, not live files in this checkout. The
[historical source resolver](../references/documentation-history.md#historical-citations) links every
original source to its exact Git revision; the
[source ledger](../references/documentation-sources.csv) records current destinations.

## Verification

Run from the repository root:

```sh
python3 tools/traceability/trace_check.py docs/design
python3 tools/traceability/rtm_gen.py docs/design --check
```

To regenerate the matrix after artifact-link changes, run the second command
without `--check`. Research metadata and governance fixtures stay outside this
scan root.
