# Using HoyaDB with an AI coding agent

This catalog and its [implementation skill](skills/hoyadb-integration/SKILL.md)
describe the expected procedure for **building an application with HoyaDB
installed from a remote package registry**. Work in the application's repository
using the installed SDK and its versioned documentation. A Hoya source checkout,
local tarball, source copy or library build is not part of the normal procedure.

The guide and skill travel with the client/testing npm package. Reading them
from `node_modules` after a registry install is ordinary dependency consumption,
not a `file:` dependency or a copy of the library's source into the application.
The present publication status is recorded separately at the end of this guide.

## Expected implementation procedure

1. **Choose the integration.** Inspect the app's package manager, lockfile and
   target platform. Establish whether it needs memory or persistent storage,
   local-only data or backend sync, and a new schema or an existing-data migration.
   Keep its current checkout and conventions.
2. **Install the released dependency.** Select a compatible published version
   and its required peers using the release documentation. Install it from the
   registry through the app's package manager and retain the lockfile. Use the
   native release distribution appropriate to the platform when needed.
3. **Load the implementation skill.** For an npm installation, ask the agent to
   read `node_modules/@hoyadb/client/skills/hoyadb-integration/SKILL.md` and the
   accompanying `AI.md`. The agent need not install a global plugin or clone
   Hoya. Automatic skill discovery is specific to the agent host.
4. **Implement an initial working feature.** Declare the app's schema, open the
   correct store, add CRUD and query/observation behavior, and wire cleanup into
   the app lifecycle. For persistent storage, implement retained key ownership.
   Add sync, server ID handling or migration only when the feature needs them.
5. **Verify in the consumer app.** Run its typecheck, bundler/build and relevant
   tests against the installed dependency. Exercise offline behavior and, for
   persistence, an actual cold restart. Report the selected SDK version and
   changes to app code and dependency locks.

## Find the matching API

| Application | Public entry / installed reference | Important boundary |
| --- | --- | --- |
| JavaScript or TypeScript application | `@hoyadb/client`; `web.d.ts`, `node.d.ts`, `application.d.ts` in the installed client | Builder schemas select the plain application API. Node defaults to memory; it is not persistent Node SQLite. |
| Explicit Node entry | `@hoyadb/client/node` | Uses the packaged WASM runtime. |
| Injected application runtime | `@hoyadb/client/application` | Portable plain rows, schema builders and application transport types. |
| Advanced tagged host integration | `@hoyadb/client/host`; installed `index.d.ts` | Tagged values and opaque transport differ from the application API. Do not mix their DTOs. |
| React Native | `@hoyadb/react-native`; its `index.d.ts`, `README.md`, `package.json` | Native factories plus the matching client peer. Check exact React/RN peers and native artifact compatibility. |
| Real-engine testing | `@hoyadb/testing`; its `index.d.ts` | Injects deterministic capabilities into the real core; it is not a second in-memory implementation. |
| Swift | `HoyaDB` / `HoyaDBTesting` products; resolved package's public interface and release docs | Use the actual Swift declarations; a JS method name does not establish a Swift convenience method. |
| Android/Kotlin | `dev.hoyadb` declarations in the resolved SDK / sources JAR and release docs | Inspect the resolved AAR, POM and included ABIs, not just a requested Gradle coordinate. |
| Rust / C | Matching crate API / `hoyadb.h` | Preserve typed Rust and ABI contracts; do not reconstruct them from JS examples. |

The installed `package.json` export map determines supported JS import paths.
Reading `AI.md` or declaration files through the filesystem does not make them
runtime import subpaths. Do not deep-import private `src/` files to bypass an
export mismatch. Check the package's module type and the consumer's actual
resolver conditions. Node, a browser and Metro are different consumers.

## Tools an agent can use

These are existing tools to discover in the working environment, not additional
Hoya dependencies. Use only the rows needed for the task.

