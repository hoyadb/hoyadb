---
id: VIS-0001
title: hoyadb — an offline-first, relational, reactive embedded database
type: vision
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-10-20
parents: []
tags: [product, vision, offline-first]
---

# VIS-0001: hoyadb — an offline-first, relational, reactive embedded database

This vision describes the intended product and its scope. Current implementation and release status are recorded in the package guides and release gates.

## Press Release

**hoyadb: apps that keep working when the network doesn't**

hoyadb is an embedded database that lets an application — native mobile,
React Native, web, or desktop — read and write its data locally first and
stay fully usable with no network connection at all. An app declares a
schema with real relationships between tables, subscribes to query results
that update live as local data changes, and writes through named
domain-level actions rather than raw record edits. When connectivity comes
back, a sync layer reconciles local state with the application's own
backend.

"Offline-first usually means one of two compromises," said the project
owner. "Either you sync reads and leave writes to the app, or you sync raw
row diffs and lose the reason the change happened. hoyadb's primary write
unit is a named command carrying its arguments and a trusted identity —
so the backend can re-run the *intent* authoritatively instead of replaying
somebody's guess at the resulting rows."

One implementation, written once in Rust, is shared across every target:
compiled to WebAssembly for the browser and exposed through native bindings
on mobile and desktop. Persistence sits behind a single narrow storage
interface, so the concrete engine underneath can differ per platform — a
SQLite-family engine by default — without changing anything above it.

hoyadb is a library, not a hosted service. It never opens a network
connection itself, never implements authentication, and sends no telemetry
anywhere. It hands the integrating developer a payload and a trigger; the
developer's own code makes the call, over whatever transport and
authentication their backend already uses.

## FAQ

**Q: Who is this for?**
A: Developers building mobile, React Native, web or desktop applications that must remain usable offline and reconcile with a backend.

**Q: What does a developer actually do?**
A: Declare a schema (`REQ-0001`, `REQ-0002`), read or subscribe to queries
against it (`REQ-0007`, `REQ-0008`), and write either through a named
command (`REQ-0010`) or, for tables with no domain logic, through the plain
entity-diff fallback (`REQ-0023`). `UX-0001` walks that whole flow
end to end.

**Q: Why commands instead of plain row diffs?**
A: Value diffing alone breaks down for anything needing merge semantics
beyond "last write wins" — counters and ordered lists most clearly. A
command sidesteps this because the backend re-runs the action against real
server-side state rather than merging two versions of a row. The plain
diff path still exists as a fallback for tables that do not need it
(`ADR-0005`).

**Q: What happens when the backend disagrees with the local result?**
A: The backend's outcome is authoritative. It can confirm, correct, or
reject the optimistic local apply. hoyadb surfaces that through the same
subscription mechanism used for ordinary data changes and exposes a
rollback path (`REQ-0017`); deciding what the app does next is the
developer's own responsibility.

**Q: Does hoyadb guarantee a command produces the same result on both
sides?**
A: No, and this is deliberate. The command handler is the developer's own
code on both the client and the backend; hoyadb owns the infrastructure
around it, not its logic. This is recorded as an explicit non-goal in
`ADR-0014` and tracked as a live project risk in `RISK-0002` — not as a
solved problem.

**Q: How fast is it?**
A: There is no universal numeric latency guarantee. Performance claims require a measured workload, engine and target (`OBJ-0004`, `ADR-0022`).

**Q: What is explicitly not in scope?**
A: Authentication, network calls, transport security and client/server command-handler compatibility belong to the integration. HoyaDB targets authoritative-backend sync and application-owned schemas; it does not operate a hosted backend.

**Does source exist?** The repository contains the core, storage adapters and
platform SDK sources. Their contracts and acceptance gates remain distinct from
the presence of code. Use current checks before claiming target support.

## Traceability

- Parents: none — this is the top of the chain.
- Made measurable by: `OBJ-0001` … `OBJ-0006`.
