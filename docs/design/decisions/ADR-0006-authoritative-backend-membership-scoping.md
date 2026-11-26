---
id: ADR-0006
title: Topology & scoping — one authoritative backend, server-computed membership
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0003]
tags: [sync, scoping, topology, membership]
---

# ADR-0006: Topology & scoping — one authoritative backend, server-computed membership

## Decision

Use one authoritative backend with server-declared, server-computed membership
rules. Scope follows identity and permissions rather than the client's currently
open screens. The backend is the authority for conflict resolution and scoped
data delivery.

## Consequences

The backend must express, compute and bound membership work. Rule syntax is not
fixed by this decision. Membership narrowing does not authorize automatic local
purging; ADR-0015 and THR-0004 govern retained data. ADR-0018 defines same-identity
device behavior. Peer-to-peer topology is outside the current contract.
SPEC-0012 and the membership cases define acceptance.
