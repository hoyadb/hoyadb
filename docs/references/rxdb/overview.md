---
source: https://rxdb.info/overview.html
source_type: doc-site
fetched: 2026-09-13
---

# RxDB — Overview

Observed, not accepted — this describes RxDB, not hoyadb.

## What it is

RxDB (Reactive Database) is a JavaScript, offline-first, document/schema
database with reactive queries and built-in replication/sync.

## Core layers/entities observed

- **Storage layer** — pluggable via an `RxStorage` interface. Backends
  observed in the nav: IndexedDB (browser, Capacitor), SQLite (Capacitor,
  React Native, Expo, Tauri, Electron, Node.js), MongoDB, Deno KV, and
  others. The storage backend is swappable independently of the rest of the
  stack.
- **Query layer** — `RxQuery`, with a query cache and query optimizer
  mentioned in the nav (details deferred to `rx-query.html` unit).
- **Replication layer** — a core replication protocol (`replication.html`)
  with multiple transport-specific plugins built on top (HTTP, WebSocket,
  CouchDB, WebRTC P2P, GraphQL, Firestore, MongoDB, etc.) plus a "Partial
  Sync" feature for syncing a subset of data.
- **Core entities**: `RxDatabase`, `RxSchema`, `RxCollection`, `RxDocument`.

## Platforms targeted

Browser (localStorage, IndexedDB, OPFS), React Native, Expo, Node.js,
Capacitor, Electron, Tauri, Deno — i.e. one JS-based core reused across
browser, mobile (via RN/Capacitor), and desktop (via Electron/Tauri) shells.

## Data model

Schema-based, document-oriented (not relational), with a "Population"
feature and "Relations" support layered on top to link documents —
i.e. relations are an addition on top of a document model, not a native
relational engine.

## Relevance to hoyadb's open questions

- Storage pluggability: RxDB's `RxStorage` interface — one core, swappable
  backend per platform — is a direct precedent for hoyadb's "storage engine
  may change across environment" need.
- Data model: RxDB is document-oriented with bolted-on relations, which
  contrasts with hoyadb's stated preference for "relational-ish" — worth
  flagging as a difference to reconcile, not a pattern to copy directly.
