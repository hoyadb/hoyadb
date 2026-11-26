---
id: ADR-0015
title: On membership revocation hoyadb exposes a reset operation, it does not purge on its own
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0006]
tags: [sync, scoping, membership, security]
---

# ADR-0015: On membership revocation hoyadb exposes a reset operation, it does not purge on its own

Source: `legacy/PLAN_DECISIONS.md` D7, folded into `legacy/VISION.md`'s "Sync — what a
client holds" and `legacy/docs/06-sync/scoping-topology.md`.

## Context and problem statement

When a client's server-computed membership narrows (`ADR-0006`) — say it is
removed from an org — nothing said what happens to data it has already
synced. This is a data-retention question with a real confidentiality
dimension.

## Considered options

- **Expose an explicit reset/reinitialize operation the developer invokes**;
  hoyadb neither detects the revocation nor purges anything on its own.
- **Selectively purge** the records that dropped out of the recomputed
  membership, on the next successful sync.
- **Purge immediately** on being told membership changed.

## Decision outcome

In the context of data a client already holds when its membership narrows,
facing hoyadb's inability to detect that event or to reconcile it against
writes queued offline, we decided for **an explicit, developer-invoked
reset/reinitialize operation** and against selective purging on next sync
and against immediate purging, to achieve a mechanism hoyadb can actually
implement correctly as a library, accepting that **if the developer does
not invoke it, previously-synced data simply persists**.

### Consequences

**Good**
- Reset is total and unambiguous — rebuild the local store, or point at a
  fresh one (e.g. a new local filename) — rather than a partial purge whose
  correctness hoyadb cannot verify.
- Consistent with hoyadb's posture throughout: D7's answer states it
  directly — "Don't forget we are creating a library, not an application.
  We provide the API, process and integration guide, the developer manage
  the correct integration."
- Detecting the membership change is a developer-owned process; hoyadb does
  not poll for or infer it, so it makes no claim it cannot back.

**Bad**
- **This is a deliberate, recorded decision not to mitigate a real
  confidentiality exposure.** `THR-0004` carries it at `accepted-risk`
  rather than `mitigated`, which is the honest status: hoyadb offers the
  tool and does not police its use.
- Queued writes against data the client is about to lose scope over are
  **not** special-cased. They are sent and processed normally once
  connectivity returns, and the backend's own authorization check rejects
  whatever the now-revoked identity may no longer write — surfaced through
  the ordinary rejection path (`ADR-0012`).
- A reset's interaction with unacknowledged queued commands is undefined —
  the same disclosed gap `ADR-0009` records for a migration reset.

### Confirmation

Confirmed by `SPEC-0012` (the reset operation and the queue interaction)
and the membership-revocation test cases in `TP-0001`, which verify the
operation exists and is total — not that data is purged without it, since
by decision it is not.

## Traceability

- Parent: `OBJ-0006`.
- Referenced by: `SPEC-0012`, `SPEC-0009`.
- Accepted risk recorded as: `THR-0004`.
