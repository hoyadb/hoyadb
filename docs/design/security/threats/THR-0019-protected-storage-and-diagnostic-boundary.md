---
id: THR-0019
title: Wrong keys stale ciphers sidecars or diagnostics expose local recovery
type: threat
status: mitigated
owner: contact@btk-consulting.fr
created: 2026-10-24
updated: 2026-10-24
parents: [ARC-0001, ARC-0002]
tags: [phase-six, security, stride, linddun]
---

# THR-0019: Wrong keys stale ciphers sidecars or diagnostics expose local recovery

## Threat and trust boundaries

An app supplies secret key bytes and opaque trusted context to the local owner;
the concrete engine crosses plaintext memory to durable native/OPFS files.
Foreign workers/C ABI/Swift callbacks carry bounded owned requests. Platform
backup hints and application-owned logging/transports are separate boundaries.
A wrong-key fallback, stale unauthenticated cipher, plaintext temp/sidecar,
mutable context snapshot or diagnostic dump can disclose/alter rows and durable
command/entity recovery without backend authorization.

| STRIDE | Treatment |
| --- | --- |
| Spoofing | immutable captured identity; backend re-authentication remains external |
| Tampering | maintained authenticated page encryption, wrong-key rejection, atomic recovery and corruption tests |
| Repudiation | no claim that client identity authenticates server authorship |
| Information disclosure | key-before-catalog, memory temps, encrypted sidecars, redacted boundaries/diagnostics, verified backup hint |
| Denial of service | bounded decode/key size, fail-closed explicit errors, preserve data on failed opens |
| Elevation of privilege | no extension/SQL injection or library auth/network capability introduced |

LINDDUN: linkability/identifiability/disclosure apply to identities, record names,
checkpoint/scope and queue data; do not include them in diagnostics. Unawareness
is reduced by explicit protected/unprotected entry points and truthful backup
capabilities. Detectability/volume remain possible from encrypted file size and
allowed counts; ciphertext does not hide access patterns. Nonrepudiation and
compliance are not proven by a client cipher. Use ASVS 5.0 L2 as the scoped
security review baseline, without claiming application-wide certification.

## Controls and residuals

REQ-0030/0031/0032 and REQ-0045, implemented by SPEC-0032/API-0010 and verified by
TC-0035. Reject obsolete cipher builds, plaintext/key downgrade, custom crypto,
automatic migration/rekey and unreviewed runtime additions. Keep the original
THR-0002/0007 residuals: compromised running apps/caller logging/backups outside
OS hints remain outside the claim; caller key entropy/lifecycle is external.
No user store is migrated/reset by this task. This Phase 6 artifact authorizes no new platform or release action.

## Verification status — 2026-10-24

Operator-approved ADR-0034 policies and exact graph are implemented and reviewed.
[Phase 6 execution evidence](../../../implementation/phase-six-security.md) records
protected native/Swift/OPFS, fault/restart, privacy and pinned-container checks,
and the final crypto plus lifecycle Claude reviews. Scope and residual limits
remain explicit; this status does not certify new mobile targets or publication.
