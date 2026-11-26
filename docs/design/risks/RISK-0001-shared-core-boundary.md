---
id: RISK-0001
title: Shared-core ownership and scheduling across WASM and native bindings
type: risk
status: active
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0002]
tags: [architecture, delivery, riskiest-assumption]
---

# RISK-0001: Shared-core ownership and scheduling

## Description

A single Rust core runs through WASM and native FFI (`ADR-0001`). Ownership,
serialization, host scheduling and asynchronous callback completion can fail
at each language/runtime boundary even when the core's own tests pass.

## Impact

Broken lifecycle handling can lose notifications, invoke stopped callbacks or
retain resources. Excessive boundary cost can undermine responsiveness. A failure
that invalidates shared-core assumptions requires reviewing `ADR-0001`,
`ARC-0001` and `ARC-0002`.

## Response

Follow `API-0011` for owned payloads, subscription liveness, host dispatch and
callback completion. Validate every claimed target with its build and runtime
gates (`ADR-0008`). Hold no core borrow or registry lock across foreign code.
Passing a host test does not certify mobile or browser persistence behavior.

Client/server handler compatibility remains an independent risk (`RISK-0002`).
Multi-process store ownership remains a separate boundary (`RISK-0003`).

## Traceability

- Parent: `OBJ-0002`.
- Arises from: `ADR-0001`.
- Response: `ADR-0008`, `API-0011`.
- Affected: `ARC-0001`, `ARC-0002`.
