# Repository layout and public contributor boundary

## Implementation

[Cargo.toml](../../Cargo.toml) coordinates the core, storage adapters and shared
facade. Native and web bridges live under `crates` but remain standalone/excluded
with their own lockfiles and target requirements. [package.json](../../package.json)
coordinates the client/testing npm packages; React Native is a separate package
under `packages`. Swift and Android SDKs remain under `bindings`.

Runnable tutorials live under [examples](../../examples/README.md); public HTML
documentation includes those same sources. Shared JavaScript builders live under
`tools/javascript/scripts`, ordinary consumer fixtures under
`tests/packaging/javascript`, and the exact notice corpus under
`licenses/javascript`. Builds write ignored artifacts; checked-in source does not
include compiled distribution binaries.

The public [contribution contract](../../CONTRIBUTING.md),
[traceability tools](../../tools/traceability/README.md) and CI are usable without
a private framework checkout. Maintainers may keep private tools in a separate
workspace, or retain an ignored local `ai-governance` directory. Local agent
skills under `.agents` and `.claude/skills` are likewise maintainer-only. No
private files are needed for the public checks.

## Rules

Keep package names, public interfaces, dependency versions and database behavior
stable when moving sources. Standalone target graphs retain their own lockfiles.
First-party code uses [MIT](../../LICENSE); third-party notices keep their own terms.

Current API baselines live under `tools/release/baseline`. Native notice validation
inputs live under `licenses/native`, and the native dependency inventory is
`licenses/native-dependencies.json`. These maintained inputs are separate from
fresh build/runtime results. Never change upstream license bytes or present a
stored input as a new verification run.


## Required checks

Run the commands in [CONTRIBUTING.md](../../CONTRIBUTING.md) and the affected
SDK guide. Run `python3 -m unittest discover -s tests/packaging -p 'test_*.py'`
to execute the public tools in an isolated directory without private files, and
`npm run test:packaging` after building artifacts to exercise installed packages.
Review source moves against the previous paths and retain third-party bytes.

## Update this page when

Update this map and incoming source links whenever package roots, build outputs,
workspace membership or the public/private tooling boundary changes. Keep
[SPEC-0045](../design/spec/SPEC-0045-public-repository-layout.md) and its acceptance
cases aligned with any changes to this transition's scope.
