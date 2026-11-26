---
id: ADR-0023
title: hoyadb's public API follows semver; its own bookkeeping uses a reserved namespace
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0002]
tags: [versioning, compatibility, library]
---

# ADR-0023: hoyadb's public API follows semver; its own bookkeeping uses a reserved namespace

Source: `legacy/PLAN_DECISIONS.md` D15, folded into `legacy/VISION.md`'s "Versioning &
compatibility" and `legacy/docs/08-non-functional/open-questions.md`.

## Context and problem statement

Distinct from `ADR-0009`, which covers an *application's* schema evolution.
This is about the *library's* own public API and on-disk bookkeeping format
across hoyadb releases — and about hoyadb's own tables and fields not
colliding with an application's schema, since `ADR-0002` means hoyadb and
the app share one schema namespace.

## Considered options

- **Semver on the public API, a reserved naming convention for hoyadb's own
  bookkeeping, and a major bump that *may* require a local data reset with
  the developer informed.**
- **No compatibility commitment** — treat every release as potentially
  breaking.
- **Guarantee on-disk format compatibility across all upgrades**, never
  requiring a reset.

## Decision outcome

In the context of hoyadb's own releases, facing a shared schema namespace
with the application and an evolving on-disk bookkeeping format, we decided
for **semver plus a reserved bookkeeping namespace, with a reset possible
but never default on a major bump** and against no commitment and against
guaranteeing on-disk compatibility unconditionally, to achieve
compatibility as a stated main goal without promising a migration path for
every conceivable future format change, accepting that a major version bump
can, in the worst case, cost an integrating developer their local data.

### Consequences

**Good**
- Compatibility is treated as a main goal, not an afterthought — D15's own
  framing.
- The reserved namespace (for example a fixed `__hoyadb_` prefix) means
  hoyadb's bookkeeping tables and fields — the offline queue's reserved
  table among them (`ADR-0016`) — can never collide with an application's
  own schema, present or future.
- A reset on a major bump is a *possible* consequence, never the default a
  developer should plan around, and hoyadb informs them when a given
  release requires it.

**Bad**
- **The exact reserved prefix is not fixed** — implementation-time detail.
  The *policy* is decided, the literal string is not.
- Semver on the public API says nothing about the on-disk format's own
  compatibility across minor versions; only the major-bump reset case is
  addressed.
- **Incremental adoption from an existing RxDB/WatermelonDB/Realm-backed
  app is unaddressed anywhere in the tree** — every migration example
  assumes hoyadb owned the schema from v1
  (`legacy/docs/08-non-functional/open-questions.md`). A disclosed gap.
- **Multi-tenant/multi-app backend scoping** — whether one backend serving
  several hoyadb apps, or staging versus production, needs tenant-scoped
  idempotency keys, command names or membership rules — is also unaddressed.

### Confirmation

Confirmed by `NFR-0005` (the compatibility bar) and the reserved-namespace
test cases in `TP-0001`, which verify an application schema cannot declare
a colliding name.

## Traceability

- Parent: `OBJ-0002`.
- Realized by: `NFR-0005`, `REQ-0033`.
- Distinct from: `ADR-0009` (application schema migration).
