---
id: ADR-0009
title: Schema migrations — apply an authored chain, otherwise reset the local database
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0002]
tags: [data-model, migrations, schema]
---

# ADR-0009: Schema migrations — apply an authored chain, otherwise reset the local database

## Decision

Apply an authored migration chain when it exists; otherwise reset to the latest
declared schema. Never infer an unauthored migration or leave partial upgrade
state. Migration is local and runs before synchronization resumes. Backend schema
compatibility remains governed by ADR-0013.

## Consequences

A reset discards that client's locally held data. Applications that cannot accept
that outcome must supply the missing migration chain. A reset does not itself
promise resynchronization. Migration primitives, reset/pending-write interaction
and atomic failure behavior are defined by SPEC-0005, ADR-0031 and API-0004;
this decision alone does not grant permission to delete an application's store.
