# Documentation history

The original plain-Markdown design was formalized into stable-ID artifacts,
then commit [1647b17](https://gitlab.com/alocas-ia/hoyadb/-/commit/1647b178d4fb3ee925d574367d2f9d526a4bb116)
moved its source material into the former legacy directory. The documentation
consolidation requested by the operator on 2026-10-20 groups current artifacts
under docs/design, preserves useful guides and research, and removes
superseded copies. [ADR-0029](../design/decisions/ADR-0029-documentation-artifact-root.md)
records the layout deviation. No database implementation was introduced.

## Historical citations

Old paths in accepted ADRs and original decision-log entries identify their
historical sources; they do not refer to live files in this checkout.
The [source ledger](documentation-sources.csv) records all 78 original file
paths, SHA-256 hashes, dispositions and current destinations. Destination paths
are relative to the repository root. For a retired source, the destination
identifies its current replacement; it need not reproduce the original prose.

All original sources are recoverable from the pinned baseline:

```sh
git show 1647b178d4fb3ee925d574367d2f9d526a4bb116:<original-source-path>
```

The older competing refined-vision draft was superseded, rather than promoted.
Its unique [dated research checkpoint](design-alternatives/research-status-2026-09-13.md)
was retained separately. The latest corrections from `b1c9aea` remain in the
formal specifications and walkthrough: acknowledgement means durable receipt;
authoritative outcomes supply replacement IDs and release dependent commands.

## Retained content

- Formal artifacts keep their original IDs. All 148 original formal files were
  relocated; 142 remain byte-identical. Six have focused navigation, wording,
  date or registry changes. One layout ADR and a reading index were added.
- Seven developer guides retain the glossary, walkthrough, sync integration,
  native binding, testing and relation examples. All 22 fenced code examples
  and 18 glossary terms were preserved. Guide APIs remain illustrative.
- All 40 prior-art research files, including eight hidden import trackers,
  retain their original source metadata. Seven historical alternatives notes
  retain supporting evidence. Research claims were not refreshed.
- Original needs, vision, documentation-plan and decision-answer copies were
  superseded by current artifacts and remain available at the pinned baseline.
  D0 maps to ADR-0008; D1–D20 map in order to ADR-0009–ADR-0028.
- All 28 original ADR files remain byte-identical; DECISIONS.md retains its
  complete original prefix, with the layout decision appended.

## Acceptance and verification

Scope was documentation organization, source preservation and path updates.
Product behavior, unresolved design questions and the governance submodule
were outside scope. Manual artifact inspection was appropriate for prose;
there is no database implementation or application test runner to exercise.

| Acceptance case | Expected result | Observed result |
| --- | --- | --- |
| Given the baseline inventory, when removing source copies, then every source has a verified disposition and recoverable original. | 78 unique source hashes and valid destinations. | Passed. |
| Given competing vision versions, when consolidating, then the latest receipt/outcome corrections survive. | Outcome-based ID replacement and dependency release remain explicit. | Passed, including independent review. |
| Given guides and research, when relocating, then useful examples and provenance survive. | 22 code examples, 18 terms and 40 metadata records preserved. | Passed. |
| Given the formal artifact root, when checking, then IDs resolve and the generated RTM is current. | 129 registry entries resolve; zero traceability violations. | Passed; regenerated RTM is byte-identical to baseline. |
| Given the final documentation, when resolving local paths and anchors, then no live link requires removed files. | Zero broken local Markdown links or anchors. | Passed. |
| Given proposed source deletions, when independently reviewed, then no material content-loss finding remains. | No unresolved findings. | Passed after clarifying a historical dependency comment. |

The retained sync guide discloses the existing CorrectedError/transaction
ambiguity. Its historical split-transport comment is qualified by an explicit
current dependency-eligibility rule. Neither unresolved behavior nor draft
numeric NFRs were silently settled by consolidation.

Current formal checks, run from the repository root:

```sh
python3 ai-governance/tools/trace_check.py docs/design
python3 ai-governance/tools/rtm_gen.py docs/design --check
git diff --check
```

External URL availability and prior-art claims were not reverified. No external
publication or database release is part of this documentation change. To undo
consolidation, revert its commit; recover individual original files with the
pinned Git command above while preserving unrelated work.
