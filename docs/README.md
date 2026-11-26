# HoyaDB documentation

- [Runnable tutorials](../examples/README.md): public application examples.
- [SDK integration](guides/sdk-integration.md): package entry points and limits.
- [Developer guides](guides/overview.md): schema, queries, relations, sync and testing.
- [Technical contracts](design/contracts/): interface and behavior requirements.
- [Architecture and design](design/README.md): decisions, specifications and tests.
- [Repository layout](guides/repository-layout.md): package and tooling boundaries.
- [GitHub delivery](guides/github-delivery.md): CI, registries, native runners and Pages.
- [Test quality](guides/test-quality.md): coverage and mutation gates.
- [AI integration catalog](../AI.md): packaged guidance for application development.
- [Public HTML documentation](public/README.md): building and verifying the site.
- [Contributing](../CONTRIBUTING.md): setup, checks and review requirements.

Current technical contracts are maintained under `docs/design`. Validation checks
structure, traceability and exercised behavior; it does not establish untested
platform support. API compatibility baselines live under `tools/release/baseline`,
and required native notice inputs live under `licenses/native`.
