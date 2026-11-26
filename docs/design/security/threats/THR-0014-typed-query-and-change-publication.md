---
id: THR-0014
title: Typed queries or premature change publication misrepresent local data
type: threat
status: identified
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [ARC-0001]
tags: [security, stride, storage, query]
---

# THR-0014: Typed query and committed-change boundary

New schema/query inputs and stored value tags can produce type confusion,
SQL injection, partial results or notifications for rolled-back data. Severity:
medium for local data integrity. REQ-0040/API-0003 require bound SQL, strict
physical/logical decoding, pre-execution query validation and publication only
after commit; TC-0025 P07/P09/P10/P11 exercise those controls.

STRIDE: spoofing is not applicable because no identity or authentication is
introduced; tampering covers executable SQL inputs, malformed tags and stale
indexes; repudiation is not applicable because this slice promises no audit
trail; information disclosure covers accidental payload logging and is also
THR-0002 for plaintext persistence; denial of service remains possible through
oversized inputs or broad queries, with no resource-limit claim; elevation of
privilege is not applicable because there is no authorization boundary.
No network or telemetry is introduced. This is not a hostile-file sandbox or
release security assessment. ASVS release review remains a later gate; no ASVS
conformance claim follows from these bounded storage tests.

Rows retain DM-0001's local classification and application-managed retention;
query/change snapshots are caller-owned memory, not a newly retained log.
LINDDUN: linkability/identifiability and disclosure are possible within existing
application data but no new aggregation/remote sink is created; nonrepudiation
and detectability are not introduced; unawareness/noncompliance remain the
integrator's existing local-data obligations. No personal data enters fixtures.
