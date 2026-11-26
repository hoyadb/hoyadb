---
id: ADR-0029
title: Group formal artifacts under docs/design and retain supporting guides and research separately
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

## Context and problem statement

The repository has no database implementation yet. Formal artifacts were
introduced alongside plain-Markdown design material, then commit
1647b178d4fb3ee925d574367d2f9d526a4bb116 moved the original material into a
legacy directory. User-approved consolidation now requires a clear current
reading path, preserved research and examples, and removal of that directory
without losing content or provenance.

Governance's artifact catalogue specifies directories relative to a project
root. FLOW-2 separately names specs/ for change specifications. The Constitution
phase in framework/01-lifecycle.md allows a project deviation recorded in an ADR.
This ADR records the layout deviation; governance content and checks remain in
force. Research front matter describes sources rather than project artifacts,
so it must remain outside the artifact checker's recursive scan root.

## Considered options

- Keep all formal artifact directories at repository root.
- Put all documents, including research metadata, in a single scanned docs root.
- Use docs/design as the formal artifact root and keep guides, research, and
  provenance records in sibling directories beneath docs.

## Decision outcome

In the context of organizing hoyadb's documentation before implementation,
facing scattered formal artifacts and supporting material with incompatible
metadata, we decided for docs/design as the formal artifact root and against
root-level artifact directories and a mixed scanned docs root, to achieve clear
navigation and complete traceability checks, accepting an explicit project
layout deviation and the need to update path references.

### Consequences

- Preserve existing artifact IDs and typed relationships. Registry file paths
  are relative to docs/design, where spec/registry.yaml now lives.
- Generate docs/design/traceability/rtm.md by passing docs/design to governance's
  unchanged generator and checker.
- Keep developer explanations under docs/guides and prior-art research plus
  hidden import trackers under docs/references. Source metadata retains its
  original observation dates; relocation is not a new verification of research.
- Keep DECISIONS.md at repository root as the append-only decision log.
- Retain this change's scope, manual acceptance cases, verification outcome and
  source ledger under docs/references/documentation-history.md and
  docs/references/documentation-sources.csv instead of root-level specs/.
  Temporary execution steps and resume checkpoints are removed after completion.
- Preserve accepted ADR bodies and prior decision-log entries. Old path strings
  inside them are historical citations resolved against the pinned baseline
  through the source ledger, not instructions to read deleted local files.
- Do not resolve product-design questions during the migration. Carry open
  questions forward explicitly; record superseding ADRs for changed decisions.

### Confirmation

The operator approved the docs consolidation and execution plan in this session.
Confirm implementation through docs/references/documentation-history.md: complete source
coverage, local link integrity, original ADR-body preservation, clean formal
traceability, and an up-to-date generated RTM. Independent agent review supplements
operator review; it does not authorize merge or publication.
