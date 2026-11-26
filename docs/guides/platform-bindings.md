# Platform bindings

> For the implemented JavaScript application API and executed examples, build the [public HTML documentation](../public/README.md) and start with its Getting started page. Illustrative syntax below describes design intent; consult the SDK contracts for concrete APIs.

> Supporting guide. Current design authority: [Platform bindings architecture](../design/architecture/ARC-0002-platform-bindings.md).
> Code and package names are illustrative design examples; no SDK implementation
> is supplied by this guide. Current API contracts define the supported SDK surface.
> For current platform SDK package usage and limits, see
> [Local SDK integration](sdk-integration.md) and its runtime gates.

The Rust-core → WASM-for-web / native-bindings-for-mobile-desktop split:
what crosses the boundary, what stays inside the core on every platform,
and the resulting language-facing surface per platform.

## Non-goals

Carried forward from [VISION.md](../design/vision/VIS-0001-hoyadb.md)'s "What we deliberately
avoid", applicable to this doc:

- **A separate implementation per platform.** Every binding below wraps
  the same core; none reimplement its logic.

## What stays inside the core on every platform

The entire [data model](../design/spec/SPEC-0003-schema-and-relations.md), the
[storage interface](glossary.md#storage-interface) (specified in
[04-storage/storage-interface.md](../design/spec/SPEC-0001-storage-interface.md)) and
its concrete engine dispatch, [reactivity](../design/spec/SPEC-0006-subscriptions.md)'s
incremental query diffing, and the [sync](../design/architecture/ARC-0001-system.md) state
machine ([offline queue](glossary.md#offline-queue), idempotency
tracking, id-mapping for dependent writes) all live inside the Rust core,
unchanged by which platform is hosting it. A platform binding never
reimplements any of this — it only
translates calls in and events out.

One Rust core owns shared database semantics (`ARC-0001`). Platform bindings adapt ownership, scheduling and value conversion. FFI/WASM overhead and lifecycle correctness require validation on each supported target (`API-0011`).

## What crosses the boundary

Only what a platform binding needs to translate application calls into
core operations and core events back into application-visible callbacks:

- **In**: schema declarations, [command](glossary.md#command)/[entity-diff](glossary.md#entity-diff-fallback-path) write calls, query/
  [subscription](glossary.md#subscription) requests, configuration
  ([trusted context](glossary.md#trusted-context) extension point,
  encryption key material, the
  [`transport` implementation](sync-implementation.md#client-side-implementing-a-real-transport)
  the core calls into for sync).
- **Out**: query/subscription results and their change notifications,
  command outcomes (the `CommandOutcome` enum from
  [06-sync/command-protocol.md](../design/spec/SPEC-0008-command-protocol.md)), and
  offline-queue status.

No domain logic, no sync state, and no storage access cross this boundary
in the other direction — a binding is a thin translation layer, not a
second place any of this logic could diverge.

### Web (WASM)

A TypeScript consumer imports hoyadb's package and gets the API surface
used throughout this doc tree (`defineSchema`, `db.query(...).observe(...)`
— see [query-model.md](../design/spec/SPEC-0004-query-model.md) for what a query
supports — `defineCommand`, etc.) — a JS/TS-native binding over the Rust
core compiled to WebAssembly:

```typescript
import { openDatabase, defineSchema } from "@hoyadb/client";

const schema = defineSchema({ /* ... */ });
const db = await openDatabase({
  schema,
  filename: "app.hoyadb",
  transport: createHttpTransport(/* ... */), // see implementation-guide.md
});
```

Under the hood, the WASM module runs the core; the `@hoyadb/client`
package is the thin JS-facing wrapper translating typed JS calls into
core operations and core change-events into JS Promises/callbacks.

### Mobile (React Native, native iOS/Android)

A React Native consumer sees the same `@hoyadb/client`-shaped API as web
(same schema/query/command surface), backed by a native binding rather
than WASM — necessary because WASM is not the natural runtime target on
mobile.

A native Swift or Kotlin consumer (for an app not going through React
Native) sees an idiomatic per-language wrapper over the same core, e.g.:

```swift
let db = try await HoyaDB.open(schema: schema, filename: "app.hoyadb")
let pendingOrders = try await db.query("orders")
    .where("status", .equals, "pending")
    .fetch()
```

```kotlin
val db = HoyaDB.open(schema, filename = "app.hoyadb")
val pendingOrders = db.query("orders")
    .where("status", Op.EQUALS, "pending")
    .fetch()
```

Every language-facing wrapper exposes the same underlying concepts (schema,
query/subscription, command, offline-queue status) with the same
semantics defined throughout this doc tree — only naming conventions and
async idioms differ per language.

### Desktop (Electron/Tauri-style shells)

Follows the web (WASM) or native binding path depending on the shell's own
runtime — an Electron-style shell (Node + Chromium) uses the same
TypeScript surface as web; a Tauri-style shell (native host process) uses
a native binding the same way mobile does.

## React Native's binding — flagged, unconfirmed

React Native uses the maintained SDK surface described in `API-0011` and the package activation guide. A binding candidate is not a supported runtime until its build and runtime gates pass.

## Reactivity across the core/platform boundary

Live subscription notifications cross the Rust/platform boundary through the ownership and scheduling contract in `API-0011`. Platform dispatch, cancellation and callback lifetime require target-specific verification; a host test does not certify a mobile runtime.

The closely related question of *when* a
[command handler](glossary.md#command-handler) (specified in
[06-sync/command-protocol.md](../design/spec/SPEC-0008-command-protocol.md#1-shape-of-a-command))
— developer's own code, not core logic — gets called back into from inside
the core is now partially settled by
[command-protocol.md](../design/spec/SPEC-0008-command-protocol.md#3-handler-execution-model)'s
ADR-0025: the core calls into a handler at most once at a time (serialized
across commands), and the handler may itself call back into the core
asynchronously for reads/writes before the core commits its result. The
concrete FFI/WASM mechanics of that callback (how an async callback across
the language boundary is actually implemented per platform) are still
unresolved — ADR-0025 fixes the ordering/atomicity contract, not the binding
mechanism that has to implement it.

## What's still open

The concrete FFI mechanism per native platform (raw C ABI, UniFFI, or
another binding-generation tool) is implementation-time detail, and the
React Native binding choice above is unconfirmed — this doc fixes the
layering and per-platform surface shape, not the literal binding
toolchain.

**Concurrent access to one local store from the same platform** — two
browser tabs, or two desktop windows, opening the same underlying
file/OPFS store at once — is unaddressed anywhere in this tree: no file
locking, no leader election for which instance drives sync, no
cross-tab/cross-window reactivity propagation. This is a routine
scenario on exactly the platforms this doc targets (web, Electron/Tauri
desktop), not an edge case — a real, disclosed gap.
