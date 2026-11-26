---
id: SPEC-0050
title: GitHub verification, package delivery and documentation hosting
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-26
updated: 2026-11-26
parents: [ARC-0001]
satisfies: [REQ-0048, NFR-0005]
depends_on: [SPEC-0044, SPEC-0045, SPEC-0046, SPEC-0047, SPEC-0041, ADR-0035]
tags: [distribution, tooling, documentation]
---

# SPEC-0050: GitHub delivery

## Scope and authorization

The operator requested GitHub CI to check, build and publish all three npm
packages and host the public documentation on GitHub Pages. The operator chose
MIT for first-party code. Preserve third-party notices and accepted decisions.
Prepare workflows and tooling; this request does not execute a publication,
commit, push, account configuration or repository migration.

## Acceptance before implementation

- Preserve every existing GitLab verification command, adding fresh packed npm
  consumer checks. Fetch locked dependencies before offline execution on clean
  runners. Build ordinary WASM before JavaScript tests and package construction.
- Give pull-request jobs read-only access, no secrets and no persistent native
  runners. Pin external actions to verified full commit hashes. The final CI
  status must reject failed, cancelled, missing and skipped prerequisite jobs.
- Release only a strict vMAJOR.MINOR.PATCH tag reachable from main, with matching
  package versions, MIT metadata and the root license. Run CI for that exact ref.
  Use the existing native builders, provenance/notice validators and RN packer;
  never recertify an older binary or suppress a failed gate.
- Use a reviewed native runner for the existing locally pinned Apple/Android
  recipes. Its required tools, paths, caches and runtime acceptance remain explicit
  provisioning/release gates. No mobile conformance is inferred from host tests.
- Publish tested client/testing/RN tarballs to npm with provenance, Android to
  Maven Central with complete metadata, signatures and checksums, and versioned
  Apple binaries to GitHub Releases. Keep registry credentials in publishing jobs.
  Namespace ownership and account setup are external prerequisites, not assumptions.
- Build Pages with the complete existing documentation verification and browser
  checks before deployment. Keep its reviewed native runner away from PR events.
- Keep the root npm coordinator private. Include MIT in each distributed package;
  retain the third-party license closures. Review the RN manifest metadata change
  independently of its unchanged runtime exports before updating the source pin.
- Replace the active native notice validators' former no-license requirement
  with the selected MIT identity and hash-bound license text. Retain all
  third-party classification, byte-identity, provider and lock checks. Historical
  packets remain unchanged; later source packaging requires fresh evidence.

TC-0054 covers the executable validation and workflow contract. Account setup and
actual hosted/mobile execution must be reported separately from local validation.

## Rollback

Remove only these workflows/tooling and restore release metadata together. Keep
GitLab CI and pre-existing working-tree edits. Registry publication is not an
automatic rollback operation; released versions are never overwritten.
