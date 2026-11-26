# hoyadb — Overview

This page introduces hoyadb and maps the current documentation. The
[formal design](../design/vision/VIS-0001-hoyadb.md), indexed by the
[artifact registry](../design/spec/registry.yaml), is the authority for
requirements, architecture, decisions and contracts. These supporting
guides explain that design through vocabulary and examples.

## What hoyadb is

hoyadb is designed as an offline-first, relational, reactive embedded
database. An application declares a schema with real relationships,
reads and writes through a local storage layer that works without a
network connection, and subscribes to query results that update as local
data changes. When connectivity is available, sync reconciles local state
with a backend. Most writes are named [commands](glossary.md#command) with
arguments, applied immediately on-device and then confirmed or corrected
by the backend's authoritative execution.

See the [product vision](../design/vision/VIS-0001-hoyadb.md) and
[README](../../README.md) for the goal and comparison to Realm, RxDB and
WatermelonDB. Current decisions and their alternatives live in
[architecture decision records](../design/decisions/); source research is
retained under [references](../references/).

## How to read the documentation

Start with the [glossary](glossary.md), then the
[walkthrough](walkthrough.md), which threads one order through the system.
Return to the glossary after the example if a term needs more context.
The [developer integration flow](../design/ux/flows/UX-0001-developer-integration-flow.md)
also describes the integration's success, error and undesigned states.

For a topic-by-topic reading path:

- **Current local SDK usage:** [integration instructions](sdk-integration.md)
  use the implemented private package exports; the linked Phase 7 evidence
  distinguishes verified behavior from pending platform and release gates.
- **Architecture and platform bindings:** the
  [system architecture](../design/architecture/ARC-0001-system.md),
  [binding architecture](../design/architecture/ARC-0002-platform-bindings.md)
  and [platform examples](platform-bindings.md) explain one Rust core,
  WASM on web and native bindings on mobile/desktop.
- **Data model:** [schema and relations](../design/spec/SPEC-0003-schema-and-relations.md),
  [relation examples](relations.md), [query model](../design/spec/SPEC-0004-query-model.md)
  and [schema migrations](../design/spec/SPEC-0005-schema-migrations.md).
- **Storage:** the [storage contract](../design/spec/SPEC-0001-storage-interface.md)
  and [SQLite-family default engine](../design/spec/SPEC-0002-default-engine.md).
- **Reactivity:** [subscriptions](../design/spec/SPEC-0006-subscriptions.md)
  and [performance guards](../design/spec/SPEC-0007-performance-guards.md).
- **Sync:** [commands](../design/spec/SPEC-0008-command-protocol.md),
  [offline queue](../design/spec/SPEC-0009-offline-queue.md),
  [entity-diff fallback](../design/spec/SPEC-0010-entity-diff-fallback.md),
  [conflict resolution](../design/spec/SPEC-0011-conflict-resolution.md),
  [membership scoping/topology](../design/spec/SPEC-0012-scoping-topology.md),
  [transport integration](../design/spec/SPEC-0013-transport-integration.md),
  [reference HTTP contract](../design/contracts/API-0001-http-sync-transport.md)
  and the worked [sync implementation guide](sync-implementation.md).
- **Security:** the [security model](../design/spec/SPEC-0014-security-model.md)
  describes engine-native at-rest encryption and the integrating developer's
  authentication and transport responsibilities.
- **Non-functional concerns:** [queue observability](../design/spec/SPEC-0015-queue-observability.md)
  covers per-command status, bounded retention and retry backoff;
  [NFRs](../design/requirements/nfr/) cover responsiveness, offline use,
  queued-write durability, footprint and compatibility. Open sections in the
  specifications retain unresolved details.
- **Testing:** the [testing guide](testing.md),
  [testing requirement](../design/requirements/REQ-0034-testing-support.md)
  and [test plan](../design/tests/plan/TP-0001-test-plan.md).

## What this is not

These documents specify intended behavior; they are not an SDK
implementation. Package names, API examples and backend pseudocode are
illustrative. The [roadmap](../../ROADMAP.md) starts with validating the
Rust/WASM/native-binding shape through a minimal spike; see also
[ADR-0008](../design/decisions/ADR-0008-implementation-roadmap-validation-spike.md).
The binding mechanism, callback propagation and several sync edge cases
remain open in the corresponding formal artifacts.

## Non-goals

The design deliberately avoids:

- A separate implementation per platform.
- A single fixed, non-swappable storage engine.
- A fully schemaless model.
- Sync that only covers reads.
- A peer-to-peer / multi-master topology with no authoritative side.
- hoyadb implementing authentication itself or mandating a specific scheme.
- hoyadb making network calls on the integrating app's behalf.

See the [vision](../design/vision/VIS-0001-hoyadb.md) and each topic's formal
ADRs/specifications for the full scope and rationale.