| Tool | Use it for |
| --- | --- |
| Filesystem reader, `rg`, `rg --files` | Installed declarations, export maps, lockfiles, source, tests and the references below. |
| Git (`status`, `diff`, `show`) | Preserve application work and review the integration diff on the user's current branch. |
| App package manager (`npm`, pnpm, Yarn, SwiftPM, Gradle, Cargo) | Select compatible published dependencies, install them and retain the resolved versions. Use one package manager consistently. |
| npm (`view`, `ls`) or equivalent metadata tools | Inspect release metadata, peers and the actually installed dependency graph. |
| App's TypeScript/compiler, test runner and bundler | Prove public imports, schema types, runtime loading and application behavior with the installed dependency. |
| SwiftPM/Xcode or Gradle/CocoaPods tooling already used by the app | Inspect resolved native dependencies and build the requested platform. Use the application's lockfiles and documented commands. |
| Browser/devtools or an owned simulator/emulator | Check actual persistence, subscriptions and cold restart. Build success alone does not prove runtime behavior. |
| Versioned release documentation / package reference | Find API examples and compatibility instructions for the installed version. |
| Optional agent orchestration, such as Paseo | Coordinate work across repositories. Discover the available tool schema and destination; Hoya does not require an orchestration service. |

For Paseo, `list_hosts` discovers a destination `serverId`; keep it with its
`agentId` when addressing that agent. `list_workspaces` and `send_agent_prompt`
are coordination tools. Read the installed Paseo skill/tool descriptions if
using them. These IDs are not database IDs, backend URLs, npm registry settings,
or Hoya configuration. Do not copy a previous session's host/workspace IDs.

## Skills

- **Application implementation:** use [hoyadb-integration](skills/hoyadb-integration/SKILL.md).
  It covers remote dependency installation, schema and store setup, CRUD,
  observations, optional sync and app-level verification. The package delivers
  its references together; no Hoya contributor workflow is required.
- **Version matching:** use the skill accompanying the installed SDK, or its
  versioned release documentation. When updating the SDK, review its matching
  skill and migration notes as well. Do not use an unversioned newer skill to
  assume APIs exist in an older installed release.
- **Optional host skills:** use a platform or orchestration skill only when
  present and relevant. This repository does not install Paseo, editor plugins,
  a Hoya MCP server, or arbitrary third-party skills into the consumer.

## Install from the registry

The commands below are the expected npm procedure. Replace version placeholders
with compatible published versions identified from the release documentation;
they are not claims that a particular release is available today. For an existing
app, use its current package manager and dependency policy rather than adding
another lockfile.

Inspect the intended release and then install it:

```sh
npm view '@hoyadb/client@<client-version>' version repository dist.integrity peerDependencies --json
npm install --save-exact '@hoyadb/client@<client-version>'
```

For React Native, select the native package and a client version satisfying its
peer requirements; do not assume the two version numbers must always match:

```sh
npm view '@hoyadb/react-native@<rn-sdk-version>' peerDependencies --json
npm install --save-exact '@hoyadb/client@<client-version>' '@hoyadb/react-native@<rn-sdk-version>'
```

If the implementation needs the real-engine testing helpers, install the
compatible testing package as a development dependency:

```sh
npm install --save-dev --save-exact '@hoyadb/testing@<testing-version>'
```

Inspect the resulting manifest, lockfile and installed graph (`npm ls
@hoyadb/client @hoyadb/react-native @hoyadb/testing --depth=0` for those packages
the app uses). In npm projects, subsequent clean CI installs use `npm ci` with
the lockfile and the app's existing installation policy. The package manager
fetches the built SDK; app developers do not build Hoya's Rust/WASM binaries.
Native consumers still run their normal app autolinking, Pods/Gradle and build
steps, following that release's instructions.

