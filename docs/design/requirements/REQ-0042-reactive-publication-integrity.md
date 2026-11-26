---
id: REQ-0042
title: Reactive delivery preserves committed state and cancellation
type: requirement
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [OBJ-0002, OBJ-0005]
mitigates: [THR-0016]
tags: [reactivity, reactivity]
---

# REQ-0042: Reactive publication integrity

WHEN a public transaction commits, THE system SHALL maintain observations from
its net committed changes and publish only owned complete snapshots/counts.
Failed transactions SHALL produce no notification. Post-commit refresh failures
SHALL be distinguishable from write failure and stale successful snapshots.
Cancellation/close SHALL suppress queued delivery and release registrations;
bindings SHALL validate liveness before each callback and retain no borrowed
core state across callbacks. Equivalent observers SHALL share maintenance and
count-only observations SHALL not materialize complete records.
