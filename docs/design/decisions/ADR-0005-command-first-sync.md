---
id: ADR-0005
title: Sync mechanism — named commands as the primary write path, entity-diff as fallback
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0001, OBJ-0003]
tags: [sync, commands, entity-diff]
---

# ADR-0005: Sync mechanism — named commands as the primary write path, entity-diff as fallback

## Decision

Named commands are the primary synchronized write path. Keep entity-diff writes
as a lower-ceremony fallback, with the two paths coexisting per table/write.
Commands carry intent so backend domain logic can resolve operations that cannot
be reconstructed from a final value alone.

## Consequences

Both paths require the same trusted context, authorization and membership rules.
The backend independently runs its handler and remains authoritative. Local and
backend handler compatibility remains an application responsibility (ADR-0014,
RISK-0002). Maintaining two paths requires consistent queue, reconciliation and
recovery semantics. Bootstrap behavior is governed separately by ADR-0033 and
API-0009. SPEC-0008 and SPEC-0010 define the protocols.
