---
id: DEF-0050
title: Clean JavaScript CI lacks required WASM assets
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-30
updated: 2026-10-30
parents: [SPEC-0046]
affects: [SPEC-0045, REQ-0048]
regression_for: [TC-0050]
tags: [review, regression]
---

# DEF-0050: Clean JavaScript CI lacks required WASM assets

## Symptom

The 2026-10-30 review recorded REL-01: The Node CI job runs application integration tests that import ignored crates/hoyadb-web/pkg assets, with no producer or artifact dependency; a clean source copy fails ERR_MODULE_NOT_FOUND.

## Acceptance

Build ordinary WASM with pinned tooling and supply it to the full JavaScript suite; document the same prerequisites for contributors and verify clean-source execution.

## Root cause and fix

The `javascript-wasm` CI job fetches the standalone web lockfile, installs the
builder's exact wasm-bindgen CLI and existing Clang/LLVM/ripgrep tools, builds
ordinary WASM, and uploads `crates/hoyadb-web/pkg/`. The Node job explicitly
requires those artifacts and retains the full JavaScript and RN host suites.
CONTRIBUTING documents the same clean-source prerequisites.

The packaging regression failed because the original Node job had no asset
dependency, then passed after the CI correction (four packaging tests total).
The CI YAML also parsed successfully. A fresh source copy without node_modules
or generated inputs was supplied only the ordinary WASM producer artifact:
offline npm install passed, then 187 client, 11 testing and 10 RN host tests
passed. This checks the local artifact-consumption path, not remote GitLab
execution, image availability, Linux compilation or network installation.

