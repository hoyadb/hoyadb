---
id: DEF-0060
title: Apple build manifest omits Cargo configuration and build inputs
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-31
updated: 2026-10-31
parents: [SPEC-0047]
affects: [SPEC-0047]
regression_for: [TC-0051]
tags: [packaging, provenance, tooling]
---

# DEF-0060: Apple build source inventory omits build inputs

## Symptom and confirmed source cause

`bindings/swift/build-artifacts.py:sources` lists Rust source files, crate
manifests, lockfiles and headers but omits the root `Cargo.toml`. It also has no
capture of crate build scripts, non-Rust source resources or Cargo configuration.
A change to one of those build inputs can leave the recorded before/after map
unchanged. This was identified while preparing the final platform rebuild;
earlier retained certificates remain historical and are not rewritten.

## Regression and acceptance

Add fixture-based inventory tests before correcting the capture. Changing root
Cargo configuration, an existing local crate build script or a source resource
must change the inventory; adding or removing such inputs must also change it.
Capture the builder itself and applicable repository Cargo configuration. Keep
generated build outputs out of the source inventory. Reject missing required
inputs and source drift before writing a successful build record.

This inventory is source evidence, not proof of runtime behavior or a claim to
capture arbitrary environment state and external toolchain inputs. Final native
artifact verification still records those separately.

## Correction and verification

The unchanged builder failed eight assertions across five fixture test groups:
root manifest and builder changes, new scripts/resources, configuration changes,
and a missing root manifest were invisible; the generated-output exclusion
control passed. Source capture now includes those inputs and re-enumerates them
at both ends of compilation. All nine packaging test groups pass, including
the five new inventory groups. No Apple binary was built for these unit checks.

The first implementation run exposed a fixture path mismatch between macOS
`/var` and `/private/var`, not a product failure. The fixture now canonicalizes
its root as the real builder does. The corrected fixture was rerun against the
unchanged old function and reproduced the same eight behavioral failures before
the final GREEN. Both failed runs and old source bytes remain in
`/tmp/hoyadb-followup-20261031/apple-source-inventory-*`.

The scope remains the five currently declared native crates and conventional
`build.rs`/`src` resource layout. An altered local dependency graph or resources
read outside that layout require inventory review, as the Swift README states.
Independent final build-input and toolchain capture is still required. Claude
Fable 5.1 reviewed the implementation and fixture method, independently reran
all nine packaging tests, and found no actionable defect. It confirmed the
declared source/toolchain scope and harmless over-capture of crate-local Cargo
configuration; ordinary invocation uses the repository root as its working
directory.
