---
id: SPEC-0042
title: Implement the original JavaScript application workflow and teach it directly
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-28
updated: 2026-10-28
parents: [ARC-0002]
satisfies: [REQ-0046, REQ-0047, REQ-0010, REQ-0011, REQ-0023]
depends_on: [API-0012, API-0011, SPEC-0041, UX-0003]
tags: [javascript, developer-experience, documentation]
---

# SPEC-0042: Implement and teach the JavaScript application workflow

## Need and checked gap
Operator feedback: database instantiation, queries, observations, commands and
handlers are unclear in the HTML documentation. The operator further requests
completion of library support for the original JS examples before reworking the
docs. Checked original examples in guides/walkthrough.md, platform-bindings.md,
testing.md and SPEC-0008 use builders, plain values, a bound command.execute,
row callbacks, table record updates and automatic pull. API-0011 exposes tagged
DTOs and manual runtime setup instead; automatic startSync currently only pushes.
Core already supplies the underlying transactions, dependencies and resolution.

## Scope and approach
Implement API-0012 as an additive JS application facade over existing core/SDK
calls, keeping advanced tagged /host behavior. Add schema/value builders, plain
queries/records/observations, explicit owner-bound commands, scoped handlers,
object transport adaptation, a Node bootstrap, explicit entity configuration
and automatic host pull. React Native reuses the shared facade on its existing
channel; this establishes source/host tests, not mobile runtime verification.
No core algorithm, existing user store, dependency or held platform is changed.

## Documentation and acceptance
Getting started becomes binding-first with a platform selector. The primary
application tutorial shows schema, one db module, query/fetch, observe, command
definition, local handler, execute, backend handler/transport, reconciliation
and disposal as short sequential snippets from one executable example.
An ordered file map shows imports and the difference between the public app
db, the tracked handler db and the backend. No undeclared renderOrders helper,
ambient db, Facade plumbing or test assertions in the beginner snippets.
Complete source remains available separately. Original illustrative guides link
to the implemented workflow and clearly explain security/explicit-contract
precisions instead of presenting unsupported syntax as current.

Every included snippet must be an exact named region in a fully executed file;
missing/duplicate/nested regions fail the build. A Node real-WASM test and a
real Chrome worker test run the same application flow. Test the original-style
builders/operators, plain values, async scoped atomicity, dependencies, observer
cleanup, retained recovery, failed transport, pull/epoch timers and wrong key
preflight. Run existing JS tests, package/type checks and HTML/link/browser
checks. Source-only Swift/Kotlin reference stays labelled; JVM remains held.

## Rollback
Revert only the facade additions, exports, tests and docs. No persisted format
change or migration is needed; all work uses the existing durable core records.
