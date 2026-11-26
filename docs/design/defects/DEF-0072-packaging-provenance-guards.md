---
id: DEF-0072
title: Packaging accepts stale native or WASM artifacts and unbound notice locks
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-10
updated: 2026-11-10
parents: [SPEC-0034]
affects: [SPEC-0033, SPEC-0046]
regression_for: [TC-0038, TC-0036]
tags: [packaging, provenance, review, regression]
---

# DEF-0072: Packaging accepts stale native or WASM artifacts and unbound notice locks

## Symptom

The Android notice index recorded a stale root `Cargo.lock` digest and a lock path that no longer exists, and the packager did not compare either. AAR packaging bound the Rust static archive bytes but not the Rust sources. `build.mjs` copied an existing WASM beside a manifest of current source hashes. The web `build.sh` test-control guard passed when `rg` was missing and could package a stale crate-local WASM under a redirected target directory. A selected slice without `evidence.json` raised an uncaught exception.

## Acceptance

Packaging rejects a notice index bound to other lock bytes or paths, a native archive older than or built from a different Rust source closure, a prebuilt WASM older than its sources, a missing guard tool, and missing or malformed native evidence.

## Root cause and fix

`package-aar.py` verifies lock path and digest and the recorded Rust source closure (`rust_sources.py`, recorded by `build-jni.py`); `build.mjs` refuses stale generated assets through `freshness.mjs`, a modification-time guard and not provenance; `build.sh` pins its target directory and distinguishes `rg` exit codes. The Android index lock binding was corrected by hand because its generator is historical evidence.

Existing JNI evidence carries no source closure, so AAR packaging refuses it until the libraries are relinked. The Apple notice index still labels its lock `bindings/native/Cargo.lock`; it is copied byte-for-byte into the packaged framework and was left for the next authorized Apple rebuild. Android tool tests: 72 passed; web build-script tests: 5 passed; `tests/packaging`: 10 passed after its fixture copied `freshness.mjs`.

## Follow-up

The Apple notice index now labels its lock `crates/hoyadb-native/Cargo.lock` and `validate_corpus` rejects another label. The local Apple framework and the four JNI libraries were rebuilt; the rebuild changed library bytes, so the six per-library digests in `crates/hoyadb-native/third-party/index.json` were rebound from a fresh `llvm-nm` capture that is **not yet filed or reviewed**: `provider_evidence_source` still names the 2026-11-07 record. This record stays open until a maintainer files or rejects that capture. The `AN02BuildOrchestration` fixture now creates the root `Cargo.toml` the build script hashes (55 tests OK).
