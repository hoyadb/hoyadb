---
id: THR-0015
title: Schema metadata or migration and reset failures erase or mislabel local data
type: threat
status: identified
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [ARC-0001]
tags: [security, stride, schema, migration, data-loss]
---

# THR-0015: Managed schema lifecycle data loss

The schema declaration, authored migration plan and persisted catalog form a
new input boundary. Treating missing/corrupt metadata as a fresh store, confusing
application version with private format version, accepting stale state or
committing metadata separately from rows can erase valid local data or label
partly migrated rows as current. A reset of unknown reserved/queue data could
silently discard pending writes. Severity: high for local data integrity.

REQ-0041/API-0004 require exact state/codec validation, namespace enforcement,
compatible legacy adoption, expected-state comparison and whole-plan atomicity.
Only a known supported queue-free forward missing-chain state can select reset;
a migration execution failure cannot. TC-0026 ADR-0015–ADR-0024 verify those controls with
owned fixtures, actual SQL errors and genuine restart observations.

## STRIDE

- Spoofing: forged schema/version metadata can impersonate a supported managed
  state; strictly decode its complete known shape and canonical definition.
- Tampering: hostile names, altered metadata, stale plans or partial schema
  writes can redirect or relabel data; use bound values, compare expected state,
  validate complete final inventory/rows and commit all state atomically.
- Repudiation: no durable audit-trail or identity guarantee is introduced;
  typed errors and recorded synthetic test outcomes are not an audit log.
- Information disclosure: diagnostics must not emit row contents, schema values
  containing secrets or payload telemetry. Existing plaintext exposure remains
  THR-0002; no new remote sink is introduced.
- Denial of service: malformed lengths/UTF-8, excessive schema size, scans and
  storage locks can prevent open. Reject malformed framing safely and roll back
  ordinary errors; resource quotas and hostile-file sandboxing remain unproved.
- Elevation of privilege: no authorization/security principal is introduced.
  The reserved-name barrier protects library state, not application permissions.

## LINDDUN and retention

Linkability and identifiability remain within existing application-local records;
relations/metadata introduce no remote identity aggregation. Nonrepudiation is
not introduced because there is no new audit service. Detectability and disclosure
remain possible through the existing plaintext local-store boundary. Unawareness
and noncompliance risks include unexpected deletion, addressed by explicit
chain-or-reset behavior, supported-state checks and documented limits; this is
not a new consent or legal-compliance mechanism.

Application data retains DM-0001's classification and application-controlled
retention. Canonical schema/counter metadata stays local for the managed store's
lifetime; reset replaces owned application data/metadata under the declared
policy. Test fixtures contain no personal data, and cleanup touches only their
owned files/OPFS namespace. Unknown or future bookkeeping, including queues,
is preserved through rejection until its lifecycle is separately specified.

This is a design-time threat review for a bounded library change. ASVS release
review, encryption/backup controls and crash/power-loss recovery remain later
gates; no release security certification is claimed by these test cases.
