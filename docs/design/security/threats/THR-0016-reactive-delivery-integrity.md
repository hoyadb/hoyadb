---
id: THR-0016
title: Stale partial or canceled reactive notifications misrepresent data
type: threat
status: identified
owner: contact@btk-consulting.fr
created: 2026-10-22
updated: 2026-10-22
parents: [ARC-0001, ARC-0002]
tags: [reactivity, reactivity]
---

# THR-0016: Reactive delivery boundary

Severity medium for data integrity/resource lifetime. STRIDE: spoofed/stale
subscription handles (checked monotonic identity), tampered partial transaction
notifications (net committed snapshots), repudiation (typed local results without
payload logs), information disclosure (owned public rows strip metadata and
cancellation clears pending data), denial of service (shared maintenance,
count coalescing, final-cancel/drop releases caches; host drains row queues),
elevation (no new auth boundary, raw storage remains exclusive).

REQ-0042/API-0005 mitigate borrowed callback lifetimes, stale generations,
reentrancy and silent stale success. TC-0029 tests rollback, net-zero batches,
errors/retry, cancellation/close and actual host callbacks. LINDDUN: no new
identity, telemetry or egress; app data stays local and ephemeral queued
snapshots retain until drain/cancel/drop. Slow row consumers can retain queued
snapshots; hosts must drain/cancel, with no background scheduler promised.
Concurrent clients, hostile C pointers and foreign unwinding remain unsupported.
