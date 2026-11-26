---
id: ADR-0025
title: Command handlers are async and may read locally; hoyadb serializes them and commits each atomically
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, commands, concurrency, transactions]
---

# ADR-0025: Command handlers are async and may read locally; hoyadb serializes them and commits each atomically

## Context and problem statement

The worked handler example only ever showed synchronous writes, with no
read, while the app-facing `execute()` call was already documented as
awaited elsewhere, with no stated connection between the two. Can a handler
read local state before deciding what to write — and if local reads are
async, what still guarantees a handler's writes apply as one atomic,
ordered local transaction?

## Considered options

- **Handlers are `async`; `db` supports awaited reads and writes; hoyadb
  runs at most one handler at a time and commits each handler's writes as
  one ordered transaction when it returns.**
- **Require synchronous handler code**, so atomicity falls out of the
  single-threaded execution.
- **Send the client's recorded action log** instead of the command, and
  have the backend replay it.

## Decision outcome

In the context of what a command handler may do locally, facing an async
query layer and an "instant, atomic, in-order local apply" promise made
elsewhere, we decided for **async handlers with serialized execution and a
single atomic commit per handler** and against requiring synchronous
handler code and against replaying a client-recorded action log, to achieve
read-before-write handlers without giving up ordering or atomicity,
accepting global serialization — one handler at a time across the whole
app — as the cost.

### Consequences

**Good**
- The guarantee is stated precisely: handler `db` access goes through a
  tracked object, so a later read in the same handler sees that handler's
  own earlier writes (read-your-own-writes), while nothing outside — a live
  subscription, other app code, another handler — observes any of it until
  the whole tracked log commits as one transaction. Partial mid-handler
  state is never visible.
- Client-side only: the envelope is still name, args and trusted context,
  and the backend still independently re-runs its own handler copy, so "the
  backend re-run is itself the resolution" is unchanged (`ADR-0005`).
- Establishes the explicit local execution order that makes `ADR-0026`'s
  batching safe.

**Bad**
- **Global serialization** — one handler at a time across the whole app,
  not scoped to overlapping tables or rows — trades throughput for
  simplicity. A finer-grained locking scheme was not designed and is a real
  follow-up if bulk-write throughput matters in practice. **Open.**
- **No liveness story.** Nothing specifies a timeout, cancellation or
  watchdog for a handler that never resolves (a stuck `await`, an unbounded
  loop). Under global serialization one hung handler blocks every other
  command indefinitely. **Open.**
- **The guarantee's scope against other write paths is unstated** —
  whether a concurrent entity-diff write or an in-progress `pull()` apply
  touching the same rows is excluded while a handler runs is not addressed
  anywhere in the tree. **Open.**
- The rejected action-log alternative survives only as an **optional,
  debug-only** attachment: the backend may always ignore it, and trusting
  it instead of re-running the handler silently removes the one authority
  check the command path relies on. Its concrete shape is not fixed.

### Confirmation

Confirmed by `SPEC-0008`'s execution-model section, `SPEC-0009`'s
durability guarantee resting on it, and the handler-ordering and
read-your-own-writes test cases in `TP-0001`. The three open items above
have no test cases, having no decided behaviour.

### Async, serialized, atomically committed (chosen)
- Good: read-before-write handlers; ordering and atomicity preserved.
- Bad: global serialization; no liveness story; scope vs. other write paths
  unstated.

### Require synchronous handlers
- Good: atomicity falls out for free.
- Bad: not warranted by precedent, and would forbid a legitimate local read
  inside a handler.

### Replay a client-recorded action log
- Bad: removes the backend's independent authority check — replaying
  recorded writes cannot catch "this product is actually out of stock now,"
  because nothing server-side evaluates the business rule. Restoring the
  check means either database constraints (strictly weaker than arbitrary
  handler code) or a second independent server-side pass, which is the
  handler re-run again with extra steps.

## Traceability

- Parent: `OBJ-0003`.
- Referenced by: `SPEC-0008`, `SPEC-0009`, `ARC-0002`.
- Enables: `ADR-0026`.
