---
id: ADR-0009
title: Schema migrations — apply an authored chain, otherwise reset the local database
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0002]
tags: [data-model, migrations, schema]
---

# ADR-0009: Schema migrations — apply an authored chain, otherwise reset the local database

Source: `legacy/PLAN_DECISIONS.md` D1, folded into `legacy/VISION.md`'s "Schema
migrations" and `legacy/docs/03-data-model/migrations.md`.

## Context and problem statement

Because `ADR-0002` makes hoyadb own its schema, hoyadb must also own how an
app's schema evolves after it ships without losing local data. Nothing in
`legacy/VISION.md` or `legacy/refined-vision/` covered schema evolution at all before D1
— this was a genuine gap, not an undecided default.

## Considered options

- **Apply an explicitly authored migration chain where one exists; reset
  the local database to the latest schema where it does not.** Takes its
  direction (not a literal implementation) from WatermelonDB's migration
  model.
- **Infer a migration** where no chain was authored — derive the schema
  delta automatically and apply it.
- **Refuse to open** a database whose version has no authored chain,
  leaving the app to handle it.

## Decision outcome

In the context of evolving an application's schema across its releases,
facing the absence of any prior design for it, we decided for **apply the
chain if it exists, otherwise reset to the latest schema** and against
inferring an unauthored migration and against refusing to open, to achieve
a deliberately simple policy with no partial-upgrade state and no guessed
migrations, accepting that a missing chain means discarding that client's
locally-held data.

### Consequences

**Good**
- Two outcomes only, both explicit: the chain ran, or the database was
  reset. No partial-upgrade state is ever left behind, and no migration is
  ever inferred that a developer did not author.
- A migration is a purely local operation applied on app start, before sync
  resumes — sequenced independently of sync, whose own schema-version check
  (`ADR-0013`) handles a client out of date relative to the backend.

**Bad**
- **A reset discards locally-held data for that client.** An app that
  cannot tolerate this for a given schema jump must author the missing
  chain; hoyadb offers no softer fallback than "chain exists" vs. "reset."
- A reset does not on its own resync data back down from the backend —
  whether that happens depends on how scoping and the offline queue are
  configured for that deployment.
- The exact set of migration step primitives (add/drop/rename column,
  add/drop table, backfill) is **not fixed**; implementation-time detail.
- **A real, disclosed gap**: a reset and the offline queue do not
  cross-reference each other. Dropping the local store presumably drops
  commands still sitting unacknowledged in the queue, silently discarding
  a user's pending write. Neither `legacy/docs/03-data-model/migrations.md` nor
  `legacy/docs/06-sync/offline-queue.md` names this interaction today. **Not
  resolved here.**

### Confirmation

Confirmed by `SPEC-0005` (schema migrations) and the migration test cases
in `TP-0001`, which cover both the chain-exists and no-chain paths. The
reset-versus-queued-writes interaction has no test case, because it has no
decided behaviour to verify.

## Pros and cons of the options

### Chain-or-reset (chosen)
- Good: simple, explicit, no inferred migrations, no partial state.
- Bad: data loss when a chain is missing; interaction with the offline
  queue undefined.

### Infer the migration
- Good: no data loss in the common additive case.
- Bad: a guessed migration that a developer never authored or reviewed, on
  a schema hoyadb owns but does not understand semantically.

### Refuse to open
- Good: no data loss and no guessing.
- Bad: leaves an app with a database it cannot open and no recovery path
  hoyadb provides — worse for the integrating developer than a reset.

## Traceability

- Parent: `OBJ-0002`.
- Referenced by: `SPEC-0005`.
- Depends on: `ADR-0002` (hoyadb owning the schema is what creates this
  problem), `ADR-0013` (schema version on the wire).
