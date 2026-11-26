# npm packages

| Directory | Package | Purpose |
| --- | --- | --- |
| [client](client/) | `@hoyadb/client` | Shared JavaScript/TypeScript API with web and Node entry points |
| [testing](testing/) | `@hoyadb/testing` | Test capabilities using the real database |
| [react-native](react-native/) | `@hoyadb/react-native` | Native transport and platform integration |

The root private npm workspace coordinates client/testing. React Native retains
its separate approved app/toolchain setup. All three packages remain unpublished;
their MIT/public metadata prepares them for release. [Build and packed-consumer commands](../tools/javascript/README.md)
assemble artifacts from these sources and the production WASM bridge. Third-party
notices are sourced from `licenses/javascript`, not copied from private tooling.

The [AI integration catalog](../AI.md) and [consumer skill](../skills/hoyadb-integration/SKILL.md)
are copied into built client/testing packages as `AI.md` and
`skills/hoyadb-integration/SKILL.md`. They can be read directly from installed
packages without a repository checkout or new runtime dependency. Source-package
directories must go through the documented builder to receive these assets.
The skill's consumer procedure starts with a registry installation and implements
features inside the app; the builder is a library-maintainer step, not a consumer
prerequisite. Remote publication remains a separate future action.
