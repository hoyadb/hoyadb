---
name: hoyadb-integration
description: Implement HoyaDB features in an application using remotely installed SDK releases, including schema, storage, CRUD, observations, entity IDs and optional backend sync.
---

# HoyaDB integration

Implement the user's feature in the **application repository**, using Hoya as a
normal remote package dependency. Read the accompanying [consumer guide](../../AI.md)
for installation templates, public entry points and API constraints. This is the
expected procedure for released packages, even while publication is pending.
It does not require a Hoya source checkout, local tarballs or building the library.

## Install and select the API

1. Inspect the app's platform, package manager, lockfile and existing dependency
   versions. Preserve its current branch and conventions. Default to registry
   dependencies for a new integration; retain an explicitly requested local mode.
2. Select a compatible published SDK version using release docs and package
   metadata. Install with the app's normal package manager, recording resolved
   versions in its lockfile. For React Native, include the matching client peer
   and follow that release's app autolinking/Pods/Gradle instructions. Do not build
   Hoya native artifacts or install a speculative registry coordinate.
3. Read the installed public exports/declarations and versioned release docs.
   Use plain application APIs for ordinary JS/TS feature work; use the tagged
   host API only when that integration needs it. Select native factories for RN,
   and the language's own public SDK for Swift/Kotlin/Rust/C. Do not deep-import
   implementation files or borrow APIs from another database/binding.

If publication is not yet available, report that installation cannot be executed
yet and keep the expected remote procedure. Do not silently replace it with a
local copy. A missing feature or doc calls for release guidance/support or a
compatible SDK version, not an automatic request for a source snapshot.

## Implement the feature

- **Schema and store:** declare the app's tables, column types, optional fields
  and relations. In the JS application API, use `defineSchema`, `table`, `column`
  and `relation` as supported by the installed declarations. Select memory or
  protected persistence deliberately and open the store through the platform's
  public factory. Own its lifetime at the app's existing service/provider boundary.
- **Persistent data:** acquire and retain the app's 32-byte key through its
  secure storage integration. Treat wrong/missing keys as failures, not requests
  to erase data or fall back to plaintext. For an existing database, implement
  the supported migration without dropping pending work.
- **CRUD and UI updates:** implement the requested behavior using public table
  and query APIs. In the JS application API, use `db.table(name)` for record
  operations and `db.query(name)` for queries/observations. Map results into the
  app's existing state/view layer, handle errors and dispose observations when
  the owning view/service retires. Close the store at its actual lifecycle boundary.
- **Sync, only when needed:** choose the supported command or entity workflow,
  supply authenticated app transport and reconciliation, and retain offline
  writes across retries. Inspect the exact installed transport types; Hoya does
  not imply a particular backend endpoint or authentication scheme. Core owns
  queue/retry/mapping semantics; do not reimplement them in an app wrapper.
- **IDs, when relevant:** public `resolveEntityId(table, id)` resolves known
  entity mappings when supported. It does not establish server readiness or row
  existence. Apply the app's backend contract before uploads/external requests;
  keep unresolved work pending. `server_record_id` may be an app-specific helper;
  Paseo `serverId` routes agents. Neither is a universal Hoya method. Avoid a
  parallel alias ledger or modifications to frozen envelopes.

## Verify in the application

Run the app's typecheck, bundler/build and tests against the installed package.
Cover the feature's offline CRUD, observation updates and cleanup. For persistent
data, test cold restart with task-owned data; for sync, test failed/lost replies,
retry and ID reconciliation; for migration, test retained data and pending work.
Use compatible `@hoyadb/testing` helpers when useful. Do not require Hoya's own
source-build, packaging or contributor checks for a normal consumer integration.

Deliver the app-code and dependency-lock diff, installed SDK version, tests run
and remaining limitations. Separate type/host tests, native builds and actual
device behavior. This skill does not publish packages or change other apps.
