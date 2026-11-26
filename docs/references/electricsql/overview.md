---
source: https://electric.ax/docs/sync
source_type: doc-site
fetched: 2026-09-13
---

# ElectricSQL — Overview

Observed, not accepted — this describes Electric, not hoyadb.

## What it is

A **read-path-only sync engine for Postgres**: it syncs data *out of*
Postgres into local clients over HTTP, using a primitive called a **Shape**
(see [architecture.md](architecture.md)). It explicitly does not handle
writes back to Postgres — "delegating that responsibility elsewhere" (see
[sync.md](sync.md)).

## Transport

HTTP is the stated transport for the client-facing protocol (a documented
[HTTP API](https://electric.ax/docs/sync/api/http)). Clients exist for
TypeScript and Elixir, plus the protocol is open for custom
implementations. The exact live-update transport underneath the HTTP API
(long-polling vs. streaming vs. something else) was **not stated
explicitly on this page** — noted here as unconfirmed rather than guessed
at, pending the HTTP API unit.

## Relevance to hoyadb

The core architectural idea — **a sync engine that only owns reads,
leaving writes to whatever API the app already has** — is the most
structurally different split of responsibility found across this whole
reference set. Every other source (RxDB, WatermelonDB, Realm, Zero,
PowerSync) has the sync layer own both directions. Worth weighing
explicitly for hoyadb: a read-only sync layer is a smaller, more
self-contained piece to build and reason about, at the cost of leaving the
write/conflict/command story entirely to a separate mechanism hoyadb would
still have to design.
