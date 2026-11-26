---
source: https://zero.rocicorp.dev/docs/sync
source_type: doc-site
fetched: 2026-09-13
---

# Zero (Rocicorp) — Overview / "What is Sync?"

Observed, not accepted — this describes Zero, not hoyadb.

## What it is

A sync engine: keeps local (client-side) and server-side copies of changing
data consistent, so the app reads/writes *only* to the local copy — never
directly to the network — while the engine reconciles in the background.
Positioned as successor to Rocicorp's earlier Replicache (archived/sunset
June 2026, per web search this session).

## Problems it names with plain server-centric APIs

- **Latency**: every read/write round-trips to the server, adding "hundreds
  of milliseconds to each interaction."
- **Staleness**: clients have no reliable way to know when to refetch, so
  devices can silently diverge.
- **Connectivity dependency**: the app is fully broken offline.

## Its distinctive conflict-handling claim

Zero uses **"server reconciliation"**, described explicitly as "an elegant
and flexible technique pioneered by the video game industry" for handling
conflicts arising from multi-device writes. (The page doesn't spell out the
mechanics on this pass — see [mutators](sync.md#mutators-definition-and-dual-execution) /
[server-side execution](sync.md#server-side-execution-details) for how mutations actually execute
client-then-server, which is presumably what this technique refers to.)

## Stated differentiators vs. prior sync engines

The docs claim Zero addresses failures of earlier sync engines by
supporting: fine-grained authorization, partial sync, and custom business
logic on the read or write path — all "without requiring non-standard
backend databases" (i.e., works against an ordinary relational database
rather than requiring a special CRDT-native store).

## Relevance to hoyadb

"Server reconciliation ... pioneered by the video game industry" is a
named, citable prior-art lineage for optimistic-local-execute +
authoritative-server-re-execute, distinct from every conflict strategy seen
in RxDB/WatermelonDB/Realm (all of which resolve conflicts between two
already-written *states*, not by re-running an operation). Worth digging
into the game-industry client-prediction/reconciliation literature
separately if hoyadb pursues command-based sync, since Zero itself is
citing that as the origin of its technique rather than inventing it.
