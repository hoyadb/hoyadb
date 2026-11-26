# Glossary

> Supporting guide. Current design authority: [Product vision](../design/vision/VIS-0001-hoyadb.md).
> Code and package names are illustrative design examples; no SDK implementation
> is supplied by this guide. Current API contracts define the supported SDK surface.

hoyadb's own vocabulary. The supporting guides use these terms as
defined here and link back to this page on first use. Formal specifications
define the authoritative contracts for each concept.

Terms are listed alphabetically. Each entry names the doc where the concept
is specified in full — this page defines the term, it doesn't replace the
doc that designs it.

Read this before anything else, but expect some entries not to fully land
on a first pass — there's no worked scenario yet to anchor a term to.
Worth a second read after [10-walkthrough.md](walkthrough.md), once
the concepts here have actually been seen in action.

## Non-goals

This glossary defines HoyaDB vocabulary. Schema and relation semantics are specified in `SPEC-0003`; this page does not introduce additional design choices.

## Ack (acknowledgement)

The backend's confirmation that a [command](#command) or
[entity-diff](#entity-diff-fallback-path) write was durably received. An ack is distinct
from *acceptance*: a command can be acked (received) and still be rejected
or corrected once the backend actually runs it. See
[06-sync/command-protocol.md](../design/spec/SPEC-0008-command-protocol.md).

## Bucket

A named, server-computed grouping of records that a client either holds in
full or doesn't, used to express [scoping](#scoping) without re-deriving it
per client session. See [06-sync/scoping-topology.md](../design/spec/SPEC-0012-scoping-topology.md).

## Command

The primary sync unit for a write with domain logic: a named action (e.g.
`add-order-v1`) carrying validated arguments plus a
[trusted context](#trusted-context). A command runs immediately against the
local store (see [optimistic apply](#optimistic-apply)), then is re-run
authoritatively by the backend. See
[06-sync/command-protocol.md](../design/spec/SPEC-0008-command-protocol.md).

## Command handler

The developer-authored function that implements a command's actual logic,
once on the client and once on the backend. hoyadb runs the handler at the
right time (optimistic local apply, authoritative server re-run) but never
inspects or verifies what it does — see
[06-sync/command-protocol.md](../design/spec/SPEC-0008-command-protocol.md)'s determinism
section. A handler is `async` and may read from the local store as well as
write to it; hoyadb serializes handler execution (only one runs at a time)
and commits a handler's writes as one atomic, ordered local transaction —
see [command-protocol.md](../design/spec/SPEC-0008-command-protocol.md#3-handler-execution-model).

## Conflict resolution

How the [entity-diff](#entity-diff-fallback-path) fallback path decides which of two
writes to the same field wins when they disagree. Not applicable to the
[command](#command) path, whose backend re-run is itself the resolution.
See [06-sync/conflict-resolution.md](../design/spec/SPEC-0011-conflict-resolution.md).

## Entity-diff (fallback path)

The plain create/update/delete sync path for tables or fields with no real
domain logic to express as a [command](#command). Lower-ceremony than a
command in that it skips designing domain logic, but not in authorization:
an entity-diff write is checked against the same
[trusted context](#trusted-context) and [membership rule](#membership-rule)
a command would be. See
[06-sync/entity-diff-fallback.md](../design/spec/SPEC-0010-entity-diff-fallback.md).

## Idempotency key

A client-generated, time-sortable identifier (ULID-leaning) attached to
every command so a backend can tell a resent command (retry after a lost
[ack](#ack-acknowledgement)) apart from a genuinely new one. See
[06-sync/command-protocol.md](../design/spec/SPEC-0008-command-protocol.md).

## Membership rule

A server-side declaration of what data a given identity is entitled to
hold (e.g. "this account's data"), computed once server-side rather than
inferred from which queries a client's UI happens to have mounted. See
[06-sync/scoping-topology.md](../design/spec/SPEC-0012-scoping-topology.md).

## Offline queue

The durable, ordered, local record of sent-but-unacknowledged commands
(and entity-diff writes), tracked through explicit states so an app
restart can recover rather than lose a write silently. See
[06-sync/offline-queue.md](../design/spec/SPEC-0009-offline-queue.md).

## Optimistic apply

Running a [command](#command) (or entity-diff write) against the local
store immediately, before the backend has confirmed it, so the app feels
instant. The backend's later authoritative outcome can confirm, correct,
or reject this local result — see [rollback](#rollback).

## Reset / reinitialize operation

The explicit, developer-invoked operation that rebuilds hoyadb's local
store (or points it at a fresh one), used when a client's
[membership](#membership-rule) narrows and previously-synced data must
stop being held. hoyadb never triggers this on its own. See
[06-sync/scoping-topology.md](../design/spec/SPEC-0012-scoping-topology.md).

## Rollback

Reverting a local [optimistic apply](#optimistic-apply) after the backend's
authoritative outcome differs from or rejects it. hoyadb surfaces the
correction and exposes the rollback path; deciding what the app does next
(retry, notify the user, reconcile some other way) is the integrating
developer's responsibility. See
[06-sync/command-protocol.md](../design/spec/SPEC-0008-command-protocol.md).

## Schema version

The version number of a client's locally-declared schema, carried on every
command/entity-diff payload so the backend can detect a mismatch. Distinct
from hoyadb's own library versioning — see
[NFR-0005](../design/requirements/nfr/NFR-0005-api-compatibility.md).
Migrating a client between schema versions is specified in
[03-data-model/migrations.md](../design/spec/SPEC-0005-schema-migrations.md).

## Scoping

The general term for how much data a client holds — computed from
[membership rules](#membership-rule), not from UI state. See
[06-sync/scoping-topology.md](../design/spec/SPEC-0012-scoping-topology.md).

## Set-membership change

The default trigger for a live [subscription](#subscription) to re-notify:
a record entering or leaving the query's matched result set. Contrasted
with a field-level change to an already-matched record, which does *not*
re-notify by default. See
[05-reactivity/subscriptions.md](../design/spec/SPEC-0006-subscriptions.md).

## Storage interface

The narrow, engine-agnostic contract that every concrete storage engine
implementation (SQLite-family default, or another engine on another
platform) must satisfy, so the layers above it don't change per platform.
See [04-storage/storage-interface.md](../design/spec/SPEC-0001-storage-interface.md).

## Subscription

A live-updating handle on a query's result, as opposed to a one-shot read
of the same query. See
[05-reactivity/subscriptions.md](../design/spec/SPEC-0006-subscriptions.md).

## Trusted context

An opaque, already-authenticated identity value that rides alongside a
command or entity-diff write, supplied by the integrating developer's own
authentication mechanism, which hoyadb carries faithfully and makes
client-unforgeable once set. hoyadb does not establish this identity
itself. See [06-sync/command-protocol.md](../design/spec/SPEC-0008-command-protocol.md).
