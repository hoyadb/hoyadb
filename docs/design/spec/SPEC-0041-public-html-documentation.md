---
id: SPEC-0041
title: Executable public HTML documentation
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-27
updated: 2026-10-27
parents: [ARC-0001]
satisfies: [REQ-0047]
depends_on: [API-0004, API-0005, API-0006, API-0009, API-0010, API-0011, UX-0003]
tags: [documentation, public-html, executable-examples]
---

# SPEC-0041: Executable public HTML documentation

## Need and scope
The operator requests complete HTML documentation: getting started, concepts,
practical guides and public reference, with every example tested and Claude
assistance. Existing Markdown combines implemented APIs and future design.
Provide an offline-readable static site for the current library, built without
new dependencies. Include Rust reference generated from the actual public
crates and SDK declaration reference. Draft-only features are labelled pending.

## Behavior and acceptance
Source pages, executable examples and the build/verification scripts live in
`docs/public/`. The generated site is `docs/public/dist/index.html` (ignored
build output). Navigation, search, responsive layout, source links and a
verification page support the reading flow in UX-0003. No CDN, telemetry or
runtime network request is needed. Code blocks are included from tested files;
no handwritten untested example is represented as working code. Reference
signatures are generated from source declarations or rustdoc.

Coverage includes installation/local packages, CRUD, types and schemas, queries,
relations, migrations, observations, commands, durable retries, reconciliation,
entity sync, pull/merge, storage/security, errors, testing and platform setup.
Document public methods and limitations without claiming all planned features
or platform gates are complete. Existing user edits are preserved. No product
API, storage format, dependency, release or user store changes are authorized.
JVM/Android execution remains held; Android reference is descriptive source
reference, with no executable Kotlin example or runtime verification claim.

## Execution matrix and version
Claude assistance means authoring and review assistance; the site has no in-page
AI assistant or network feature. This is the root interpretation of "Use Claude
to help you", not a new product requirement. All displayed examples are runnable
Rust or JavaScript. Rust examples run offline via Cargo in examples.
JavaScript runs with Node against the ordinary WASM built offline by the existing
crates/hoyadb-web/build.sh and exact installed wasm-bindgen CLI; no download occurs.
Swift, React Native and Android pages contain descriptive source references,
not unexecuted application examples. JVM/Android execution remains held.

Every code block identifies executable example or source-only declaration,
source path and verification result. Every page shows the source commit/build
date and unreleased label. The verification report also records source hashes,
commands, actual outcomes and limits. A guide symbol inventory is validated
against generated current reference. Search has an inline index and works on
file:// without fetch/XHR. Structural accessibility checks are supplemented by
a recorded browser check at 320 CSS pixels and keyboard focus. No automated
structural result alone claims WCAG conformance. Persistence examples create and
remove their own temporary directory and use a throwaway fixture key.

## Verification and rollback
TC-0045 specifies tests. Execute every displayed executable example against
real library implementations, not a replacement database. Build all ordinary
Rust public reference pages, check site-local links and fragments, escaping,
example inclusion, search and accessibility structure. Claude reviews content
and API coverage. Record actual commands and outcomes on the site. A compile
error is setup failure, not a behavioral RED. Removing only these new docs and
tooling reverses this change; no user data or library behavior changes.

## Binding-first correction — 2026-10-28

Operator feedback supersedes the Rust-first entry flow. SPEC-0042/API-0012 and
TC-0046 require implementation of the original-style JS application layer and
a short app-first tutorial. Named source regions may be displayed separately
when the complete source file executes in the verification run. Low-level
Facade/WASM testing bootstrap belongs in advanced embedding material, not the
first binding example. No platform execution hold is lifted.

## Per-binding quickstarts — 2026-10-28
SPEC-0043/TC-0047 add language-aware coloring and explicit getting-started guides
for every binding. Executed native examples use local artifacts and record their
hashes. Where execution is held, source-checked snippets carry an explicit
not-executed label and reason. This replaces the initial native-reference-only
presentation restriction; it does not lift a platform execution hold.