Official command references: [npm view](https://docs.npmjs.com/cli/v11/commands/npm-view/),
[npm install](https://docs.npmjs.com/cli/v11/commands/npm-install/),
[npm ci](https://docs.npmjs.com/cli/v11/commands/npm-ci/) and
[Node package exports](https://nodejs.org/api/packages.html#package-entry-points).

Other platforms use their standard remote dependency flow. Obtain exact
coordinates from the release documentation; this guide does not invent them:

| Dependency | Evidence to inspect |
| --- | --- |
| SwiftPM | Add the official remote package URL with a compatible version requirement; retain `Package.resolved` when applicable. Use the distributed native artifacts. |
| CocoaPods / RN iOS | Consume the installed RN package's podspec and distributed native artifacts through the app's normal Pods setup; retain `Podfile.lock`. |
| Gradle / Android | Add the documented Maven repository and SDK coordinate/version; verify the resolved AAR and supported ABIs, retaining dependency locks where configured. |
| Cargo | Add the published crate/version and required features from its release documentation; follow the application's lockfile policy. |

If a selected release or native artifact is unavailable, report that specific
installation limitation. Do not silently substitute a source checkout, `file:`
dependency, local build or another package. A local-development workflow remains
an explicit alternative when requested. Writing or using this skill does not
publish a release or change the dependency setup of another application.

## IDs: resolve identity separately from server readiness

`resolveEntityId(table, id)` is the public JS application/host method. It resolves
a known alias through the core; it does not check row existence or promise that
the backend has accepted the record. An unchanged result can still be a pending
local create. Query equality predicates do not automatically rewrite saved IDs.
Resolve held navigation/upload references before constructing their queries.

`configureEntitiesDurable(scope, tables)` is an explicit fresh entity-store opt-in
to format 4. It retains aliases independently of bounded completion history.
Existing configured format-2/3 stores reject that opt-in unchanged; it is not an
automatic upgrade. Reset clears aliases and data while retaining format 4;
older binaries cannot open that format. Use the installed release's storage
compatibility and migration guidance before changing an existing store.

`server_id` is not a public Hoya SDK helper in these declarations. A name such as
`server_record_id` can be an application's policy that combines resolution,
pending recovery and its backend contract. Do not import or recreate that helper
as a universal Hoya API. Before sending an ID to an external upload/API, establish
the app's server-readiness condition; absence of a `local:` prefix is insufficient
(entity-generated IDs use a separate `entity:` namespace). Keep unresolved work
pending. Let core reconciliation apply mappings and rewrite declared references;
do not maintain a parallel JS ID ledger or mutate frozen queued envelopes.
Backends may accept identity-preserving mappings, including an entity-generated
ID. Prefixes alone are not a universal server-readiness policy.

## Integration rules to check in the actual API

- Choose the plain application surface or tagged host surface deliberately.
  Match transport/reconciliation types to it. Core owns querying, admission,
  retry deadlines and conflict semantics; adapters supply capabilities and
  application behavior.
- Protected persistence requires a retained, application-owned 32-byte key.
  Hoya does not provide an app Keychain/Keystore lifecycle. Missing/wrong keys
  must not become plaintext fallback or a silent fresh store. Memory reopen is
  not a process restart test.
- The application supplies authenticated transport, scope/context and business
  reconciliation. The SDK does not imply a `/sync` route, ULID server IDs or a
  particular backend. Preserve exact command/entity correlations on retries and
  handle partial replies without losing the remaining writes.
- Use the appropriate schema/reference types and preserve integer/time widths.
  For an atomic prepared parent/child operation, inspect host
  `entityBatchAliased` in the installed declarations and that release's prepared
  write documentation; do not preinsert a fake synced parent.
- Readonly frames are bounded (ABI1: 1 MiB); fetches may return `LimitExceeded`.
  Do not silently clip results or claim automatic paging from an app facade as
  SDK behavior. Use actual query/count APIs and test realistic payloads.
- Dispose observations and owners explicitly. Account changes, reset and close
  must retire stale async work. A resolved sync call alone is not an assertion
  that every recovery record is gone; inspect the public status for the app's
  completion condition.

## Sync and responsiveness review

Use the installed release's sync contract and these questions when reviewing an
integration. They are application responsibilities, not additional SDK methods.

- **Can offline work proceed during HTTP?** Avoid holding an app's writer/read
  gates across transport waits. If a facade assembles paged reads or mutable
  models, preserve that logical read against reconciliation and stale async
  materialization. An app-owned reservation around the public transport callback
  must reacquire before returning or throwing, and retire on account/owner changes.
  Simply unlocking the entire sync does not establish snapshot safety.
- **What does completion mean?** In the current JS application API, `sync()` is
  an explicit pass; a newly mapped parent can leave eligible children for a later
  pass. Await `queueStatus()` and inspect its `live` counter using the declared
  type. A manual continuation needs finite, observed progress and
  cancellation guards. Core owns retry deadlines; do not spin until empty or
  infer progress from pruned history. Outbound completion is not pull convergence.
- **Will a lost reply replay identically?** Persist the first outcome with the
  backend write. Do not rebuild it from a newer row or prune deduplication merely
  because a fixed number of days passed. Conflicting retained outcomes are
  protocol errors; current state belongs in pull.
- **Is the reply a confirmation or a correction?** Empty plan writes preserve
  mapping checks but do not guarantee convergence. Never drop every applied
  outcome's correction. Check submitted values, removals and authoritative clocks;
  retain genuine normalizations and test later same-row edits, clock skew and
  reopen. Typed entity corrections outside the recovery closure, including an
  unrelated parent update, remain restricted. Old failed receipts are a separate
  recovery decision, not permission to rewrite the outcome or clear the store.
- **Was the delay measured?** Separate handler/local-commit timing from visible
  frames and automation settling. Test the actual UI and process restart using
  the installed artifact. A fast handler, passing host test or completed build
  alone does not prove responsive, converged device behavior.

The matching release's SDK integration guide provides the detailed rationale
and source/contract references. For an explicit source investigation, its path
is listed below; a consumer need not clone or build Hoya to follow this checklist.

## Consumer references

Use these references in order; none requires cloning or building Hoya:

1. **Delivered implementation guidance:** this `AI.md` and the accompanying
   skill, read directly from the installed package.
2. **Installed API:** package exports and TypeScript declarations, Swift's public
   interface, Kotlin SDK/sources JAR, or the matching crate/header reference.
   These determine which APIs the consumer can call.
3. **Versioned release documentation:** installation, platform compatibility,
   examples, sync contract, migration notes and changelog linked by the official
   release or package metadata. Keep the selected version visible when browsing.

If published guidance is missing or conflicts with the installed API, identify
the gap and use release support or another compatible supported version. Source
inspection can help diagnose a library defect, but obtaining a source snapshot
and rebuilding Hoya is not a prerequisite for normal application implementation.

## Optional source reference map

For library contributors or an explicit source-level investigation, the following
paths are relative to a matching Hoya checkout. They are not required consumer
steps and do not resolve inside an installed npm package. Historical walkthroughs
and prior-art research can describe illustrative APIs rather than released ones.

| Task | Read in the matching source tree |
| --- | --- |
| Current SDK entry points and limits | `docs/guides/sdk-integration.md`; `packages/client/application.d.ts`; `packages/client/index.d.ts` |
| Runnable application examples | `examples/README.md`; `examples/application.mjs`; `examples/react-native-bootstrap.mjs` (check each example's recorded execution status) |
| Application vs tagged contracts | `docs/design/contracts/API-0012-javascript-application-binding.md`; `docs/design/contracts/API-0011-general-platform-sdk.md` |
| Durable IDs / prepared writes | `docs/design/spec/SPEC-0049-durable-entity-aliases.md`; `docs/design/spec/SPEC-0048-entity-batch-aliases.md` |
| Protected storage and keys | `docs/guides/security-integration.md`; `docs/design/spec/SPEC-0014-security-model.md` |
| React Native / native packaging | `packages/react-native/README.md`; `packages/react-native/tools/pack.py`; `packages/react-native/ACTIVATION.md` (repository's restricted verification workflow, not a universal app setup requirement) |
| Swift / Android / C | `bindings/swift/README.md`; `bindings/swift/Sources/HoyaDB/`; `bindings/android/README.md`; `bindings/android/src/main/kotlin/dev/hoyadb/`; `crates/hoyadb-native/include/hoyadb.h` |
| Local JS artifacts and independent consumers | `tools/javascript/README.md`; `tests/packaging/javascript/` |
| Current public docs and example verification | `docs/public/README.md`; `docs/public/pages/`; `docs/public/language-examples.json` |
| Design and current verification | `docs/design/README.md`; `CONTRIBUTING.md` |
| Contributing / compatibility / release gates | `CONTRIBUTING.md`; `tools/release/README.md`; `tools/traceability/README.md` |

## Repeatable verification

In an **application**, use its existing commands to check imports/types, offline
CRUD, observations, sync/retry and the specific behavior being changed. For
persistent storage, use task-owned data to check close/reopen and a fresh process,
including wrong-key failure. For ID changes, test held references before/after
reconciliation and after restart. Preserve real user stores.

Use the app's existing tests and the released `@hoyadb/testing` helpers when
relevant. Hoya's own contributor, traceability, packaging and binary-build gates
belong to library development; see `CONTRIBUTING.md` only when working on Hoya
itself. Passing consumer tests does not certify every platform or backend.

## Current distribution status

The procedure above describes the intended released-package experience. The
current repository still marks its npm packages private and its Rust crates
unpublished; registry releases and native remote coordinates are not established
by this document. No package was published while writing this guidance.

The JS builder already includes this guide and the skill in client/testing
archives. Existing older archives may not contain them, and publication of a
native skill bundle is not claimed. Local artifacts remain available for
explicit development/testing. Their shared private version number alone does
not identify a build; `SOURCE-MANIFEST.json` records pack-time hashes, not proof
that a binary was compiled from those sources.
