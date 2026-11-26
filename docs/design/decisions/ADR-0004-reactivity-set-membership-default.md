---
id: ADR-0004
title: Reactivity — one query object, two modes, set-membership change as the default trigger
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0005]
tags: [reactivity, subscriptions, performance]
---

# ADR-0004: Reactivity — one query object, two modes, set-membership change as the default trigger

## Decision

Use a query object with one-shot and live modes. The observation default emits
on result-set membership changes. Count and selected-column observation are
explicit opt-ins under SPEC-0006 and SPEC-0007; they do not silently change the
default notification contract.

## Consequences

Incremental result maintenance and sharing equivalent subscriptions require
explicit correctness and cost checks. No observation mode is cheapest for every
query. Cross-platform notification scheduling and ownership remain part of the
binding contract; the host must honor lifecycle and delivery requirements.
