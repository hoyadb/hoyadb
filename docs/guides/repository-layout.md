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
unchanged when moving sources. Do not merge standalone target graphs merely to
make folder names uniform. The public layout does not enable publication or
choose a distribution license.

Accepted ADR bodies, the existing decision log, research imports and historical
implementation evidence are retained as recorded. Their old paths and private
process references describe their original source snapshot. Use this map for
current development; do not rewrite old hashes or evidence to imply a rerun.

| Historical path | Current source |
| --- | --- |
| `bindings/native` | `crates/hoyadb-native` |
| `bindings/web` | `crates/hoyadb-web` |
| `bindings/javascript/client` | `packages/client` |
| `bindings/javascript/testing` | `packages/testing` |
| `bindings/react-native` | `packages/react-native` |
| `bindings/javascript/scripts` | `tools/javascript/scripts` |
| `bindings/javascript/consumer` | `tests/packaging/javascript` |
| `bindings/javascript/third-party` | `licenses/javascript` |
| `docs/public/examples` | `examples` |
| `ai-governance/tools` | `tools/traceability` (checker/generator subset only) |

The history may still contain the former submodule URL and revision. Removing
the current gitlink is not a history rewrite or confidentiality audit.

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
