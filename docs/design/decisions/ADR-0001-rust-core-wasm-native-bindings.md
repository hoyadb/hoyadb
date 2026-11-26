---
id: ADR-0001
title: Platform & language — one Rust core, compiled to WASM and exposed via native bindings
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0002]
tags: [architecture, platform, rust, wasm]
---

# ADR-0001: Platform & language — one Rust core, compiled to WASM and exposed via native bindings

## Decision

Use one Rust core, compiled to WebAssembly for web and exposed through native
bindings for platform SDKs. Shared storage and synchronization semantics belong
in core; wrappers translate ownership, values, scheduling and platform errors.

## Consequences

A shared implementation reduces semantic drift across platforms. Each target
still requires a verified binding, explicit notification delivery and lifecycle
handling. FFI/WASM overhead, copying and scheduling require measurement; a core
language choice does not establish end-to-end performance or platform support.
The binding mechanism remains an implementation choice within ARC-0002.

## Traceability

OBJ-0002, ARC-0001, ARC-0002, ADR-0008 and RISK-0001 govern this boundary.
