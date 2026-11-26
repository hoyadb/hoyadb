---
id: ADR-0032
title: Host-driven command scopes and durable guarded recovery
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-10-23
updated: 2026-10-23
parents: [REQ-0011, REQ-0012, REQ-0017, REQ-0018, REQ-0019, REQ-0021, REQ-0043]
depends_on: [RFD-0003, ADR-0025, ADR-0016, ADR-0031]
tags: [command-sync, commands, recovery]
---

# ADR-0032: Command sync recovery

## Context and problem statement

The operator explicitly approved RFD-0003's reviewed P1–P7 policies after
requesting full command sync. Earlier decisions require async global serialization,
atomic optimistic enqueue, intent transport and explicit rollback, but leave
scope lifetime, dependency graphs, receipt/outcome recovery and retention open.

## Considered options

- Host-driven scoped handlers with one shared coordinator, staged local views,
  durable recovery records and conflict-checked reconciliation.
- Holding storage transactions across foreign asynchronous callbacks.
- Guessing references from strings and restoring undo images unconditionally.
- Treating receipt as final success or discarding live work at history eviction.

## Decision outcome

In the context of portable command sync, facing asynchronous host lifetimes and
incomplete recovery policies, we decided for **RFD-0003 P1–P7: shared FIFO
serialization with explicit abort, bounded dependency-free JSON, durable explicit
dependency graphs and separate receipt/outcome state, application-defined guarded
reconciliation, queue-aware lifecycle rejection and bounded host-driven retry**,
and against storage locks over host awaits, guessed references, unconditional
before-image restoration and discarding unresolved recovery work, to achieve
atomic local intent and recoverable cross-language execution, accepting global
serialization, staging costs, explicit conflicts and manual resolution of stuck
or ambiguous work.

## Consequences

The supported native process shares one coordinator; browser callers share one
authoritative worker. Separate tabs/processes concurrently accessing one store
remain unsupported. Explicit abort/close handles abandonment; there is no implicit
watchdog or ability to stop an infinite host loop. All public mutation paths
under the command owner respect the same isolation boundary.

Structured arguments/context and stable keys are captured without networking.
Per-store random namespaces distinguish different stores; retransmission freezes
exact outgoing bytes. Explicit path declarations and durable maps handle DAGs,
with a bounded recent-map cache after live dependencies finish. Receipt stops
network retry while a separate record retains later-outcome recovery.

Arbitrary backend results require an application reconciliation plan. Rollback
and correction guard against independent later edits instead of overwriting
them. Unresolved recovery and sent-cancel tombstones retain capacity and prevent
unsafe reset/migration. Existing queue-free lifecycle behavior and SQLite's
private physical format remain intact; old readers may reject new metadata.

No runtime dependency, business-handler replay protocol, automatic authentication,
library networking/telemetry, production store migration or release is approved
by this decision. Encryption, pull/entity diffs and packaged SDK breadth retain
their later roadmap phases.

## Confirmation

SPEC-0029/TC-0032 define five implementation slices and twenty acceptance cases.
Reviewed exact API/storage contracts precede each behavioral RED/GREEN cycle.
Completion requires actual memory/native/browser behavior, suspended host
handlers, independent restart/crash boundaries, guarded rollback and host HTTP
evidence; approval of this ADR is not evidence of implemented behavior.
