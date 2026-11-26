---
id: SPEC-0045
title: Release-oriented source layout and independent public contribution checks
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-30
updated: 2026-10-30
parents: [ARC-0001]
satisfies: [REQ-0048, NFR-0005]
depends_on: [ADR-0029, API-0011, API-0012]
tags: [distribution, tooling, compatibility]
---

# SPEC-0045: Public repository layout

## Scope and behavior

Implement the operator-approved source reorganization: Rust production bridges
move from `bindings/native` and `bindings/web` to `crates/hoyadb-native` and
`crates/hoyadb-web`; npm packages become `packages/client`, `packages/testing`
and `packages/react-native`; shared JavaScript build/consumer tooling lives in
`tools/javascript/scripts`, consumer fixtures in `tests/packaging/javascript`,
and notices in `licenses/javascript`; executable documentation tutorials move to
`examples`.
Swift and Android SDKs remain under `bindings`. Package names, exports, behavior,
dependency versions and existing platform acceptance requirements are preserved.

Keep the native and web bridges as standalone Cargo workspaces/exclusions with
their existing lockfiles. Keep the npm coordinator's existing client/testing
workspace membership; React Native retains its separately reviewed dependency
and consumer setup. No new dependency or package-manager switch is needed.

The public contribution contract is self-contained in AGENTS.md, CONTRIBUTING.md
and linked project guides. Extract only the dependency-free traceability checker,
RTM generator, regression tests and fixtures into `tools/traceability`, recording
their exact source revision and hashes. Do not copy the private framework's
rules, templates, examples or skills into the public subset. Preserve all
validation behavior and CI gates. Remove the tracked private gitlink and
submodule configuration while leaving the existing local checkout intact and
ignored. Private maintainer tooling is optional for contributors.

Update active scripts, source links, manifests and documentation for new paths.
Keep accepted ADR bodies, append-only decision-log entries, prior-art imports
and historical evidence bytes unchanged. A current path map explains historical
references. Generated artifacts remain ignored and must not be recertified merely
because they moved. Do not claim archived HEAD builds verify uncommitted moves.

## Acceptance and verification

TC-0049 specifies isolated public-tool execution, malformed-artifact rejection,
RTM stale-file rejection, package consumers and preservation boundaries. Run the
existing Rust workspace/native tests, npm/RN tests, public documentation tooling,
formatting, lint and relevant builds before and after the moves. Native documentation runners choose the source path
recorded in their selected commit, supporting both layouts without substituting
working-tree code or rewriting archived source. Validate actual
packed npm packages outside the checkout. Check that history/notice bytes and
existing user changes survive. Report platform/toolchain checks that cannot run.

## Non-goals and rollback

No API or database behavior changes, dependency activation, new CLI, license
selection, registry publication, public repository creation, history rewriting,
mobile support expansion or security-policy weakening. A standalone security
reporting address is not invented; public setup records the remaining decision.
Reverse the path map and restore tracked submodule metadata to roll back source
organization. The local private checkout is retained throughout.
