---
id: THR-0018
title: Ambiguous diffs stale pulls or reset races corrupt scoped recovery
type: threat
status: identified
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [ARC-0001, ARC-0002]
tags: [entity-sync, stride, privacy, pull, reset]
---

# THR-0018: Entity/pull/reset trust boundaries

New input is bounded entity context, per-write outcomes, authoritative pull
pages, persisted field/progress/scope state and reset choices. Integrity impact
is high. RFD-0004 is a proposal; REQ-0044 and TC-0034 are the mitigation/evidence
chain, not a claim that controls or operator approval already exist.

| STRIDE | Hazard | Proposed control |
| --- | --- | --- |
| Spoofing | A repeated ID, foreign scope or pre-reset response targets new work | Immutable writeId plus table/record tuple, opaque scope binding, owned request/generation checks |
| Tampering | Partial page commit, forged clocks/schema, reset/undo overwrite | Exact versioned records, bounded schema/value validation, atomic guarded plan, conservative live-row conflict |
| Repudiation | Client clock or receipt mistaken for server resolution | Distinct receipt/outcome/progress; AuthorityRequired for unresolved equal-time values |
| Information disclosure | Scope switch exposes prior tenant or stale callbacks deliver retained values | Explicit mismatched-scope rejection/reset, invalidate owned deliveries after commit, payload-free status/errors |
| Denial of service | Unbounded pages/metadata, endless continuation or retries | Bounded pages/values, nonterminal continuation progress, common queue capacity/backoff, explicit progress and retained-tombstone count |
| Elevation of privilege | Entity path or merge hook bypasses membership/auth | Identical trusted context to commands; backend authentication/membership; hooks cannot manufacture authorization |

LINDDUN: field clocks, tombstones and queue links may reveal record relationships
and activity; progress may reveal counts. Treat application values, identities,
opaque checkpoints and scope bindings as Confidential. Keep them device-local
except the intended developer transport exchange. Status/errors omit contents;
no telemetry or diagnostic export. Bounded live/history queue retention follows
command sync; checkpoints/progress remain until reset, tombstones until reset absent
a designed server compaction protocol. State that metadata growth explicitly.

Only explicit reset discards scoped data; no revocation detector is added.
Client scope is not authentication, and tests cannot prove backend authorization,
real server arrival order or physical power-loss safety. Encryption/backup/ASVS
release review remain security gates; existing plaintext risk is not closed here.
