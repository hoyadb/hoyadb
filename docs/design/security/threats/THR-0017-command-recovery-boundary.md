---
id: THR-0017
title: Forged scopes or incomplete command recovery corrupt local and pending work
type: threat
status: identified
owner: contact@btk-consulting.fr
created: 2026-10-23
updated: 2026-10-23
parents: [ARC-0001, ARC-0002]
tags: [command-sync, commands, stride, privacy]
---

# THR-0017: Command execution and recovery boundary

Severity high for integrity and confidential retained command data. New inputs
are host scope calls, structured arguments/context, persisted recovery state and
transport outcomes. RFD-0003 is proposed; this threat review does not approve its
policies. REQ-0043 is the mitigation requirement and TC-0032 the security evidence.

| STRIDE category | Hazard | Proposed control and cases |
| --- | --- | --- |
| Spoofing | Stale/cross-owner scope or outcome impersonates live work | Checked owner/ticket/generation and key correlation; C04/C12/C15 |
| Tampering | Partial enqueue, forged dependencies, unsafe correction or stale rollback | One transaction, exact metadata, explicit paths, validated plans and conflict guards; C06–C11/C13/C14/C17 |
| Repudiation | Receipt confused with authoritative success or backend audit | Distinct durable states and typed local events; C09/C12; no backend identity/audit claim |
| Information disclosure | Identity/args/undo images enter logs, status, callbacks after disposal | Immutable context, redacted diagnostics/status and liveness checks; C05/C18/C20 |
| Denial of service | Unbounded JSON/queue, hung handler, retry storm, retained tombstones | Decode/capacity limits, explicit abort, configured backoff and observable recovery pressure; C03/C05/C16/C19 |
| Elevation of privilege | Client log or opaque identity treated as backend authorization | No log replay protocol; backend re-runs and independently authenticates; C05/C18 |

LINDDUN: envelopes, dependency links and before-images can link/identify personal
records already held by the application. They remain Confidential device-local
data until the developer transport sends intended envelopes; undo images are
never transmitted. No new telemetry/diagnostic export is introduced. Receipt
records retain necessary recovery data until outcome/reconciliation; unresolved
work is bounded by the live-record cap, not silently evicted. Resolved history
is count-bounded; its default exposed status projection omits payloads. Local
retained correlation records may include Confidential envelopes/outcomes needed
for exact duplicate comparisons; clear undo images when no recovery requires
them, and evict unneeded records under the history bound. This is local retention,
not redacted physical storage or encryption. Cancellation tombstones retain key/
state and any still-needed recovery data until explicit forget. Exact retained
fields are API-0007; successful rollback should clear unnecessary undo images.

Existing plaintext at-rest exposure remains THR-0002 and is not solved by command sync.
Hostile arbitrary native pointers, foreign unwinding, malicious host loops and
independent concurrent clients remain unsupported. Tests cannot prove backend
deduplication, authorization, delivery or physical power-loss durability. ASVS
release and encryption/backup gates remain later work.
