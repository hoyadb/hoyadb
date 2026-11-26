---
id: TC-0054
title: GitHub delivery preserves checks and rejects unsafe releases
type: test-case
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-26
updated: 2026-11-26
parents: [TP-0001]
verifies: [SPEC-0050]
tags: [distribution, regression, tooling]
---

# TC-0054: GitHub delivery

1. CI retains all GitLab scripts, adds clean npm artifact consumption and fetches
   each standalone Cargo graph before offline commands. Actions are SHA-pinned;
   PRs cannot use native runners, secrets or publishing permissions.
2. The aggregate status accepts only a nonempty collection of successful jobs;
   failed, cancelled, skipped, malformed and absent results fail.
3. Release preflight rejects malformed tags, version mismatch, private packages,
   missing MIT license or metadata and mismatched peer/dependency versions.
4. First-party license bytes reach every npm package. Root remains private;
   runtime exports and native Codegen remain unchanged.
5. Maven preparation rejects a missing/mismatched certified artifact and incomplete
   metadata; produces sources, documentation, notice closure and Maven metadata
   without downloading dependencies or inventing namespace ownership.
6. Workflow lint validates CI/release/Pages. Release builds use the checked tag;
   npm receives only validated tarballs, and Pages receives only the freshly
   verified site. Native failures prevent dependent publishing jobs.
7. The shared spike formatter preserves all core queue-state spellings, including
   `Discarded` as `"discarded"`. Native tests and browser WASM compilation pass.
8. Android consumer guard tests need no installed compiler. Synthetic, hash-bound
   compiler/runtime files exercise distinct compiler drift, runtime drift and
   Java injection rejections without activating a subprocess.
9. Browser dispatch includes the shared backfill and both pull-recovery review
   cases; client expectations request all added cases and check their names.

Run `python3 -m unittest discover -s tools/ci -p 'test_*.py'` and
`python3 -m unittest discover -s tests/packaging -p 'test_*.py'`, workflow lint,
compatibility and traceability gates. Actual GitHub, registry and reviewed native
runner execution require their configured external services/toolchains.
