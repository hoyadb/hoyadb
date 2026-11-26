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

Subsequent operator instructions authorized a complete local/GitHub history
reset, the `hoyadb/hoyadb` public destination, and committing/pushing verified
delivery work. Main starts at a new root commit; the previous local history was
preserved in a verified external Git bundle. GitLab's server history is untouched.

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

## Hosted-run regression acceptance

The initial hosted run found an exhaustive-match compile failure in the shared
experimental command fixture. Its status formatter must preserve every core
state, including `Discarded` as `"discarded"` (SPEC-0040). A table regression
test must cover all state spellings, and both native and browser spike gates
must compile without suppressing the exhaustive match. This changes only the
fixture translation; core storage and public SDK contracts remain unchanged.

Android consumer guard unit tests must use task-owned synthetic compiler/runtime
files with real digests, never require the developer's installed toolchain.
Compiler drift, runtime drift and Java injection must each fail with their own
guard reason before any subprocess activation. Production toolchain review and
native execution gates remain unchanged.

Warning-denied browser compilation must register the existing shared migration
backfill, tombstone mapping-target and rolled-back sent-cancellation regression
cases in the browser harness. Its client must request the added cases and check
their exact result names. Do not silence unused-case warnings or claim browser
execution from compilation alone.

## Rollback

Remove only these workflows/tooling and restore release metadata together. Keep
GitLab CI and pre-existing working-tree edits. Registry publication is not an
automatic rollback operation; released versions are never overwritten.
