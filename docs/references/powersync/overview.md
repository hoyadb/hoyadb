---
source: https://docs.powersync.com/architecture/architecture-overview.md
source_type: doc-site
fetched: 2026-09-13
---

# PowerSync — Overview / Architecture

Observed, not accepted — this describes PowerSync, not hoyadb.

**Note on source quality**: this page is largely a diagram with limited
surrounding prose; the fetch could describe its existence but not its
visual content in detail. Recorded here is only what was actually
stated in text, not a reconstruction of the diagram.

## What it is

PowerSync keeps a client-side SQLite database in sync with a backend
source database (Postgres, MySQL, MongoDB, SQL Server, or Convex, per
its supported-backends list), bidirectionally.

## Three named components

1. **The PowerSync Service** — a centralized sync server that orchestrates
   synchronization.
2. **Client SDKs** — per-platform/language libraries managing the local
   SQLite database and client-side sync state (JS/Web, React Native/Expo,
   Node.js, Capacitor, Tauri, Dart/Flutter, Kotlin, Swift, .NET, Rust).
3. **Backend integration layer** — bridges the source database and the
   PowerSync Service.

The page states there is a dedicated sync protocol governing
client↔service communication, and separate documentation sections (not
fetched in this pass) on the Service's internals, client-side
implementation, protocol specifics, and consistency guarantees.

## Relevance to hoyadb

Confirms PowerSync's shape is: one shared backend-facing sync service
(not embedded logic each client implements independently, unlike RxDB/
WatermelonDB where the "server" side is just whatever pull/push endpoints
the app author writes) sitting in front of an existing relational/document
database, with per-platform local SQLite on the client. Worth deciding
whether hoyadb wants a similar "always a service in the middle" shape, or
the RxDB/WatermelonDB/Zero style where the sync logic is a library/contract
the backend implements directly with no separate service required.
