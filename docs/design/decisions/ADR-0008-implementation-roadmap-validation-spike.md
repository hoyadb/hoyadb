---
id: ADR-0008
title: Sequence the work as an implementation roadmap whose first step validates the Rust core shape
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0002]
tags: [roadmap, process, spike]
---

# ADR-0008: Validate the shared core on target platforms

## Context

A shared Rust core requires executable validation through WASM and native bindings.
Compilation alone cannot establish callback lifetime, scheduling or runtime support.

## Decision outcome

Validate storage operations, reactive notifications and command callbacks across
both boundaries before claiming platform support. Core/platform ownership stays
in `ARC-0002`; concrete SDK contracts and delivery gates stay in `API-0011`.
A failure that invalidates the shared-core architecture requires reviewing `ADR-0001`.

## Consequences

Each supported target needs its own build and runtime checks. Host tests cannot
certify mobile or browser persistence behavior. Handler compatibility remains an
independent integration risk (`RISK-0002`).

## Confirmation

Run a Rust core in a browser through WASM and through a native binding. Exercise
owned notifications, cancellation and asynchronous callback completion as well
as storage calls; report any unverified target explicitly.

## Traceability

- Parent: `OBJ-0002`.
- Related: `ADR-0001`, `ARC-0002`, `RISK-0001`, `RISK-0003`.
