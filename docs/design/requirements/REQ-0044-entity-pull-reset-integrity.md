---
id: REQ-0044
title: Entity synchronization preserves correlation scope and atomic recovery
type: requirement
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [OBJ-0002, OBJ-0003]
mitigates: [THR-0018, RISK-0005]
tags: [entity-sync, integrity, bootstrap, reset]
---

# REQ-0044: Entity, pull and reset integrity

WHEN generating a configured entity mutation, THE system SHALL atomically
commit its exact patch and local/recovery effects through the shared durable
queue, retaining unambiguous per-write correlation without an idempotency key.
WHEN applying authoritative input, THE system SHALL preserve backend scope and
tie authority, validate bounded values and commit data, comparison metadata,
checkpoint and bootstrap progress together. Stale, invalid, conflicting or
failed application SHALL NOT advance a checkpoint or overwrite protected work.
WHEN explicitly resetting, THE system SHALL preserve recovery by default and
discard it only with the integrator's explicit discard policy; successful reset
SHALL invalidate old responses/subscriptions without reusing write identities.

RFD-0004 proposes the precise choices. This draft is not approval of those
policies. TC-0034 supplies executable acceptance and SPEC-0031 the delivery scope.
Authorization/membership stay backend-owned; opaque client context and scope
binding do not prove authorization. No implicit revocation purge or telemetry.
