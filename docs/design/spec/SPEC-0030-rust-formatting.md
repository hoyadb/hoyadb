---
id: SPEC-0030
title: Apply consistent Rust formatting without changing behavior
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-23
updated: 2026-10-24
parents: [SPEC-0019]
satisfies: [NFR-0005]
tags: [tooling, formatting, rust]
---

# SPEC-0030: Rust formatting

Origin: operator requested a pause in feature implementation to establish
formatting rules and clean up the existing code.

Use the pinned stable rustfmt across the workspace, shared conformance tests
and isolated Rust spikes. Set edition/style edition 2024, four spaces, LF,
a 100-column layout target, multiline struct literals and named enum variants,
and multiline if/else and let/else blocks. Keep one blank line between
declarations and methods; stable rustfmt preserves these separators but does
not insert or enforce them. Add EditorConfig defaults and document the commands.

Acceptance: TC-0033 verifies the formatter's layout, formatting checks for all
five Cargo manifests, and the existing workspace/host-spike tests. Add the
layout regression before changing the configuration and observe the expected
diff. Existing CI formatting checks inherit the root configuration; run the
layout regression in the workspace job too.

This is a formatting-only change preserving NFR-0005's compatibility boundary:
no public API, stored data, dependency, algorithm or error behavior changes.
JavaScript, Swift, Python, research, generated evidence and the governance
submodule are outside this pass. No new feature work is authorized here.

Rollback: revert only this pass's configuration, documentation and whitespace
changes, preserving pre-existing user edits. No database operation is needed.

## Executed verification — 2026-10-24

- F01: the layout regression failed with the expected compact-layout diff
  before configuration, then passed; shell syntax also passed.
- F02: all five Cargo formatting checks passed. Comparing all 77 Rust files
  against HEAD formatted with the new configuration found only additional
  blank-line separators; literal-text comparison found no changes.
- F03: `cargo test --workspace --offline --locked` passed 234 tests across
  23 suites. Cross-platform host tests passed 11 tests and native spike tests
  passed 20 tests.
- Workspace all-targets Clippy and all four spike Clippy checks passed with
  warnings denied. Browser Clippy used the README's LLVM 22 compiler/ar paths;
  the first attempt with the default Apple compiler failed because it lacks
  the wasm32 backend.
- Core/memory WASM build, traceability (0 violations), RTM freshness and
  `git diff --check` passed. No remote CI or browser/Swift runtime execution
  is claimed for this formatting-only pass.

Existing CI runner tags and untracked `.agents/` work were preserved. Changes
remain uncommitted; no commit, merge or push was requested for this pass.
