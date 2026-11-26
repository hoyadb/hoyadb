---
source: https://electric.ax/docs/sync/guides/writes
source_type: doc-site
fetched: 2026-09-13
---

# ElectricSQL — Write patterns (observed)

Observed, not accepted — this describes patterns Electric's docs recommend
for handling writes, not hoyadb's design. Since Electric itself only
syncs reads, this page is really "how to build the write half yourself" —
directly relevant to hoyadb regardless of which sync architecture it picks,
because it names a **progression** of increasingly local-first write
patterns rather than a single fixed design.

## The four named patterns, in increasing sophistication

1. **Online writes** — plain REST calls straight to the backend, which
   writes Postgres, which Electric then syncs back down. Simplest;
   no offline write support at all, full network-latency exposure on
   every write.
2. **Optimistic state** — same backend call, but the UI shows the pending
   change immediately ("state that you display 'optimistically' whilst
   waiting for an async operation ... to complete") and discards it once
   Electric's replication stream syncs the confirmed, real change back.
   Component-scoped (e.g. React's `useOptimistic`) — doesn't survive a
   reload and isn't shared across components.
3. **Shared persistent optimistic state** — the same idea, but the pending
   local write lives in a shared, persisted store (the docs' example uses
   Valtio + localStorage) instead of component state, so it survives
   reloads and is visible everywhere in the app. Reconciliation is
   explicit and stream-driven: "Subscribe to the shape stream until the
   local write syncs back through it. At which point, delete the local
   write from the optimistic state" — i.e. the optimistic write is
   considered resolved the moment the *same* change shows up in the
   authoritative Shape stream, not via any separate acknowledgment
   channel.
4. **Through-the-database sync** — the most complete pattern: the app
   writes directly to a local embedded database (PGlite), and database
   **triggers** capture those writes into a changelog table that a
   background process syncs to the server. This is the only one of the
   four patterns that works fully offline for writes, not just reads — at
   the cost of real added machinery (shadow/changelog tables, trigger
   logic) that the other three patterns avoid entirely.

## Reconciliation principle common to all four

Every pattern keeps **synced (authoritative) state and local optimistic
writes as two separate things**, merged only at render/read time — never
mutating the synced copy directly with a local guess. Confirmation that a
local write "landed" comes from seeing the equivalent change arrive back
through the normal sync stream, not from a distinct write-acknowledgment
protocol.

## Relevance to hoyadb

This is a genuinely different framing from every other source in this set:
instead of one prescribed sync protocol, Electric's docs present writes as
a **spectrum apps choose a point on**, from "no offline writes at all" to
"fully offline via local DB + trigger-captured changelog" — explicitly
naming the complexity/capability trade-off at each step rather than picking
one for you. The "resolve optimistic state by watching for it to reappear
in the authoritative stream, don't invent a separate ack channel" principle
is a clean, minimal reconciliation idea worth weighing against Zero's
explicit two-stage (`.client`/`.server`) confirmation and WatermelonDB's
explicit push-response-based confirmation — three different answers to
"how does the client know its optimistic write is truly durable." The
through-the-database pattern's trigger-captured changelog is also a fourth
distinct implementation of "turn local writes into a syncable log,"
alongside WatermelonDB's `_status`/`_changed` columns, PowerSync's
SQL-statement-derived upload queue, and Zero's mutator re-execution.
