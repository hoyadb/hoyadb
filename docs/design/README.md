# Formal design artifacts

This directory contains the current technical contracts and their traceability.
[spec/registry.yaml](spec/registry.yaml) allocates stable IDs relative to this
directory. The [traceability matrix](traceability/rtm.md) connects requirements
and tests. Contributor checks use the public [tools](../../tools/traceability/README.md).

## Reading path

1. [Vision](vision/VIS-0001-hoyadb.md) and [objectives](objectives/).
2. [System architecture](architecture/ARC-0001-system.md) and
   [platform boundary](architecture/ARC-0002-platform-bindings.md).
3. [Decisions](decisions/) and the [decision index](../../DECISIONS.md).
4. [Requirements](requirements/), [data models](data/),
   [specifications](spec/) and [contracts](contracts/).
5. [Developer flow](ux/flows/UX-0001-developer-integration-flow.md),
   [design system](ux/README.md), [test plan](tests/plan/TP-0001-test-plan.md)
   and [test cases](tests/cases/).
6. [Risks](risks/) and [security threats](security/threats/).

Contracts describe intended behavior. Use [current guides](../guides/overview.md)
and the [contribution commands](../../CONTRIBUTING.md) when working with source.
Do not infer native runtime support from host or source-only checks.

## Verification

```sh
python3 tools/traceability/trace_check.py docs/design
python3 tools/traceability/rtm_gen.py docs/design --check
```

Regenerate the matrix without `--check` after artifact-link changes. Invalid
tooling fixtures remain outside this scan root.
