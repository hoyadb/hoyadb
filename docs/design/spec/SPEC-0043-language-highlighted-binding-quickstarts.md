---
id: SPEC-0043
title: Language-highlighted examples and getting started for every binding
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-28
updated: 2026-10-28
parents: [ARC-0001]
satisfies: [REQ-0047]
depends_on: [SPEC-0041, SPEC-0042, API-0011, API-0012, UX-0003]
tags: [documentation, bindings, syntax-highlighting]
---

# SPEC-0043: Readable examples for every binding

## Origin and scope
The operator requests syntax colors appropriate to each example language and
the same kind of getting started for every bound language. The current site has
plain code blocks and a detailed JavaScript flow, while other SDKs mainly have
source reference. Add a consistent entry for JavaScript, TypeScript, Rust,
Swift, Kotlin/Android, React Native and the native C ABI. This changes public
documentation and its verification tools, not product behavior or dependencies.

## Rendering
Highlight at build time with a small language-aware lexical renderer using the
Python standard library. It is syntax coloring, not a compiler/parser. Infer
language from the actual source extension, including .d.ts and .tsx. Cover
JavaScript, TypeScript/TSX, Rust, Swift, Kotlin, C, JSON and shell; unknown files
remain escaped text. Show the language in code-block labels and attributes.
Rustdoc retains its own generated highlighting. Color keywords, strings,
comments, numbers, types and callable names with the shared tokens in UX-0003.
Highlight executable examples, included regions and source/declaration views.

Always escape source before emitting markup; token spans are the only inserted
markup. Extracted text must equal the original code exactly, including Unicode,
whitespace, quotes, raw/triple strings and comment delimiters inside strings.
No CDN, new dependency, runtime fetch or highlighter network call is allowed.
Token colors meet 4.5:1 against the code background; color is an enhancement,
not the only language or verification-status indicator. Preserve selectable
source, keyboard focus and code scrolling without page overflow at 320 px.

## Getting-started coverage
Each entry states local package/import setup, opens one shared owner, defines a
small schema, writes/queries rows, observes and disposes, explains a local
command handler separately from backend transport, performs sync/reconciliation
where available, and closes. Use actual APIs and idioms of that binding; do not
invent JavaScript convenience methods for Swift, Kotlin or C. Link reference
and protected-storage/key guidance. C teaches its framed ABI and borrowed reply
ownership explicitly, rather than claiming a fluent database object.

Source regions come from complete checked-in examples, with exact language and
verification labels. Kotlin and native-only React Native bootstrap examples may
be source-checked and labelled "Source-checked · not compiled or executed" while execution
is held or no suitable runtime is available. Their common executable logic is
verified against the real core where practical, without claiming native runtime
coverage. Source-checked means included from a checked-in file whose named symbols
are validated against current declarations; it does not mean compiled or executed.
Use "not executed" alone only when compilation actually passed. All source-only blocks name the source path, reason and verification
entry; none may be called executed. This request supersedes SPEC-0041's initial
restriction to descriptive native reference pages, not its execution holds.

## Verification matrix
- Rust and existing JavaScript: retain and run all current executable examples.
- TypeScript: typecheck with the installed compiler against local public package
  declarations, emit and execute the example with Node and the actual WASM core.
- Swift: compile and execute with the installed Swift toolchain and local native
  artifact. Record exact artifact hashes. Rebuild the local macOS artifact from
  current source if required; do not download targets or dependencies.
- C: compile/link against the local native library and execute real ABI calls.
- React Native: test shared application logic through the real core; source-check
  the native bootstrap and state the runtime limit. No emulator/dependency setup
  is implied by writing its tutorial.
- Kotlin: the existing JVM/Android execution hold remains until the operator
  explicitly lifts it; otherwise source-check and label, never claim execution.

TC-0047 covers coloring, source preservation, coverage, execution labels and
negative controls. Verification manifests include new tooling, examples,
packages and native artifacts used by a run. Native builds go only to task-owned,
ignored target/ locations; never overwrite packaged artifacts or a vendored
XCFramework. Record source and artifact hashes for each build. No user store is opened. The final
site is built and inspected in Chrome, with local search, contrast and reflow
checks. Existing user edits and frozen remediation packets remain untouched.

## Rollback
Revert only this documentation/tooling addition. There is no product API,
storage, transport protocol, schema format, runtime dependency or release change.

## Readability refinement (operator feedback, 2026-10-28)
Tutorial pages teach one operation at a time: introduce the purpose, include a
small named region from the tested example, then explain the result and relevant
application practice. Prefer roughly 5–25 lines per block; split longer logical
phases where useful. Do not shorten an example by omitting required operations
or inventing APIs. Whole executable files remain linked for running and copying.
Reference/source pages may show complete files.

Keep verification-only assertions, fixture plumbing and retry-test scaffolding
in the runnable source outside tutorial regions. Error handling that an app
needs remains visible. Explain shared helpers and when example-only unwraps,
fixed clocks, stand-in transports and disposable keys must be replaced in an
application. Use whitespace and descriptive local names. Preserve all executable
assertions and completion markers, and rerun every changed example. Review all
binding quickstarts and conceptual guides for these conventions; the change is
presentation and example organization only, with no library API change.

A displayed result comment must be backed by an assertion immediately after its
region in the complete example. Prose identifies example helpers whose definitions
are outside a snippet. Each region contains whole statements (balanced delimiters
outside strings/comments), without ellipses standing in for required code. Page
steps follow source-region order, or explicitly explain when a reference section
revisits a step out of order.

## Rust command semantics refinement (operator feedback, 2026-10-28)
Use a concrete Todo example to explain create-task-v1(title) and
complete-task-v1(task_id). Show argument declarations separately from invocation
values, a named application handler, and CommandResult.key versus the JSON result
chosen by that handler. State that the Rust core validates definitions and admits
scopes; the application calls its handler between begin and finish rather than
registering a handler on CommandDefinition. Business validation belongs in that
handler, and failures abort the active scope.

The completion handler updates the task and inserts an activity row using the
same request. One finish commits both writes and one queue record. Test a real
late failure after staging the update, abort it, and verify unchanged rows and
queue state. Preserve explicit server mappings for all command-created IDs.
Keep the wire/retry discussion separate from the basic local-command walkthrough.
No new library API is implied by application helper functions in this example.
The completion definition declares task_id as a reference path. Demonstrate
completion before the first sync: its envelope waits for creation, then uses
the authoritative task ID after reconciliation. A retry after abort acquires a
new request and begins again with the same intent; aborted requests are not reused.
