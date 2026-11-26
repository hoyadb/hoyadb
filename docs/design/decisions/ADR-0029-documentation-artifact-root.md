---
id: ADR-0029
title: Keep formal artifacts under docs/design
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-10-20
updated: 2026-10-20
parents: [VIS-0001]
tags: [documentation, governance, layout]
---

# ADR-0029: Documentation artifact root

## Context

Formal contracts need stable IDs, typed relationships and a single scan root.
Developer guides and runnable examples have different purposes and metadata.

## Decision outcome

Keep formal artifacts under `docs/design`. Registry paths are relative to that
root and traceability tools scan it explicitly. Keep developer guides under
`docs/guides`, examples under `examples`, and public HTML sources under
`docs/public`. Index current decisions in the root `DECISIONS.md`.

## Consequences

Source moves require updating incoming links and registry paths. Documentation
organization does not change product behavior. Fixtures with intentionally
invalid metadata stay outside the formal scan root.

## Confirmation

Run the traceability checker and regenerate/check the RTM using `docs/design`.
Validate relative links and the public documentation build.
